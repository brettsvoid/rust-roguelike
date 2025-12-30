use bevy::input::keyboard::KeyboardInput;
use bevy::input::ButtonState;
use bevy::prelude::*;

use crate::camera::Camera as GameCamera;
use crate::components::{AreaOfEffect, Targeting, WantsToUseItem};
use crate::distance::DistanceAlg;
use crate::map::{Map, Position, TileType, GRID_PX, MAP_HEIGHT, MAP_WIDTH};
use crate::monsters::Monster;
use crate::player::Player;
use crate::resources::UiFont;
use crate::{RunState, TargetingInfo};

use super::components::{RangeIndicator, TargetBorder, TargetHighlight, TargetingMenu};

pub struct TargetingPlugin;

impl Plugin for TargetingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(RunState::ShowTargeting),
            (spawn_targeting_ui, spawn_target_borders, spawn_range_indicator),
        )
        .add_systems(OnExit(RunState::ShowTargeting), despawn_targeting_ui)
        .add_systems(
            Update,
            (update_target_highlight, handle_targeting)
                .chain()
                .run_if(in_state(RunState::ShowTargeting)),
        );
    }
}

fn spawn_targeting_ui(mut commands: Commands, font: Res<UiFont>, targeting_info: Res<TargetingInfo>) {
    let range = targeting_info.range;

    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(10.0),
                left: Val::Px(10.0),
                padding: UiRect::all(Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.1, 0.1, 0.1, 0.9)),
            TargetingMenu,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new(format!(
                    "Select Target (Range: {})\nClick on a target or press Escape to cancel",
                    range
                )),
                TextFont {
                    font: font.0.clone(),
                    font_size: 16.0,
                    ..default()
                },
                TextColor(Color::srgb(1.0, 1.0, 0.0)), // Yellow
            ));
        });
}

fn despawn_targeting_ui(
    mut commands: Commands,
    menu_query: Query<Entity, With<TargetingMenu>>,
    highlight_query: Query<Entity, With<TargetHighlight>>,
    border_query: Query<Entity, With<TargetBorder>>,
    range_query: Query<Entity, With<RangeIndicator>>,
) {
    for entity in &menu_query {
        commands.entity(entity).despawn_recursive();
    }
    for entity in &highlight_query {
        commands.entity(entity).despawn();
    }
    for entity in &border_query {
        commands.entity(entity).despawn();
    }
    for entity in &range_query {
        commands.entity(entity).despawn();
    }
}

fn spawn_target_borders(
    mut commands: Commands,
    window: Query<&Window>,
    game_camera: Res<GameCamera>,
    map: Res<Map>,
    targeting_info: Res<TargetingInfo>,
    player_query: Query<&Position, With<Player>>,
    monster_query: Query<&Position, With<Monster>>,
    targeting_query: Query<&Targeting>,
) {
    // Only show borders for items that require an entity target
    let Some(item) = targeting_info.item else {
        return;
    };
    let targeting = targeting_query.get(item).copied().unwrap_or_default();
    if targeting != Targeting::SingleEntity {
        return;
    }

    let Ok(window) = window.get_single() else {
        return;
    };

    let Ok(player_pos) = player_query.get_single() else {
        return;
    };

    let half_width = window.width() / 2.0;
    let half_height = window.height() / 2.0;
    let border_width = 2.0;

    // Find all monsters within range and on visible tiles
    for monster_pos in &monster_query {
        let idx = map.xy_idx(monster_pos.x, monster_pos.y);
        if !map.visible_tiles[idx] {
            continue;
        }

        let distance = DistanceAlg::Euclidean.distance2d(
            Vec2::new(player_pos.x as f32, player_pos.y as f32),
            Vec2::new(monster_pos.x as f32, monster_pos.y as f32),
        );

        if distance <= targeting_info.range as f32 {
            // Convert world coords to screen coords for rendering
            let (screen_x, screen_y) = game_camera.world_to_screen(monster_pos.x, monster_pos.y);
            let center_x = (screen_x as f32) * GRID_PX.x + (GRID_PX.x / 2.0) - half_width;
            let center_y = (screen_y as f32) * -GRID_PX.y - (GRID_PX.y / 2.0) + half_height;

            // Top border
            commands.spawn((
                Sprite {
                    color: Color::srgb(0.0, 1.0, 1.0),
                    custom_size: Some(Vec2::new(GRID_PX.x, border_width)),
                    ..default()
                },
                Transform::from_xyz(center_x, center_y + GRID_PX.y / 2.0 - border_width / 2.0, 0.5),
                TargetBorder,
            ));

            // Bottom border
            commands.spawn((
                Sprite {
                    color: Color::srgb(0.0, 1.0, 1.0),
                    custom_size: Some(Vec2::new(GRID_PX.x, border_width)),
                    ..default()
                },
                Transform::from_xyz(center_x, center_y - GRID_PX.y / 2.0 + border_width / 2.0, 0.5),
                TargetBorder,
            ));

            // Left border
            commands.spawn((
                Sprite {
                    color: Color::srgb(0.0, 1.0, 1.0),
                    custom_size: Some(Vec2::new(border_width, GRID_PX.y)),
                    ..default()
                },
                Transform::from_xyz(center_x - GRID_PX.x / 2.0 + border_width / 2.0, center_y, 0.5),
                TargetBorder,
            ));

            // Right border
            commands.spawn((
                Sprite {
                    color: Color::srgb(0.0, 1.0, 1.0),
                    custom_size: Some(Vec2::new(border_width, GRID_PX.y)),
                    ..default()
                },
                Transform::from_xyz(center_x + GRID_PX.x / 2.0 - border_width / 2.0, center_y, 0.5),
                TargetBorder,
            ));
        }
    }
}

