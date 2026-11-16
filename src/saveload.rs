//! Save and load: `savegame.json`, written on Q and read by Continue.
//!
//! Every hero, mob, bystander, prop and item is saved by its raws name plus
//! the state that changes in play. Loading spawns each afresh from the raws
//! (so its model, weapon, archetype, light and cover come back as the raws
//! give them) and then writes the saved state over it. `snapshot` is the
//! check: a loaded world must snapshot the same as the world that was saved.
//!
//! A save from another version, or one that cannot be read, is reported and
//! moved aside (`savegame.json.bad-<secs>`), never half loaded.

use std::collections::HashMap;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use std::fs;
use std::path::Path;

use crate::abilities::{HeroAbility, HeroClass};
use crate::combat::CombatStats;
use crate::components::{
    AreaOfEffect, BlocksTile, BlocksVisibility, CausesConfusion, Confusion, DefenseBonus, Door, EquipmentSlot,
    Equippable, Equipped, Hidden, HungerClock, InBackpack, InflictsDamage, Item, Name, Poison, PoisonsOnHit,
    ProvidesHealing, Ranged, Targeting, Weapon,
};
use crate::cover::Cover;
use crate::economy::{Gold, Purse};
use crate::enemy_ai::{Alertness, Archetype};
use crate::entity_render::{EntityShape, Renderable};
use crate::experience::{Experience, XpValue};
use crate::game_flow::{KeepMode, Mode, RunState};
use crate::gamelog::GameLog;
use crate::levels::Delve;
use crate::lighting::LightSource;
use crate::map::{tile_walkable, Map, Position, Tile, TileType};
use crate::map_render::{spawn_map_tiles, TileReveal};
use crate::models::Model;
use crate::overwatch::{Hunkered, Overwatch};
use crate::party::{ActiveUnit, PartyMember, PARTY_SIZE};
use crate::raws::{spawn_class_abilities, spawn_named_item, spawn_named_mob, spawn_named_prop, RawMaster, RAWS};
use crate::spawner::party_member;
use crate::viewshed::Viewshed;

#[cfg(not(test))]
const SAVE_FILE: &str = "savegame.json";
// Tests drive the real death path; keep it away from a developer's save.
#[cfg(test)]
const SAVE_FILE: &str = "savegame.test-never-written.json";

/// Bump when a save from the last version can no longer be read as it is.
pub const SAVE_VERSION: u32 = 2;

// ============================================================================
// The save file
// ============================================================================

#[derive(Serialize, Deserialize)]
pub struct SaveData {
    pub version: u32,
    pub map: SerializedMap,
    /// In slot order.
    pub party: Vec<SavedHero>,
    /// The slot of the active hero.
    pub active: usize,
    pub mode: SavedMode,
    /// Mobs, bystanders, props and items, on the map or carried.
    pub things: Vec<SavedThing>,
    pub game_log: Vec<String>,
    pub gold: i32,
    pub delve: Delve,
}

#[derive(Serialize, Deserialize)]
pub struct SerializedMap {
    pub tiles: Vec<TileType>,
    pub revealed_tiles: Vec<bool>,
    pub width: i32,
    pub height: i32,
    pub depth: i32,
    pub bloodstains: Vec<usize>,
}

/// `Mode` with each hero by slot.
#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub enum SavedMode {
    Explore,
    Combat { round: u32, ap_spent: Vec<(usize, u8)> },
}

/// What wears off: on heroes and mobs alike.
#[derive(Serialize, Deserialize, Default)]
pub struct Status {
    pub poison: Option<Poison>,
    pub confusion: Option<i32>,
    pub overwatch: bool,
    pub hunkered: bool,
}

#[derive(Serialize, Deserialize)]
pub struct SavedHero {
    pub slot: usize,
    pub name: String,
    /// The raws class: its model and abilities are spawned afresh.
    pub class: String,
    pub x: i32,
    pub y: i32,
    pub stats: CombatStats,
    pub hunger: HungerClock,
    pub experience: Experience,
    pub status: Status,
    /// Rounds left on each ability, by ability slot.
    pub cooldowns: Vec<u8>,
}

