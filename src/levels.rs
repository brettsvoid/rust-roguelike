//! Level lifecycle: starting a new game, taking the stairs down, and going
//! back to town by the up stairs on depth 1 or a Town Portal scroll.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::combat::CombatStats;
use crate::components::{Equipped, InBackpack, Name, TownPortal, WantsToUseItem};
use crate::economy::stock::restock;
use crate::economy::town::TownState;
use crate::economy::{trade, Purse};
use crate::game_flow::RunState;
use crate::gamelog::GameLog;
use crate::map::{Map, Position, TileType, FONT_SIZE};
use crate::map_builders;
use crate::map_render::{spawn_map_tiles, TileReveal};
use crate::party::{self, ActiveUnit, PartyMember};
use crate::resources::UiFont;
use crate::rng::GameRng;
use crate::spawner;
use crate::viewshed::Viewshed;

/// The town's depth. The dungeon runs from depth 1 to 12 below it.
pub const TOWN_DEPTH: i32 = 0;

/// The prize on depth 12. A party carrying it must climb out on foot.
pub const HEART_OF_THE_ABYSS: &str = "Heart of the Abyss";

/// Where a level change leads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Travel {
    /// Down stairs: one level deeper, or from town to the deepest depth reached.
    Descend,
    /// The up stairs on depth 1 or a Town Portal.
    ToTown,
}

/// How far this run has delved, and how long the party has been away from
/// town. Saved with the game.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Delve {
    /// The deepest dungeon depth the party has reached; 0 before the first descent.
    pub deepest: i32,
    /// Dungeon levels entered since the party last left town.
    pub levels_visited: u32,
}

impl Delve {
    /// A new game starting at `depth`.
    pub fn starting_at(depth: i32) -> Self {
        let mut delve = Delve::default();
        delve.arrive(depth);
        delve
    }

    /// The depth `travel` from `depth` leads to. Leaving town goes back to
    /// the deepest depth reached, never above depth 1.
    pub fn destination(&self, depth: i32, travel: Travel) -> i32 {
        match travel {
            Travel::ToTown => TOWN_DEPTH,
            Travel::Descend if depth == TOWN_DEPTH => self.deepest.max(1),
            Travel::Descend => depth + 1,
        }
    }

    /// Record arriving at `depth`. Arriving in town returns the days spent
    /// away: one day per dungeon level entered since leaving.
    pub fn arrive(&mut self, depth: i32) -> u32 {
        if depth == TOWN_DEPTH {
            return std::mem::take(&mut self.levels_visited);
        }
        self.deepest = self.deepest.max(depth);
        self.levels_visited += 1;
        0
    }
}

/// Why a Town Portal cannot be read here, or `None` when it can.
/// `carried` is every item name the party carries or wears.
pub fn portal_refusal<'a>(depth: i32, mut carried: impl Iterator<Item = &'a str>) -> Option<&'static str> {
    if depth == TOWN_DEPTH {
        Some("You are already in town.")
    } else if carried.any(|name| name == HEART_OF_THE_ABYSS) {
        Some("The Heart of the Abyss smothers the portal. You must climb out.")
    } else {
        None
    }
}

/// Set when a Town Portal is read; `after_party_action` then changes level.
#[derive(Resource, Default)]
pub struct PendingPortal(pub bool);

/// Build the level at `depth` and spawn its entities. Depth 1 gets up
/// stairs at the start. Returns where the party should stand.
fn build_level(commands: &mut Commands, map: &mut Map, rng: &mut GameRng, font: &UiFont, town: &TownState, depth: i32) -> (i32, i32) {
    let mut builder = map_builders::level_builder(depth, rng, town);
    builder.build_map(rng);
    *map = builder.get_map();
    let start = builder.get_starting_position();
    if depth == 1 {
        let idx = map.xy_idx(start.0, start.1);
        map.tiles[idx] = TileType::UpStairs;
    }

    let text_font = TextFont {
        font: font.0.clone().into(),
        font_size: FontSize::Px(FONT_SIZE),
        ..default()
    };
    spawn_map_tiles(commands, map, &text_font, TileReveal::Hidden);
    builder.spawn_entities(commands, rng);
    start
}

/// Build the starting map and spawn the party plus initial entities.
/// Called from the main menu when starting a new game (in town), and by the
/// screenshot harness at any depth. Starting in town refills the vendors'
/// shelves.
pub fn spawn_new_game(
    commands: &mut Commands,
    map: &mut Map,
    rng: &mut GameRng,
    font: &UiFont,
    town: &mut TownState,
    depth: i32,
) {
    if depth == TOWN_DEPTH {
        restock(town, &crate::raws::RAWS.lock().unwrap().raws.items);
    }
    let start = build_level(commands, map, rng, font, town, depth);
    let tiles = party::spawn_tiles(map, start, party::PARTY_SIZE);
    let party = spawner::spawn_party(commands, &tiles);
    commands.insert_resource(ActiveUnit(party.first().copied()));
    commands.insert_resource(Purse::default());
    commands.insert_resource(Delve::starting_at(depth));
}

/// Read a Town Portal: refuse in town or while carrying the Heart (the
/// scroll is kept), otherwise flag the trip home. Runs before
/// `item_use_system`, which then uses up the scroll.
pub fn use_town_portal(
    mut commands: Commands,
    mut gamelog: ResMut<GameLog>,
    mut portal: ResMut<PendingPortal>,
    map: Res<Map>,
    uses: Query<(Entity, &WantsToUseItem)>,
    portals: Query<(), With<TownPortal>>,
    carried: Query<&Name, Or<(With<InBackpack>, With<Equipped>)>>,
) {
    for (user, wants) in &uses {
        if portals.get(wants.item).is_err() {
            continue;
        }
        match portal_refusal(map.depth, carried.iter().map(|n| n.name.as_str())) {
            Some(why) => {
                gamelog.entries.push(why.to_string());
                commands.entity(user).remove::<WantsToUseItem>();
            }
            None => {
                gamelog.entries.push("A shimmering portal opens and pulls the party home.".to_string());
                portal.0 = true;
            }
        }
    }
}