fn spawn_range_indicator(
    mut commands: Commands,
    window: Query<&Window>,
    game_camera: Res<GameCamera>,
    map: Res<Map>,
    targeting_info: Res<TargetingInfo>,
    player_query: Query<&Position, With<Player>>,
) {
    let Ok(window) = window.get_single() else {
        return;
    };

    let Ok(player_pos) = player_query.get_single() else {
        return;
    };

    let half_width = window.width() / 2.0;
    let half_height = window.height() / 2.0;
    let border_width = 1.0;
    let range = targeting_info.range;

    // Muted gray color for range border
    let border_color = Color::srgba(0.5, 0.5, 0.5, 0.5);

    // Helper to check if a tile is a valid floor tile in range
    let is_valid_floor = |x: i32, y: i32| -> bool {
        if x < 0 || x >= MAP_WIDTH as i32 || y < 0 || y >= MAP_HEIGHT as i32 {
            return false;
        }
        let idx = map.xy_idx(x, y);
        map.tiles[idx] == TileType::Floor
    };

    // Check each tile and draw borders on the edge of range
    for dy in -range..=range {
        for dx in -range..=range {
            let tile_x = player_pos.x + dx;
            let tile_y = player_pos.y + dy;

            // Check bounds
            if tile_x < 0 || tile_x >= MAP_WIDTH as i32 || tile_y < 0 || tile_y >= MAP_HEIGHT as i32 {
                continue;
            }

            let idx = map.xy_idx(tile_x, tile_y);

            // Only draw on floor tiles that are visible
            if map.tiles[idx] != TileType::Floor || !map.visible_tiles[idx] {
                continue;
            }

            let distance = DistanceAlg::Euclidean.distance2d(
                Vec2::new(player_pos.x as f32, player_pos.y as f32),
                Vec2::new(tile_x as f32, tile_y as f32),
            );

            // Only draw on tiles that are within range
            if distance > range as f32 {
                continue;
            }

            // Convert world coords to screen coords for rendering
            let (screen_x, screen_y) = game_camera.world_to_screen(tile_x, tile_y);
            let center_x = (screen_x as f32) * GRID_PX.x + (GRID_PX.x / 2.0) - half_width;
            let center_y = (screen_y as f32) * -GRID_PX.y - (GRID_PX.y / 2.0) + half_height;

            // Check neighbors - only draw border if neighbor is out of range AND is a floor tile

            // Top neighbor
            if is_valid_floor(tile_x, tile_y - 1) {
                let top_dist = DistanceAlg::Euclidean.distance2d(
                    Vec2::new(player_pos.x as f32, player_pos.y as f32),
                    Vec2::new(tile_x as f32, (tile_y - 1) as f32),
                );
                if top_dist > range as f32 {
                    commands.spawn((
                        Sprite {
                            color: border_color,
                            custom_size: Some(Vec2::new(GRID_PX.x, border_width)),
                            ..default()
                        },
                        Transform::from_xyz(center_x, center_y + GRID_PX.y / 2.0 - border_width / 2.0, 0.3),
                        RangeIndicator,
                    ));
                }
            }

            // Bottom neighbor
            if is_valid_floor(tile_x, tile_y + 1) {
                let bottom_dist = DistanceAlg::Euclidean.distance2d(
                    Vec2::new(player_pos.x as f32, player_pos.y as f32),
                    Vec2::new(tile_x as f32, (tile_y + 1) as f32),
                );
                if bottom_dist > range as f32 {
                    commands.spawn((
                        Sprite {
                            color: border_color,
                            custom_size: Some(Vec2::new(GRID_PX.x, border_width)),
                            ..default()
                        },
                        Transform::from_xyz(center_x, center_y - GRID_PX.y / 2.0 + border_width / 2.0, 0.3),
                        RangeIndicator,
                    ));
                }
            }

            // Left neighbor
            if is_valid_floor(tile_x - 1, tile_y) {
                let left_dist = DistanceAlg::Euclidean.distance2d(
                    Vec2::new(player_pos.x as f32, player_pos.y as f32),
                    Vec2::new((tile_x - 1) as f32, tile_y as f32),
                );
                if left_dist > range as f32 {
                    commands.spawn((
                        Sprite {
                            color: border_color,
                            custom_size: Some(Vec2::new(border_width, GRID_PX.y)),
                            ..default()
                        },
                        Transform::from_xyz(center_x - GRID_PX.x / 2.0 + border_width / 2.0, center_y, 0.3),
                        RangeIndicator,
                    ));
                }
            }

            // Right neighbor
            if is_valid_floor(tile_x + 1, tile_y) {
                let right_dist = DistanceAlg::Euclidean.distance2d(
                    Vec2::new(player_pos.x as f32, player_pos.y as f32),
                    Vec2::new((tile_x + 1) as f32, tile_y as f32),
                );
                if right_dist > range as f32 {
                    commands.spawn((
                        Sprite {
                            color: border_color,
                            custom_size: Some(Vec2::new(border_width, GRID_PX.y)),
                            ..default()
                        },
                        Transform::from_xyz(center_x + GRID_PX.x / 2.0 - border_width / 2.0, center_y, 0.3),
                        RangeIndicator,
                    ));
                }
            }
        }
    }
}

