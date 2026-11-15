//! The town's persistent state. It lives in its own file, `town.json`, which
//! outlives every run: permadeath deletes `savegame.json` and never this file.
//!
//! `TownState` is plain serde data so the trade simulation (E04) can be a
//! pure function over it. Parsing and loading are pure functions too; the
//! Bevy systems at the bottom only call them.

use std::collections::BTreeMap;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[cfg(not(target_arch = "wasm32"))]
use std::path::{Path, PathBuf};

/// Bump when `TownState` changes in a way old files cannot load into.
pub const TOWN_VERSION: u32 = 1;

#[cfg(not(target_arch = "wasm32"))]
const TOWN_FILE: &str = "town.json";

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Sector {
    Smithing,
    Alchemy,
    Textiles,
    Provisions,
    Faith,
    Arcana,
}

impl Sector {
    pub const ALL: [Sector; 6] = [
        Sector::Smithing,
        Sector::Alchemy,
        Sector::Textiles,
        Sector::Provisions,
        Sector::Faith,
        Sector::Arcana,
    ];
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Vendor {
    Blacksmith,
    Alchemist,
    Clothier,
    Barkeep,
    Priest,
}

impl Vendor {
    pub const ALL: [Vendor; 5] = [
        Vendor::Blacksmith,
        Vendor::Alchemist,
        Vendor::Clothier,
        Vendor::Barkeep,
        Vendor::Priest,
    ];
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct SectorState {
    /// Gold value (at base value) of this sector's goods held in town.
    pub stock: i32,
    /// Starts at 1. Gates vendor tiers (E05) and the town's look (E06).
    pub level: u32,
    /// Trade volume towards the next level (E04, shown by E08).
    pub progress: i32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct NewsEntry {
    pub day: u32,
    pub text: String,
}

#[derive(Resource, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct TownState {
    pub version: u32,
    /// Fixed when the town is founded so its layout is stable (E06).
    pub seed: u64,
    /// Days since the town was founded; advanced by the simulation (E04).
    pub day: u32,
    /// The town's own gold, separate from the party's `Purse`.
    pub treasury: i32,
    pub sectors: BTreeMap<Sector, SectorState>,
    /// What each vendor has for sale: raws item name to quantity (E02, E05).
    pub vendor_stock: BTreeMap<Vendor, BTreeMap<String, u32>>,
    /// Oldest first (E04 writes it, E08 shows it).
    pub news: Vec<NewsEntry>,
}

impl TownState {
    /// A newly founded town: every sector at level 1, nothing in stock.
    pub fn fresh(seed: u64) -> Self {
        TownState {
            version: TOWN_VERSION,
            seed,
            day: 0,
            treasury: 0,
            sectors: Sector::ALL
                .into_iter()
                .map(|s| {
                    (
                        s,
                        SectorState {
                            stock: 0,
                            level: 1,
                            progress: 0,
                        },
                    )
                })
                .collect(),
            vendor_stock: Vendor::ALL
                .into_iter()
                .map(|v| (v, BTreeMap::new()))
                .collect(),
            news: Vec::new(),
        }
    }