#[derive(Serialize, Deserialize)]
pub enum Place {
    Ground { x: i32, y: i32 },
    /// In the backpack of the hero in `slot`.
    Backpack { slot: usize },
    /// Worn or wielded by the hero in `slot`.
    Worn { slot: usize, on: EquipmentSlot },
}

/// A mob, bystander, prop or item: its raws entry and what changed since.
#[derive(Serialize, Deserialize)]
pub struct SavedThing {
    pub raws: String,
    /// The name shown, e.g. "Goblin #3".
    pub name: String,
    pub place: Place,
    pub stats: Option<CombatStats>,
    pub alertness: Option<Alertness>,
    pub gold: Option<i32>,
    pub door_open: Option<bool>,
    pub hidden: bool,
    pub status: Status,
}

// ============================================================================
// Saving
// ============================================================================

/// Q: save the game (in a run with a hero alive) and exit.
pub fn save_on_quit(world: &mut World) {
    if !world.resource::<ButtonInput<KeyCode>>().just_released(KeyCode::KeyQ) {
        return;
    }
    if *world.resource::<State<RunState>>().get() != RunState::MainMenu {
        save_game(world);
    }
    world.write_message(AppExit::Success);
}

/// The game as it stands, or `None` without a party.
pub fn collect(world: &mut World) -> Option<SaveData> {
    let mut cooldowns: HashMap<Entity, Vec<(usize, u8)>> = HashMap::new();
    for (ability, parent) in world.query::<(&HeroAbility, &ChildOf)>().iter(world) {
        cooldowns.entry(parent.parent()).or_default().push((ability.slot, ability.left));
    }
    let mut slots = HashMap::new();
    let mut party: Vec<SavedHero> = world
        .query::<(
            Entity,
            &PartyMember,
            &Position,
            &Name,
            &CombatStats,
            &HungerClock,
            &HeroClass,
            &Experience,
            StatusQuery,
        )>()
        .iter(world)
        .map(|(hero, member, pos, name, stats, hunger, class, experience, status)| {
            slots.insert(hero, member.slot);
            let mut left = cooldowns.remove(&hero).unwrap_or_default();
            left.sort();
            SavedHero {
                slot: member.slot,
                name: name.name.clone(),
                class: class.0.clone(),
                x: pos.x,
                y: pos.y,
                stats: stats.clone(),
                hunger: *hunger,
                experience: *experience,
                status: saved_status(status),
                cooldowns: left.into_iter().map(|(_, left)| left).collect(),
            }
        })
        .collect();
    if party.is_empty() {
        return None;
    }
    party.sort_by_key(|hero| hero.slot);

    let raws = RAWS.lock().unwrap();
    let mut things = Vec::new();
    let mut query = world.query_filtered::<(
        &Name,
        Option<&Position>,
        Option<&InBackpack>,
        Option<&Equipped>,
        Option<&CombatStats>,
        Option<&Alertness>,
        Option<&Gold>,
        Option<&Door>,
        Has<Hidden>,
        StatusQuery,
    ), (Without<PartyMember>, Without<Tile>, Without<HeroAbility>)>();
    for (name, pos, backpack, worn, stats, alertness, gold, door, hidden, status) in query.iter(world) {
        let place = match (pos, backpack, worn) {
            (_, _, Some(worn)) => slots.get(&worn.owner).map(|&slot| Place::Worn { slot, on: worn.slot }),
            (_, Some(backpack), _) => slots.get(&backpack.owner).map(|&slot| Place::Backpack { slot }),
            (Some(pos), ..) => Some(Place::Ground { x: pos.x, y: pos.y }),
            _ => continue,
        };
        let (Some(place), Some(raws_name)) = (place, raws_name(&raws, &name.name)) else {
            warn!("Not saved: {}", name.name);
            continue;
        };
        things.push(SavedThing {
            raws: raws_name.to_string(),
            name: name.name.clone(),
            place,
            stats: stats.cloned(),
            alertness: alertness.copied(),
            gold: gold.map(|g| g.amount),
            door_open: door.map(|d| d.open),
            hidden,
            status: saved_status(status),
        });
    }

    let active = world.resource::<ActiveUnit>().0.and_then(|unit| slots.get(&unit).copied());
    let mode = match world.resource::<Mode>() {
        Mode::Explore => SavedMode::Explore,
        Mode::Combat { round, ap_spent } => {
            let mut spent: Vec<(usize, u8)> =
                ap_spent.iter().filter_map(|(unit, &ap)| slots.get(unit).map(|&slot| (slot, ap))).collect();
            spent.sort();
            SavedMode::Combat { round: *round, ap_spent: spent }
        }
    };
    let map = world.resource::<Map>();
    let mut bloodstains: Vec<usize> = map.bloodstains.iter().copied().collect();
    bloodstains.sort();
    Some(SaveData {
        version: SAVE_VERSION,
        map: SerializedMap {
            tiles: map.tiles.clone(),
            revealed_tiles: map.revealed_tiles.clone(),
            width: map.width,
            height: map.height,
            depth: map.depth,
            bloodstains,
        },
        active: active.unwrap_or(party[0].slot),
        party,
        mode,
        things,
        game_log: world.resource::<GameLog>().entries.clone(),
        gold: world.resource::<Purse>().gold,
        delve: world.resource::<Delve>().clone(),
    })
}

