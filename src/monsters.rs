use bevy::prelude::*;

use crate::{
    combat::{CombatStats, WantsToMelee},
    components::{Confusion, Name},
    distance::DistanceAlg,
    gamelog::GameLog,
    map::{Map, Position, TileType, FONT_SIZE, MAP_HEIGHT, MAP_WIDTH},
    pathfinding,
    player::Player,
    resources::UiFont,
    rng::GameRng,
    spawner,
    viewshed::Viewshed,
    RunState,
};

#[derive(Component, Debug)]
pub struct Monster;

pub struct MonstersPlugin;
impl Plugin for MonstersPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_monsters).add_systems(
            Update,
            update_blocked_tiles.run_if(in_state(RunState::MonsterTurn)),
        );
    }
}

fn setup_monsters(mut commands: Commands, font: Res<UiFont>, map: Res<Map>, mut rng: ResMut<GameRng>) {
    let text_font = TextFont {
        font: font.0.clone(),
        font_size: FONT_SIZE,
        ..default()
    };

    let mut monster_id: usize = 0;

    // Skip the first room because that's where the player starts
    for room in map.rooms.iter().skip(1) {
        spawner::spawn_room(&mut commands, &mut rng, &text_font, room, &mut monster_id, map.depth);
    }
}

fn update_blocked_tiles(mut map: ResMut<Map>, monster_query: Query<&Position, With<Monster>>) {
    // Clear blocked tiles and re-populate from walls
    let size = MAP_WIDTH * MAP_HEIGHT;
    for idx in 0..size {
        map.blocked_tiles[idx] = map.tiles[idx] == TileType::Wall;
    }

    // Block tiles with monsters
    for pos in &monster_query {
        let idx = map.xy_idx(pos.x, pos.y);
        map.blocked_tiles[idx] = true;
    }
}

pub fn monster_ai(
    mut commands: Commands,
    mut map: ResMut<Map>,
    mut gamelog: ResMut<GameLog>,
    mut monster_query: Query<
        (Entity, &mut Position, &mut Viewshed, &Name, &CombatStats, Option<&mut Confusion>),
        (With<Monster>, Without<Player>),
    >,
    player_query: Single<(Entity, &Position), With<Player>>,
) {
    let (player_entity, player_pos) = player_query.into_inner();
    let player_idx = map.xy_idx(player_pos.x, player_pos.y);

    for (entity, mut pos, mut viewshed, name, stats, confusion) in &mut monster_query {
        if stats.hp <= 0 {
            continue;
        }

        // Handle confusion
        if let Some(mut confused) = confusion {
            confused.turns -= 1;
            if confused.turns < 1 {
                commands.entity(entity).remove::<Confusion>();
                gamelog.entries.push(format!("{} is no longer confused.", name.name));
            } else {
                gamelog.entries.push(format!("{} is confused and stumbles around.", name.name));
            }
            continue; // Skip normal AI while confused
        }

        let distance = DistanceAlg::Chebyshev.distance2d(
            Vec2::new(pos.x as f32, pos.y as f32),
            Vec2::new(player_pos.x as f32, player_pos.y as f32),
        );
        if distance < 1.5 {
            commands.entity(entity).insert(WantsToMelee {
                target: player_entity,
            });
            continue;
        }

        // Check if player is visible
        if viewshed
            .visible_tiles
            .contains(&(player_pos.x, player_pos.y))
        {
            let monster_idx = map.xy_idx(pos.x, pos.y);

            // Find path to player (ignoring other entities so monsters keep chasing)
            if let Some(path) = pathfinding::a_star_ignoring_entities(&map, monster_idx, player_idx) {
                // Move one step toward player (path[0] is current position)
                if path.len() > 1 {
                    let next_idx = path[1];
                    // Only move if destination is not blocked
                    if !map.blocked_tiles[next_idx] {
                        // Unblock old position, block new position
                        map.blocked_tiles[monster_idx] = false;
                        map.blocked_tiles[next_idx] = true;
                        pos.x = (next_idx % map.width as usize) as i32;
                        pos.y = (next_idx / map.width as usize) as i32;
                        viewshed.dirty = true;
                    }
                }
            }
        }
    }
}
