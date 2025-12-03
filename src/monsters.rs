use bevy::{color::palettes, prelude::*};
use rand::Rng;

use crate::{
    combat::{CombatStats, WantsToMelee},
    components::{BlocksTile, Name},
    distance::DistanceAlg,
    map::{Map, Position, TileType, FONT_SIZE, MAP_HEIGHT, MAP_WIDTH},
    pathfinding,
    player::Player,
    resources::UiFont,
    viewshed::{self, Viewshed},
    RunState,
};

#[derive(Component, Debug)]
pub struct Monster;

pub struct MonstersPlugin;
impl Plugin for MonstersPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_monsters).add_systems(
            Update,
            (
                update_monsters,
                update_blocked_tiles.run_if(in_state(RunState::MonsterTurn)),
            ),
        );
    }
}

fn setup_monsters(mut commands: Commands, font: Res<UiFont>, map: Res<Map>) {
    let text_font = TextFont {
        font: font.0.clone(),
        font_size: FONT_SIZE,
        ..default()
    };

    let mut rng = rand::thread_rng();
    let roll = rng.gen_range(0..=1);
    let monster_type: &str;
    let name: String;
    match roll {
        0 => {
            monster_type = "o";
            name = "Orc".to_string();
        }
        _ => {
            monster_type = "g";
            name = "Goblin".to_string();
        }
    };

    // Skip the first room because that's where the player starts
    for (i, room) in map.rooms.iter().skip(1).enumerate() {
        let (x, y) = room.center();
        commands.spawn((
            Monster,
            Name {
                name: format!("{} #{}", &name, i),
            },
            Position { x, y, z: 1 },
            BlocksTile,
            CombatStats {
                max_hp: 16,
                hp: 16,
                defense: 1,
                power: 4,
            },
            Text2d::new(monster_type),
            text_font.clone(),
            Viewshed {
                range: 8,
                ..default()
            },
            TextColor(palettes::basic::RED.into()),
            BackgroundColor(palettes::basic::BLACK.into()),
            Visibility::Hidden,
        ));
    }
}

fn update_monsters(
    map: Res<Map>,
    mut monster_query: Query<(&Position, &mut Visibility), With<Monster>>,
) {
    for (pos, mut visibility) in &mut monster_query {
        let idx = map.xy_idx(pos.x, pos.y);
        if map.visible_tiles[idx] && matches!(*visibility, Visibility::Hidden) {
            visibility.toggle_visible_hidden();
        } else if matches!(*visibility, Visibility::Visible) {
            visibility.toggle_visible_hidden();
        }
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
    mut monster_query: Query<
        (Entity, &mut Position, &mut Viewshed, &Name, &CombatStats),
        (With<Monster>, Without<Player>),
    >,
    player_query: Single<(Entity, &Position), With<Player>>,
) {
    let (player_entity, player_pos) = player_query.into_inner();
    let player_idx = map.xy_idx(player_pos.x, player_pos.y);

    for (entity, mut pos, mut viewshed, _name, stats) in &mut monster_query {
        if stats.hp <= 0 {
            continue;
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