type StatusQuery = (Option<&'static Poison>, Option<&'static Confusion>, Has<Overwatch>, Has<Hunkered>);

fn saved_status((poison, confusion, overwatch, hunkered): (Option<&Poison>, Option<&Confusion>, bool, bool)) -> Status {
    Status { poison: poison.copied(), confusion: confusion.map(|c| c.turns), overwatch, hunkered }
}

/// The raws entry a thing was spawned from: its name, less a mob's ` #id`.
fn raws_name<'a>(raws: &RawMaster, name: &'a str) -> Option<&'a str> {
    let known = |n: &&str| raws.item_index.contains_key(*n) || raws.mob_index.contains_key(*n) || raws.prop_index.contains_key(*n);
    [name, name.split(" #").next().unwrap_or(name)].into_iter().find(known)
}

// ============================================================================
// Loading
// ============================================================================

/// The save in `json`, or why it cannot be loaded.
pub fn parse_save(json: &str) -> Result<SaveData, String> {
    let value: serde_json::Value = serde_json::from_str(json).map_err(|e| format!("it is not readable ({e})"))?;
    match value.get("version").and_then(|v| v.as_u64()) {
        Some(version) if version == u64::from(SAVE_VERSION) => {}
        Some(version) => return Err(format!("it is from save version {version}, this game reads {SAVE_VERSION}")),
        None => return Err("it is from an older version of the game".into()),
    }
    let save: SaveData = serde_json::from_value(value).map_err(|e| format!("it is damaged ({e})"))?;
    let raws = RAWS.lock().unwrap();
    if save.party.is_empty() {
        return Err("it has no party".into());
    }
    for hero in &save.party {
        if hero.slot >= PARTY_SIZE || raws.class(&hero.class).is_none() {
            return Err(format!("{} has no class {} in slot {}", hero.name, hero.class, hero.slot));
        }
    }
    if save.map.tiles.len() != (save.map.width * save.map.height) as usize
        || save.map.revealed_tiles.len() != save.map.tiles.len()
    {
        return Err("its map is damaged".into());
    }
    Ok(save)
}

