//! Level lifecycle: starting a new game and descending to the next depth.

use bevy::prelude::*;

use crate::combat::CombatStats;
use crate::components::{EntryTrigger, Item};
use crate::economy::town::TownState;
use crate::economy::Purse;
use crate::game_flow::RunState;
use crate::gamelog::GameLog;
use crate::map::{Map, Position, Tile, FONT_SIZE};
use crate::map_builders;
use crate::map_render::{spawn_map_tiles, TileReveal};
use crate::monsters::Monster;
use crate::party::{self, ActiveUnit, PartyMember};
use crate::resources::UiFont;
use crate::rng::GameRng;
use crate::spawner;
use crate::viewshed::Viewshed;

/// Build the starting map and spawn the party plus initial entities.
/// Called from the main menu when starting a new game (depth 1), and by the
/// screenshot harness at any depth.
pub fn spawn_new_game(
    commands: &mut Commands,
    map: &mut Map,
    rng: &mut GameRng,
    font: &UiFont,
    town: &TownState,
    depth: i32,
) {
    // Depth 1 is the town; deeper levels get procedural dungeons
    let mut builder = map_builders::level_builder(depth, rng, town);
    builder.build_map(rng);
    *map = builder.get_map();

    let text_font = TextFont {
        font: font.0.clone().into(),
        font_size: FontSize::Px(FONT_SIZE),
        ..default()
    };

    spawn_map_tiles(commands, map, &text_font, TileReveal::Hidden);

    let tiles = party::spawn_tiles(map, builder.get_starting_position(), party::PARTY_SIZE);
    let party = spawner::spawn_party(commands, &tiles);
    commands.insert_resource(ActiveUnit(party.first().copied()));
    commands.insert_resource(Purse::default());

    builder.spawn_entities(commands, rng);
}

/// Tear down the current level and build the next one, keeping the party
/// and whatever it carries.
pub fn go_next_level(
    mut commands: Commands,
    mut map: ResMut<Map>,
    mut gamelog: ResMut<GameLog>,
    mut next_state: ResMut<NextState<RunState>>,
    font: Res<UiFont>,
    mut rng: ResMut<GameRng>,
    town: Res<TownState>,
    mut party_query: Query<(Entity, &PartyMember, &mut CombatStats, &mut Viewshed)>,
    // Items on the ground go with the level; carried ones have no `Position`.
    entities_to_delete: Query<
        Entity,
        Or<(With<Monster>, With<Tile>, (With<Item>, With<Position>), With<EntryTrigger>)>,
    >,
) {
    for entity in &entities_to_delete {
        commands.entity(entity).despawn();
    }

    // Generate new map with increased depth using level-appropriate builder
    let new_depth = map.depth + 1;
    let mut builder = map_builders::level_builder(new_depth, &mut rng, &town);
    builder.build_map(&mut rng);
    *map = builder.get_map();

    let text_font = TextFont {
        font: font.0.clone().into(),
        font_size: FontSize::Px(FONT_SIZE),
        ..default()
    };

    spawn_map_tiles(&mut commands, &map, &text_font, TileReveal::Hidden);

    // Spawn monsters and items via builder
    builder.spawn_entities(&mut commands, &mut rng);

    // Stand the party round the start, in slot order, each healed by half
    let mut members: Vec<_> = party_query.iter_mut().collect();
    members.sort_by_key(|(_, member, _, _)| member.slot);
    let tiles = party::spawn_tiles(&map, builder.get_starting_position(), members.len());
    for ((entity, _, mut stats, mut viewshed), (x, y)) in members.into_iter().zip(tiles) {
        commands.entity(entity).insert(Position { x, y });
        stats.hp = (stats.hp + stats.max_hp / 2).min(stats.max_hp);
        viewshed.visible_tiles.clear();
        viewshed.dirty = true;
    }

    gamelog.entries.push(format!(
        "You descend to level {}. You feel slightly rejuvenated.",
        new_depth
    ));

    next_state.set_if_different(RunState::PreRun);
}