/// Tear down the current level and build the next one, keeping the party
/// and whatever it carries. A return to town runs the trade simulation for
/// the days away and restocks the vendors.
pub fn change_level(
    mut commands: Commands,
    mut map: ResMut<Map>,
    mut gamelog: ResMut<GameLog>,
    mut next_state: ResMut<NextState<RunState>>,
    font: Res<UiFont>,
    mut rng: ResMut<GameRng>,
    mut town: ResMut<TownState>,
    mut delve: ResMut<Delve>,
    mut portal: ResMut<PendingPortal>,
    active: Res<ActiveUnit>,
    mut party_query: Query<(Entity, &PartyMember, &mut CombatStats, &mut Viewshed)>,
    // Everything placed on the map goes with the level: tiles, monsters,
    // props, doors, traps, bystanders, vendors and items on the ground.
    // Carried items have no `Position`.
    level_bound: Query<(Entity, &Position), Without<PartyMember>>,
    positions: Query<&Position, With<PartyMember>>,
) {
    let on_up_stairs = active
        .0
        .and_then(|unit| positions.get(unit).ok())
        .is_some_and(|p| map.tiles[map.xy_idx(p.x, p.y)] == TileType::UpStairs);
    let travel = if std::mem::take(&mut portal.0) || on_up_stairs { Travel::ToTown } else { Travel::Descend };
    let depth = delve.destination(map.depth, travel);

    for (entity, _) in &level_bound {
        commands.entity(entity).despawn();
    }

    let days = delve.arrive(depth);
    if depth == TOWN_DEPTH && days > 0 {
        let news = trade::simulate(&mut town, days);
        restock(&mut town, &crate::raws::RAWS.lock().unwrap().raws.items);
        gamelog.entries.push(format!(
            "You return to town after {days} day{} away. The ledger (T) has {} new entries.",
            if days == 1 { "" } else { "s" },
            news.len()
        ));
    }

    let start = build_level(&mut commands, &mut map, &mut rng, &font, &town, depth);

    // Stand the party round the start, in slot order, each healed by half
    let mut members: Vec<_> = party_query.iter_mut().collect();
    members.sort_by_key(|(_, member, _, _)| member.slot);
    let tiles = party::spawn_tiles(&map, start, members.len());
    for ((entity, _, mut stats, mut viewshed), (x, y)) in members.into_iter().zip(tiles) {
        commands.entity(entity).insert(Position { x, y });
        stats.hp = (stats.hp + stats.max_hp / 2).min(stats.max_hp);
        viewshed.visible_tiles.clear();
        viewshed.dirty = true;
    }

    if depth != TOWN_DEPTH {
        gamelog.entries.push(format!("You descend to level {depth}. You feel slightly rejuvenated."));
    }

    next_state.set_if_different(RunState::PreRun);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_delve_tracks_the_deepest_depth_and_the_days_away() {
        let mut delve = Delve::starting_at(TOWN_DEPTH);
        assert_eq!(delve.destination(TOWN_DEPTH, Travel::Descend), 1);
        for depth in 1..=3 {
            assert_eq!(delve.arrive(depth), 0);
        }
        assert_eq!(delve.destination(3, Travel::Descend), 4);
        assert_eq!(delve.destination(3, Travel::ToTown), TOWN_DEPTH);
        // One day per dungeon level entered.
        assert_eq!(delve.arrive(TOWN_DEPTH), 3);
        assert_eq!(delve, Delve { deepest: 3, levels_visited: 0 });
        // Leaving town goes back to the deepest depth; the next trip counts afresh.
        assert_eq!(delve.destination(TOWN_DEPTH, Travel::Descend), 3);
        delve.arrive(3);
        assert_eq!(delve.arrive(TOWN_DEPTH), 1);
        // A game started deep (the harness) counts that level.
        assert_eq!(Delve::starting_at(5), Delve { deepest: 5, levels_visited: 1 });
    }

    /// The start must be placed before unreachable floor is culled, or a
    /// wall at the map centre culls the whole level and leaves no stairs.
    #[test]
    fn dungeon_levels_have_down_stairs() {
        let town = TownState::fresh(1);
        let stairless: Vec<u64> = (0..100)
            .filter(|&seed| {
                let mut rng = GameRng::seeded(seed);
                let mut builder = map_builders::level_builder(3, &mut rng, &town);
                builder.build_map(&mut rng);
                !builder.get_map().tiles.contains(&TileType::DownStairs)
            })
            .collect();
        assert_eq!(stairless, Vec::<u64>::new());
    }

    #[test]
    fn the_heart_blocks_a_town_portal() {
        let refusal = |depth, carried: &[&str]| portal_refusal(depth, carried.iter().copied());
        assert_eq!(refusal(4, &["Dagger", "Town Portal"]), None);
        assert_eq!(
            refusal(12, &["Town Portal", HEART_OF_THE_ABYSS]),
            Some("The Heart of the Abyss smothers the portal. You must climb out.")
        );
        assert_eq!(refusal(TOWN_DEPTH, &["Town Portal"]), Some("You are already in town."));
    }
}