/// Replace the game in `world` with `save`: every hero, thing on the map and
/// carried item goes, the saved ones are spawned from the raws and given
/// their saved state. The next `PreRun` keeps the saved mode.
pub fn restore(world: &mut World, save: SaveData) {
    let doomed: Vec<Entity> = world
        .query_filtered::<Entity, Or<(With<Position>, With<Item>, With<PartyMember>)>>()
        .iter(world)
        .collect();
    for entity in doomed {
        // Abilities go with their hero.
        if let Ok(entity) = world.get_entity_mut(entity) {
            entity.despawn();
        }
    }

    let mut map = world.resource_mut::<Map>();
    map.tiles = save.map.tiles;
    map.revealed_tiles = save.map.revealed_tiles;
    map.width = save.map.width;
    map.height = save.map.height;
    map.depth = save.map.depth;
    map.blocked_tiles = map.tiles.iter().map(|t| !tile_walkable(*t)).collect();
    map.visible_tiles = vec![false; map.tiles.len()];
    map.tile_content = vec![Vec::new(); map.tiles.len()];
    map.bloodstains = save.map.bloodstains.into_iter().collect();
    world.resource_scope(|world, map: Mut<Map>| {
        // The font is only used by the visualiser's glyphs.
        spawn_map_tiles(&mut world.commands(), &map, &TextFont::default(), TileReveal::FromSave);
    });

    let raws = RAWS.lock().unwrap();
    let mut commands = world.commands();
    let mut heroes = HashMap::new();
    for hero in &save.party {
        let class = raws.class(&hero.class).expect("parse_save checks every class");
        let entity = commands.spawn(party_member(hero.slot, hero.x, hero.y, class)).id();
        commands
            .entity(entity)
            .insert((Name { name: hero.name.clone() }, hero.stats.clone(), hero.hunger, hero.experience));
        apply_status(&mut commands.entity(entity), &hero.status);
        spawn_class_abilities(&raws, &mut commands, entity, class);
        heroes.insert(hero.slot, entity);
    }

    for thing in &save.things {
        let (x, y) = match thing.place {
            Place::Ground { x, y } => (x, y),
            _ => (0, 0),
        };
        let spawned = spawn_named_item(&raws, &mut commands, &thing.raws, x, y)
            .or_else(|| spawn_named_mob(&raws, &mut commands, &thing.raws, x, y, None))
            .or_else(|| spawn_named_prop(&raws, &mut commands, &thing.raws, x, y));
        let Some(entity) = spawned else {
            warn!("Not loaded: no raws entry {}", thing.raws);
            continue;
        };
        let mut entity = commands.entity(entity);
        entity.insert(Name { name: thing.name.clone() });
        match thing.place {
            Place::Ground { .. } => {}
            Place::Backpack { slot } | Place::Worn { slot, .. } => {
                let Some(&owner) = heroes.get(&slot) else {
                    warn!("Not loaded: {} belongs to no hero in slot {slot}", thing.name);
                    entity.despawn();
                    continue;
                };
                entity.remove::<Position>().insert(Visibility::Hidden);
                match thing.place {
                    Place::Worn { on, .. } => entity.insert(Equipped { owner, slot: on }),
                    _ => entity.insert(InBackpack { owner }),
                };
            }
        }
        if let Some(stats) = &thing.stats {
            entity.insert(stats.clone());
        }
        if let Some(alertness) = thing.alertness {
            entity.insert(alertness);
        }
        match thing.gold {
            Some(amount) => entity.insert(Gold { amount }),
            None => entity.remove::<Gold>(),
        };
        match thing.hidden {
            true => entity.insert(Hidden),
            false => entity.remove::<Hidden>(),
        };
        apply_status(&mut entity, &thing.status);
        if thing.door_open == Some(true) {
            // As `player::try_move` opens one.
            entity.insert((Door { open: true }, EntityShape::OpenDoor)).remove::<(BlocksTile, BlocksVisibility)>();
        }
    }
    drop(raws);
    world.flush();
    let mut abilities = world.query::<(&mut HeroAbility, &ChildOf)>();
    for (mut ability, parent) in abilities.iter_mut(world) {
        let hero = save.party.iter().find(|h| heroes.get(&h.slot) == Some(&parent.parent()));
        if let Some(&left) = hero.and_then(|h| h.cooldowns.get(ability.slot)) {
            ability.left = left;
        }
    }

    world.resource_mut::<GameLog>().entries = save.game_log;
    world.insert_resource(Purse { gold: save.gold });
    world.insert_resource(save.delve);
    let active = heroes.get(&save.active).copied().or_else(|| heroes.values().next().copied());
    world.insert_resource(ActiveUnit(active));
    world.insert_resource(match save.mode {
        SavedMode::Explore => Mode::Explore,
        SavedMode::Combat { round, ap_spent } => Mode::Combat {
            round,
            ap_spent: ap_spent.into_iter().filter_map(|(slot, ap)| Some((*heroes.get(&slot)?, ap))).collect(),
        },
    });
    world.insert_resource(KeepMode(true));
}