    /// 1 for a fresh town, plus one for every sector level gained, so each
    /// level-up anywhere shows.
    pub fn level(&self) -> u32 {
        1 + self.sectors.values().map(|s| s.level - 1).sum::<u32>()
    }
}

/// Parse `town.json`, rejecting any version other than `TOWN_VERSION`.
pub fn parse_town(json: &str) -> Result<TownState, String> {
    #[derive(Deserialize)]
    struct VersionOnly {
        version: u32,
    }
    let found = serde_json::from_str::<VersionOnly>(json)
        .map_err(|e| format!("no readable version: {e}"))?
        .version;
    if found != TOWN_VERSION {
        return Err(format!("version {found}, expected {TOWN_VERSION}"));
    }
    serde_json::from_str(json).map_err(|e| e.to_string())
}

/// Read the town at `path`. A missing file gives a fresh town. An unreadable
/// or wrong-version file is renamed aside (never overwritten) and also gives
/// a fresh town. The flag is false only when that rename failed, meaning the
/// bad file is still in place and must not be saved over.
#[cfg(not(target_arch = "wasm32"))]
pub fn load_town(path: &Path, fresh_seed: u64) -> (TownState, bool) {
    let problem = match std::fs::read_to_string(path) {
        Ok(json) => match parse_town(&json) {
            Ok(town) => return (town, true),
            Err(e) => e,
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            info!("No {} yet, founding a fresh town", path.display());
            return (TownState::fresh(fresh_seed), true);
        }
        Err(e) => e.to_string(),
    };

    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let aside = PathBuf::from(format!("{}.bad-{secs}", path.display()));
    let renamed = std::fs::rename(path, &aside);
    match &renamed {
        Ok(()) => warn!(
            "Could not load {} ({problem}); moved it to {} and founded a fresh town",
            path.display(),
            aside.display()
        ),
        Err(e) => error!(
            "Could not load {} ({problem}) nor move it aside ({e}); using a fresh town that will not be saved",
            path.display()
        ),
    }
    (TownState::fresh(fresh_seed), renamed.is_ok())
}

/// Write via a temporary file and rename, so a crash mid-write cannot leave
/// a half-written town behind.
#[cfg(not(target_arch = "wasm32"))]
pub fn save_town(path: &Path, town: &TownState) -> std::io::Result<()> {
    let json = serde_json::to_string_pretty(town).map_err(std::io::Error::other)?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json)?;
    std::fs::rename(tmp, path)
}

// ============================================================================
// Bevy glue
// ============================================================================

/// Where the town is stored. A resource so tests can point it at a temp dir.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Resource)]
pub struct TownFile {
    pub path: PathBuf,
    /// False when a bad file could not be moved aside; saving is skipped.
    pub writable: bool,
}

#[cfg(not(target_arch = "wasm32"))]
impl Default for TownFile {
    fn default() -> Self {
        TownFile {
            path: PathBuf::from(TOWN_FILE),
            writable: true,
        }
    }
}

pub struct TownPlugin;

impl Plugin for TownPlugin {
    fn build(&self, app: &mut App) {
        #[cfg(not(target_arch = "wasm32"))]
        app.init_resource::<TownFile>();
        // Saving on every change also writes a fresh town at once, which
        // pins its seed.
        app.add_systems(Startup, load_town_system).add_systems(
            Update,
            save_town_system.run_if(resource_changed::<TownState>),
        );
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn load_town_system(mut commands: Commands, mut file: ResMut<TownFile>) {
    let (town, writable) = load_town(&file.path, rand::random());
    file.writable = writable;
    commands.insert_resource(town);
}

#[cfg(not(target_arch = "wasm32"))]
pub fn save_town_system(town: Res<TownState>, file: Res<TownFile>) {
    if !file.writable {
        return;
    }
    if let Err(e) = save_town(&file.path, &town) {
        error!("Failed to save {}: {e}", file.path.display());
    }
}

/// WASM has no filesystem: the town lives in memory for the session.
#[cfg(target_arch = "wasm32")]
pub fn load_town_system(mut commands: Commands) {
    commands.insert_resource(TownState::fresh(rand::random()));
}

#[cfg(target_arch = "wasm32")]
pub fn save_town_system(_town: Res<TownState>) {}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("roguelike-town-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn busy_town() -> TownState {
        let mut town = TownState::fresh(7);
        town.day = 12;
        town.treasury = 340;
        town.sectors.get_mut(&Sector::Smithing).unwrap().level = 3;
        town.vendor_stock
            .get_mut(&Vendor::Alchemist)
            .unwrap()
            .insert("Health Potion".into(), 4);
        town.news.push(NewsEntry {
            day: 11,
            text: "A caravan left for Eastmarch.".into(),
        });
        town
    }

    #[test]
    fn town_round_trips_through_json() {
        let town = busy_town();
        let json = serde_json::to_string(&town).unwrap();
        assert_eq!(parse_town(&json), Ok(town));
    }

    #[test]
    fn town_round_trips_through_the_file() {
        let path = temp_dir("roundtrip").join("town.json");
        save_town(&path, &busy_town()).unwrap();
        assert_eq!(load_town(&path, 99), (busy_town(), true));
    }

    #[test]
    fn a_fresh_town_has_every_sector_at_level_one() {
        let town = TownState::fresh(5);
        assert_eq!(town.seed, 5);
        assert_eq!(town.version, TOWN_VERSION);
        assert_eq!(town.sectors.len(), 6);
        assert!(town.sectors.values().all(|s| s.level == 1 && s.stock == 0));
        assert_eq!(town.vendor_stock.len(), 5);
    }

    #[test]
    fn a_missing_file_founds_a_fresh_town() {
        let path = temp_dir("missing").join("town.json");
        assert_eq!(load_town(&path, 3), (TownState::fresh(3), true));
    }

    fn assert_moved_aside(dir: &Path, original: &str) {
        let path = dir.join("town.json");
        assert!(!path.exists(), "the bad file is no longer in the save slot");
        let aside: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        assert_eq!(aside.len(), 1);
        assert!(aside[0].to_string_lossy().contains("town.json.bad-"));
        assert_eq!(std::fs::read_to_string(&aside[0]).unwrap(), original);
    }

    #[test]
    fn a_corrupt_file_is_moved_aside_for_a_fresh_town() {
        let dir = temp_dir("corrupt");
        std::fs::write(dir.join("town.json"), "{ not json").unwrap();
        assert_eq!(
            load_town(&dir.join("town.json"), 3),
            (TownState::fresh(3), true)
        );
        assert_moved_aside(&dir, "{ not json");
    }

    #[test]
    fn a_wrong_version_is_moved_aside_for_a_fresh_town() {
        let dir = temp_dir("version");
        let mut town = busy_town();
        town.version = TOWN_VERSION + 1;
        let json = serde_json::to_string(&town).unwrap();
        std::fs::write(dir.join("town.json"), &json).unwrap();

        assert_eq!(
            parse_town(&json),
            Err(format!(
                "version {}, expected {TOWN_VERSION}",
                TOWN_VERSION + 1
            ))
        );
        assert_eq!(
            load_town(&dir.join("town.json"), 3),
            (TownState::fresh(3), true)
        );
        assert_moved_aside(&dir, &json);
    }

    #[test]
    fn the_plugin_saves_a_fresh_town_at_once_and_every_change_after() {
        let path = temp_dir("plugin").join("town.json");
        let mut app = App::new();
        app.add_plugins(TownPlugin);
        app.insert_resource(TownFile {
            path: path.clone(),
            writable: true,
        });

        app.update();
        let seed = app.world().resource::<TownState>().seed;
        assert_eq!(
            load_town(&path, 0).0,
            TownState::fresh(seed),
            "seed pinned on disk"
        );

        app.world_mut().resource_mut::<TownState>().treasury = 25;
        app.update();
        assert_eq!(load_town(&path, 0).0.treasury, 25);
    }

    /// Runs the real town systems, the real death system and the real
    /// new-game setup in one World, then reloads `town.json` as a new
    /// session would.
    #[test]
    fn a_town_change_survives_party_death_and_a_new_game() {
        use bevy::ecs::system::RunSystemOnce;

        use crate::combat::{delete_the_dead, CombatStats};
        use crate::economy::Purse;
        use crate::game_flow::RunState;
        use crate::gamelog::GameLog;
        use crate::map::Map;
        use crate::player::Player;
        use crate::resources::UiFont;
        use crate::rng::GameRng;

        let path = temp_dir("permadeath").join("town.json");
        let mut world = World::new();
        world.insert_resource(TownFile {
            path: path.clone(),
            writable: true,
        });
        world.run_system_once(load_town_system).unwrap();
        let seed = world.resource::<TownState>().seed;

        // The change: the town earns gold, and it is saved.
        world.resource_mut::<TownState>().treasury = 500;
        world.run_system_once(save_town_system).unwrap();

        // The party dies.
        world.insert_resource(GameLog {
            entries: Vec::new(),
        });
        world.init_resource::<NextState<RunState>>();
        world.insert_resource(UiFont(Handle::default()));
        world.spawn((
            Player,
            CombatStats {
                max_hp: 30,
                hp: 0,
                defense: 0,
                power: 0,
            },
        ));
        world.run_system_once(delete_the_dead).unwrap();
        assert!(world
            .resource::<GameLog>()
            .entries
            .contains(&"You are dead".to_string()));

        // A new game starts.
        crate::raws::load_raws();
        world.insert_resource(Map::default());
        world.insert_resource(GameRng::seeded(1));
        world.insert_resource(Purse { gold: 77 });
        world
            .run_system_once(
                |mut commands: Commands,
                 mut map: ResMut<Map>,
                 mut rng: ResMut<GameRng>,
                 font: Res<UiFont>| {
                    crate::levels::spawn_new_game(&mut commands, &mut map, &mut rng, &font, 1);
                },
            )
            .unwrap();
        assert_eq!(world.resource::<Purse>().gold, 0, "the new game really ran");

        assert_eq!(world.resource::<TownState>().treasury, 500);
        let (reloaded, _) = load_town(&path, 0);
        assert_eq!((reloaded.treasury, reloaded.seed), (500, seed));
    }
}