fn update_target_highlight(
    mut commands: Commands,
    window: Query<&Window>,
    camera_query: Query<(&Camera, &GlobalTransform)>,
    game_camera: Res<GameCamera>,
    map: Res<Map>,
    targeting_info: Res<TargetingInfo>,
    player_query: Query<&Position, With<Player>>,
    monster_query: Query<&Position, With<Monster>>,
    highlight_query: Query<Entity, With<TargetHighlight>>,
    aoe_query: Query<&AreaOfEffect>,
    targeting_query: Query<&Targeting>,
) {
    // Remove existing highlight
    for entity in &highlight_query {
        commands.entity(entity).despawn();
    }

    let Ok(window) = window.get_single() else {
        return;
    };

    let Some(cursor_position) = window.cursor_position() else {
        return;
    };

    let Ok((camera, camera_transform)) = camera_query.get_single() else {
        return;
    };

    let Ok(world_pos) = camera.viewport_to_world_2d(camera_transform, cursor_position) else {
        return;
    };

    let Ok(player_pos) = player_query.get_single() else {
        return;
    };

    // Convert world position (pixels) to screen tile coordinates
    let half_width = window.width() / 2.0;
    let half_height = window.height() / 2.0;

    let screen_tile_x = ((world_pos.x + half_width) / GRID_PX.x).floor() as i32;
    let screen_tile_y = ((-world_pos.y + half_height) / GRID_PX.y).floor() as i32;

    // Convert screen tile coordinates to world map coordinates using game camera
    let (map_x, map_y) = game_camera.screen_to_world(screen_tile_x, screen_tile_y);

    // Check bounds
    if map_x < 0 || map_x >= MAP_WIDTH as i32 || map_y < 0 || map_y >= MAP_HEIGHT as i32 {
        return;
    }

    // Check if tile is visible
    let idx = map.xy_idx(map_x, map_y);
    if !map.visible_tiles[idx] {
        return;
    }

    // Check distance from player
    let distance = DistanceAlg::Euclidean.distance2d(
        Vec2::new(player_pos.x as f32, player_pos.y as f32),
        Vec2::new(map_x as f32, map_y as f32),
    );

    if distance > targeting_info.range as f32 {
        return;
    }

    let Some(item) = targeting_info.item else {
        return;
    };

    let targeting = targeting_query.get(item).copied().unwrap_or_default();

    match targeting {
        Targeting::SingleEntity => {
            // Only highlight if there's a monster
            let has_monster = monster_query.iter().any(|pos| pos.x == map_x && pos.y == map_y);

            if has_monster {
                // Use screen coords for rendering
                commands.spawn((
                    Sprite {
                        color: Color::srgba(0.0, 1.0, 1.0, 0.4), // Cyan with transparency
                        custom_size: Some(Vec2::new(GRID_PX.x, GRID_PX.y)),
                        ..default()
                    },
                    Transform::from_xyz(
                        (screen_tile_x as f32) * GRID_PX.x + (GRID_PX.x / 2.0) - half_width,
                        (screen_tile_y as f32) * -GRID_PX.y - (GRID_PX.y / 2.0) + half_height,
                        0.5,
                    ),
                    TargetHighlight,
                ));
            }
        }
        Targeting::Tile => {
            // Show AoE radius if item has AreaOfEffect
            if let Ok(aoe) = aoe_query.get(item) {
                let radius = aoe.radius;
                for dy in -radius..=radius {
                    for dx in -radius..=radius {
                        let tile_x = map_x + dx;
                        let tile_y = map_y + dy;

                        if tile_x < 0
                            || tile_x >= MAP_WIDTH as i32
                            || tile_y < 0
                            || tile_y >= MAP_HEIGHT as i32
                        {
                            continue;
                        }

                        let tile_distance = DistanceAlg::Euclidean.distance2d(
                            Vec2::new(map_x as f32, map_y as f32),
                            Vec2::new(tile_x as f32, tile_y as f32),
                        );

                        if tile_distance <= radius as f32 {
                            // Convert world coords to screen coords for rendering
                            let (sx, sy) = game_camera.world_to_screen(tile_x, tile_y);
                            commands.spawn((
                                Sprite {
                                    color: Color::srgba(1.0, 0.5, 0.0, 0.4), // Orange for AoE
                                    custom_size: Some(Vec2::new(GRID_PX.x, GRID_PX.y)),
                                    ..default()
                                },
                                Transform::from_xyz(
                                    (sx as f32) * GRID_PX.x + (GRID_PX.x / 2.0) - half_width,
                                    (sy as f32) * -GRID_PX.y - (GRID_PX.y / 2.0) + half_height,
                                    0.5,
                                ),
                                TargetHighlight,
                            ));
                        }
                    }
                }
            }
        }
    }
}