fn apply_status(entity: &mut EntityCommands, status: &Status) {
    entity.remove::<(Poison, Confusion, Overwatch, Hunkered)>();
    if let Some(poison) = status.poison {
        entity.insert(poison);
    }
    if let Some(turns) = status.confusion {
        entity.insert(Confusion { turns });
    }
    if status.overwatch {
        entity.insert(Overwatch);
    }
    if status.hunkered {
        entity.insert(Hunkered);
    }
}

// ============================================================================
// The file
// ============================================================================

pub fn save_game(world: &mut World) {
    let Some(save) = collect(world) else {
        warn!("Cannot save: no party found");
        return;
    };
    match serde_json::to_string_pretty(&save) {
        Ok(json) => match fs::write(SAVE_FILE, json) {
            Ok(()) => info!("Game saved to {SAVE_FILE}"),
            Err(e) => error!("Failed to write save file: {e}"),
        },
        Err(e) => error!("Failed to serialise save data: {e}"),
    }
}

pub fn has_save_file() -> bool {
    Path::new(SAVE_FILE).exists()
}

pub fn delete_save_file() {
    let _ = fs::remove_file(SAVE_FILE);
}

/// For the main menu: whether a save can be continued. One that cannot be
/// loaded is moved aside, and `Err` says so in words for the player.
pub fn save_status() -> Result<bool, String> {
    let Ok(json) = fs::read_to_string(SAVE_FILE) else {
        return Ok(false);
    };
    parse_save(&json).map(|_| true).map_err(|problem| discard(&problem))
}

/// Moves the save aside and returns the report.
fn discard(problem: &str) -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let aside = format!("{SAVE_FILE}.bad-{secs}");
    match fs::rename(SAVE_FILE, &aside) {
        Ok(()) => {
            warn!("Could not load {SAVE_FILE} ({problem}); moved it to {aside}");
            format!("The saved game could not be loaded: {problem}. It was moved to {aside}.")
        }
        Err(e) => {
            error!("Could not load {SAVE_FILE} ({problem}) nor move it aside ({e}); deleting it");
            delete_save_file();
            format!("The saved game could not be loaded: {problem}. It was deleted.")
        }
    }
}

/// Load the save into `world`, then delete it (permadeath). A save that
/// cannot be loaded is moved aside and reported in the game log.
pub fn load_game(world: &mut World) -> bool {
    let Ok(json) = fs::read_to_string(SAVE_FILE) else {
        warn!("No save file found");
        return false;
    };
    match parse_save(&json) {
        Ok(save) => {
            restore(world, save);
            delete_save_file();
            info!("Game loaded");
            true
        }
        Err(problem) => {
            let report = discard(&problem);
            world.resource_mut::<GameLog>().entries.push(report);
            false
        }
    }
}

/// Continue from the main menu: load, then start play.
pub fn continue_game(world: &mut World) {
    if load_game(world) {
        world.resource_mut::<NextState<RunState>>().set(RunState::PreRun);
    }
}

// ============================================================================
// Snapshot
// ============================================================================

