//! Level lifecycle: starting a new game and descending to the next depth.

use bevy::prelude::*;

use crate::combat::CombatStats;
use crate::components::{EntryTrigger, InBackpack, Item};
use crate::game_flow::RunState;
use crate::gamelog::GameLog;
use crate::map::{Map, Position, Tile, FONT_SIZE};
use crate::map_builders;
use crate::map_render::{spawn_map_tiles, TileReveal};
use crate::monsters::Monster;
use crate::player::Player;
use crate::resources::UiFont;
use crate::rng::GameRng;
use crate::spawner;
use crate::viewshed::Viewshed;

/// Build the starting map and spawn the player plus initial entities.
/// Called from the main menu when starting a new game.
pub fn spawn_new_game(
    commands: &mut Commands,
    map: &mut Map,
    rng: &mut GameRng,
    font: &UiFont,
) {
    // Depth 1 is the town; deeper levels get procedural dungeons
    let mut builder = map_builders::level_builder(1, rng);
    builder.build_map(rng);
    *map = builder.get_map();

    let text_font = TextFont {
        font: font.0.clone(),
        font_size: FONT_SIZE,
        ..default()
    };

    spawn_map_tiles(commands, map, &text_font, TileReveal::Hidden);

    let (player_x, player_y) = builder.get_starting_position();
    spawner::spawn_player(commands, &text_font, player_x, player_y);

    builder.spawn_entities(commands, rng, &text_font);
}

/// Tear down the current level and build the next one, keeping the player
/// and whatever they carry.
pub fn go_next_level(
    mut commands: Commands,
    mut map: ResMut<Map>,
    mut gamelog: ResMut<GameLog>,
    mut next_state: ResMut<NextState<RunState>>,
    font: Res<UiFont>,
    mut rng: ResMut<GameRng>,
    mut player_query: Query<(Entity, &mut CombatStats), With<Player>>,
    backpack_query: Query<(Entity, &InBackpack)>,
    entities_to_delete: Query<
        Entity,
        Or<(With<Monster>, With<Tile>, With<Item>, With<EntryTrigger>)>,
    >,
) {
    // Get player entity and items in their backpack
    let Ok((player_entity, mut player_stats)) = player_query.get_single_mut() else {
        return;
    };
    let player_items: Vec<Entity> = backpack_query
        .iter()
        .filter(|(_, backpack)| backpack.owner == player_entity)
        .map(|(entity, _)| entity)
        .collect();

    // Delete all entities except player and their backpack items
    for entity in &entities_to_delete {
        if entity != player_entity && !player_items.contains(&entity) {
            commands.entity(entity).despawn_recursive();
        }
    }

    // Generate new map with increased depth using level-appropriate builder
    let new_depth = map.depth + 1;
    let mut builder = map_builders::level_builder(new_depth, &mut rng);
    builder.build_map(&mut rng);
    *map = builder.get_map();

    let text_font = TextFont {
        font: font.0.clone(),
        font_size: FONT_SIZE,
        ..default()
    };

    spawn_map_tiles(&mut commands, &map, &text_font, TileReveal::Hidden);

    // Spawn monsters and items via builder
    builder.spawn_entities(&mut commands, &mut rng, &text_font);

    // Move player to starting position
    let (player_x, player_y) = builder.get_starting_position();
    commands.entity(player_entity).insert(Position {
        x: player_x,
        y: player_y,
    });

    // Heal player (restore up to 50% of max HP)
    let heal_amount = player_stats.max_hp / 2;
    player_stats.hp = (player_stats.hp + heal_amount).min(player_stats.max_hp);

    // Mark player's viewshed as dirty to recalculate visibility
    commands.entity(player_entity).insert(Viewshed {
        range: 8,
        visible_tiles: Vec::new(),
        dirty: true,
    });

    gamelog.entries.push(format!(
        "You descend to level {}. You feel slightly rejuvenated.",
        new_depth
    ));

    next_state.set(RunState::PreRun);
}