fn handle_targeting(
    mut commands: Commands,
    mut evr_kbd: EventReader<KeyboardInput>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    window: Query<&Window>,
    camera_query: Query<(&Camera, &GlobalTransform)>,
    game_camera: Res<GameCamera>,
    map: Res<Map>,
    targeting_info: Res<TargetingInfo>,
    mut next_state: ResMut<NextState<RunState>>,
    player_query: Query<(Entity, &Position), With<Player>>,
    monster_query: Query<&Position, With<Monster>>,
    targeting_query: Query<&Targeting>,
) {
    // Handle escape to cancel
    for ev in evr_kbd.read() {
        if ev.state == ButtonState::Pressed && ev.key_code == KeyCode::Escape {
            next_state.set(RunState::AwaitingInput);
            return;
        }
    }

    // Handle mouse click
    if !mouse_button.just_pressed(MouseButton::Left) {
        return;
    }

    let Ok(window) = window.get_single() else {
        return;
    };

    let Some(cursor_position) = window.cursor_position() else {
        return;
    };

    let Ok((camera, camera_transform)) = camera_query.get_single() else {
        return;
    };

    let Ok(world_pos) = camera.viewport_to_world_2d(camera_transform, cursor_position) else {
        return;
    };

    // Convert world position (pixels) to screen tile coordinates
    let half_width = window.width() / 2.0;
    let half_height = window.height() / 2.0;

    let screen_tile_x = ((world_pos.x + half_width) / GRID_PX.x).floor() as i32;
    let screen_tile_y = ((-world_pos.y + half_height) / GRID_PX.y).floor() as i32;

    // Convert screen tile coordinates to world map coordinates using game camera
    let (map_x, map_y) = game_camera.screen_to_world(screen_tile_x, screen_tile_y);

    // Check bounds
    if map_x < 0 || map_x >= MAP_WIDTH as i32 || map_y < 0 || map_y >= MAP_HEIGHT as i32 {
        return;
    }

    // Check if tile is visible
    let idx = map.xy_idx(map_x, map_y);
    if !map.visible_tiles[idx] {
        return;
    }

    // Get player position for range check
    let Ok((player_entity, player_pos)) = player_query.get_single() else {
        return;
    };

    // Check distance from player
    let distance = DistanceAlg::Euclidean.distance2d(
        Vec2::new(player_pos.x as f32, player_pos.y as f32),
        Vec2::new(map_x as f32, map_y as f32),
    );

    if distance > targeting_info.range as f32 {
        return; // Out of range
    }

    let Some(item) = targeting_info.item else {
        return;
    };

    let targeting = targeting_query.get(item).copied().unwrap_or_default();

    match targeting {
        Targeting::SingleEntity => {
            // Requires a monster at the position
            let has_monster = monster_query.iter().any(|pos| pos.x == map_x && pos.y == map_y);
            if has_monster {
                commands.entity(player_entity).insert(WantsToUseItem {
                    item,
                    target: Some((map_x, map_y)),
                });
                next_state.set(RunState::PlayerTurn);
            }
        }
        Targeting::Tile => {
            // Can target any visible tile in range
            commands.entity(player_entity).insert(WantsToUseItem {
                item,
                target: Some((map_x, map_y)),
            });
            next_state.set(RunState::PlayerTurn);
        }
    }
}