/// The game in `world` as sorted lines of plain text, to compare a saved
/// world with the one loaded from it. Every named entity (tiles aside) lists
/// the gameplay values below and the type of every component it has, so a
/// component the save forgets shows up as a difference even when it is not
/// in the list. Entities are named by what they are, never by `Entity` id.
pub fn snapshot(world: &mut World) -> Vec<String> {
    let mut names = HashMap::new();
    for (entity, name) in world.query::<(Entity, &Name)>().iter(world) {
        names.insert(entity, name.name.clone());
    }
    let name_of = |entity: Entity| names.get(&entity).cloned().unwrap_or_else(|| "?".into());
    let entities: Vec<Entity> =
        world.query_filtered::<Entity, (With<Name>, Without<Tile>)>().iter(world).collect();

    // Kept up by the renderer frame by frame, so a body just loaded lacks them for a frame.
    let render_bookkeeping = [
        std::any::TypeId::of::<bevy::camera::primitives::Aabb>(),
        std::any::TypeId::of::<bevy::pbr::PreviousGlobalTransform>(),
        std::any::TypeId::of::<InheritedVisibility>(),
        std::any::TypeId::of::<ViewVisibility>(),
        // Which way a body last turned: not saved, a loaded body faces south.
        std::any::TypeId::of::<crate::presentation::Facing>(),
    ];
    let mut lines = Vec::new();
    for entity in entities {
        let mut fields = Vec::new();
        macro_rules! values {
            ($($t:ty),*) => { $( if let Some(c) = world.get::<$t>(entity) { fields.push(format!("{c:?}")); } )* };
        }
        values!(
            Position, CombatStats, HungerClock, Experience, XpValue, HeroClass, PartyMember, Confusion, Poison,
            PoisonsOnHit, Gold, Weapon, Archetype, Alertness, Door, Cover, Renderable, EntityShape, Model,
            Equippable, DefenseBonus, InflictsDamage, ProvidesHealing, Ranged, AreaOfEffect, Targeting,
            CausesConfusion, LightSource
        );
        if let Some(viewshed) = world.get::<Viewshed>(entity) {
            fields.push(format!("sees {}", viewshed.range));
        }
        // Its effect holds a HashMap, whose order differs between copies.
        if let Some(ability) = world.get::<HeroAbility>(entity) {
            fields.push(format!("ability {} cooldown {} left {}", ability.slot, ability.cooldown, ability.left));
        }
        let mut owner = None;
        if let Some(worn) = world.get::<Equipped>(entity) {
            owner = Some(worn.owner);
            fields.push(format!("worn on {:?}", worn.slot));
        }
        if let Some(backpack) = world.get::<InBackpack>(entity) {
            owner = Some(backpack.owner);
            fields.push("in backpack".into());
        }
        if let Some(parent) = world.get::<ChildOf>(entity) {
            owner = Some(parent.parent());
        }
        let mut components: Vec<String> = world
            .inspect_entity(entity)
            .map(|infos| {
                infos
                    .filter(|(_, info)| !info.type_id().is_some_and(|id| render_bookkeeping.contains(&id)))
                    .map(|(_, info)| {
                        // Names need Bevy's `debug` feature; without it the type id tells them apart.
                        let name = info.name().to_string();
                        if name.starts_with('<') { format!("{:?}", info.type_id()) } else { name }
                    })
                    .collect()
            })
            .unwrap_or_default();
        components.sort();
        let who = match owner {
            Some(owner) => format!("{}'s {}", name_of(owner), name_of(entity)),
            None => name_of(entity),
        };
        lines.push(format!("{who}: {} | components {}", fields.join(", "), components.join(", ")));
    }

    let map = world.resource::<Map>();
    let mut stains: Vec<_> = map.bloodstains.iter().collect();
    stains.sort();
    lines.push(format!(
        "map depth {} {}x{} tiles {:?} revealed {:?} stains {stains:?}",
        map.depth, map.width, map.height, map.tiles, map.revealed_tiles
    ));
    let mode = match world.resource::<Mode>() {
        Mode::Explore => "explore".to_string(),
        Mode::Combat { round, ap_spent } => {
            let mut spent: Vec<String> = ap_spent.iter().map(|(unit, ap)| format!("{} {ap}", name_of(*unit))).collect();
            spent.sort();
            format!("combat round {round}, AP spent {spent:?}")
        }
    };
    lines.push(format!("mode {mode}"));
    lines.push(format!("active {:?}", world.resource::<ActiveUnit>().0.map(name_of)));
    lines.push(format!("purse {}, {:?}", world.resource::<Purse>().gold, world.resource::<Delve>()));
    lines.push(format!("log {:?}", world.resource::<GameLog>().entries));
    lines.sort();
    lines
}

/// The first line where `a` and `b` differ: its start and the parts only in
/// one of them, "saved only [..] loaded only [..]".
pub fn first_difference(a: &[String], b: &[String]) -> Option<String> {
    let missing = String::from("(nothing)");
    (0..a.len().max(b.len())).find_map(|i| {
        let (x, y) = (a.get(i).unwrap_or(&missing), b.get(i).unwrap_or(&missing));
        let parts = |line: &'_ String| line.split(", ").map(String::from).collect::<Vec<_>>();
        let (xs, ys) = (parts(x), parts(y));
        let only = |these: &[String], those: &[String]| {
            these.iter().filter(|p| !those.contains(p)).cloned().collect::<Vec<_>>()
        };
        let start: String = x.chars().take(60).collect();
        (x != y).then(|| format!("{start}... saved only {:?} loaded only {:?}", only(&xs, &ys), only(&ys, &xs)))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::raws::load_raws;

    /// A world with every resource a save reads and a load writes.
    fn empty_world() -> World {
        let mut world = World::new();
        world.insert_resource(Map::new(6, 5, 3));
        world.insert_resource(GameLog { entries: Vec::new() });
        world.insert_resource(Purse::default());
        world.insert_resource(Delve::default());
        world.insert_resource(Mode::Explore);
        world.insert_resource(ActiveUnit(None));
        world.insert_resource(KeepMode(false));
        world
    }

    /// Mid-combat in round 2: Corin has spent an AP and is active, Aldric is
    /// poisoned on overwatch with Cleave cooling down and a bought Longsword
    /// in hand, an alerted Goblin Archer carries gold and is confused, a door
    /// is open, a trap was found and a potion lies on the floor.
    fn mid_combat() -> World {
        load_raws();
        let mut world = empty_world();
        {
            let mut map = world.resource_mut::<Map>();
            for (i, tile) in map.tiles.iter_mut().enumerate() {
                *tile = if i % 7 == 0 { TileType::Wall } else { TileType::Floor };
            }
            map.revealed_tiles[3] = true;
            map.bloodstains.insert(8);
            map.tiles[9] = TileType::UpStairs;
        }
        let party = crate::spawner::spawn_party(&mut world.commands(), &[(1, 1), (2, 1), (3, 1), (4, 1)]);
        let raws = RAWS.lock().unwrap();
        let mut commands = world.commands();
        let archer = spawn_named_mob(&raws, &mut commands, "Goblin Archer", 4, 3, Some(7)).unwrap();
        commands.entity(archer).insert((
            Alertness::Alerted { last_seen: Some((1, 1)), unseen: 1 },
            Gold { amount: 9 },
            Confusion { turns: 2 },
            Hunkered,
        ));
        let door = spawn_named_prop(&raws, &mut commands, "Door", 2, 3).unwrap();
        let trap = spawn_named_prop(&raws, &mut commands, "Bear Trap", 3, 3).unwrap();
        commands.entity(trap).remove::<Hidden>();
        spawn_named_item(&raws, &mut commands, "Health Potion", 1, 2).unwrap();
        let sword = spawn_named_item(&raws, &mut commands, "Longsword", 0, 0).unwrap();
        commands.entity(sword).remove::<Position>().insert(Equipped { owner: party[0], slot: EquipmentSlot::Melee });
        let heart = spawn_named_item(&raws, &mut commands, crate::levels::HEART_OF_THE_ABYSS, 0, 0).unwrap();
        commands.entity(heart).remove::<Position>().insert(InBackpack { owner: party[3] });
        commands.entity(party[0]).insert((Poison { damage: 2, turns: 3 }, Overwatch));
        drop(raws);
        world.flush();

        let mut abilities = world.query::<(&mut HeroAbility, &ChildOf)>();
        for (mut ability, parent) in abilities.iter_mut(&mut world) {
            if parent.parent() == party[0] && ability.slot == 0 {
                ability.left = 2;
            }
        }
        world.get_mut::<CombatStats>(party[1]).unwrap().hp -= 4;
        world.get_mut::<Door>(door).unwrap().open = true;
        world.entity_mut(door).remove::<(BlocksTile, BlocksVisibility)>().insert(EntityShape::OpenDoor);
        world.insert_resource(Mode::Combat { round: 2, ap_spent: [(party[2], 1)].into() });
        world.insert_resource(ActiveUnit(Some(party[2])));
        world.insert_resource(Purse { gold: 55 });
        world.insert_resource(Delve { deepest: 3, levels_visited: 2, levels_entered: 5 });
        world.resource_mut::<GameLog>().entries.push("Aldric goes on overwatch.".into());
        world
    }

    #[test]
    fn a_game_saved_mid_combat_loads_to_the_same_state() {
        let mut saved = mid_combat();
        let json = serde_json::to_string(&collect(&mut saved).unwrap()).unwrap();
        let mut loaded = empty_world();
        restore(&mut loaded, parse_save(&json).unwrap());

        let (before, after) = (snapshot(&mut saved), snapshot(&mut loaded));
        assert_eq!(first_difference(&before, &after), None);
        assert!(loaded.resource::<KeepMode>().0, "the next PreRun keeps the combat");
        // The snapshot covers what this test set up.
        let all = before.join("\n");
        for wanted in [
            "mode combat round 2, AP spent [\"Corin 1\"]",
            "active Some(\"Corin\")",
            "Aldric's Longsword: ",
            "Aldric's Cleave: ability 0 cooldown 3 left 2",
            "Poison { damage: 2, turns: 3 }",
            "Alerted { last_seen: Some((1, 1)), unseen: 1 }",
            "Gold { amount: 9 }",
            "Door { open: true }",
            "Goblin Archer #7",
            "Heart of the Abyss",
            "UpStairs",
            "levels_entered: 5",
        ] {
            assert!(all.contains(wanted), "{wanted} missing from the snapshot:\n{all}");
        }
    }

    #[test]
    fn the_snapshot_notices_a_lost_component() {
        let mut saved = mid_combat();
        let json = serde_json::to_string(&collect(&mut saved).unwrap()).unwrap();
        let mut loaded = empty_world();
        restore(&mut loaded, parse_save(&json).unwrap());
        let archer = loaded.query::<(Entity, &Name)>().iter(&loaded).find(|(_, n)| n.name == "Goblin Archer #7").unwrap().0;
        // A marker the values list does not name.
        loaded.entity_mut(archer).remove::<crate::monsters::Monster>();
        assert!(first_difference(&snapshot(&mut saved), &snapshot(&mut loaded)).is_some());
    }

    #[test]
    fn a_save_from_another_version_or_damaged_is_refused() {
        let mut saved = mid_combat();
        let mut json = serde_json::to_value(collect(&mut saved).unwrap()).unwrap();
        json["version"] = (SAVE_VERSION + 1).into();
        assert_eq!(
            parse_save(&json.to_string()).err(),
            Some(format!("it is from save version {}, this game reads {SAVE_VERSION}", SAVE_VERSION + 1))
        );
        json.as_object_mut().unwrap().remove("version");
        assert_eq!(parse_save(&json.to_string()).err(), Some("it is from an older version of the game".into()));
        assert!(parse_save("{ not json").err().unwrap().starts_with("it is not readable"));
        json["version"] = SAVE_VERSION.into();
        json["party"][0]["class"] = "Juggler".into();
        assert!(parse_save(&json.to_string()).err().unwrap().contains("no class Juggler"));
    }
}
