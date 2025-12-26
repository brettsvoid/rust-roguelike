use bevy::prelude::*;

use crate::combat::CombatStats;
use crate::components::{HungerClock, Item};
use crate::map::{Map, Position, Revealed, RevealedState, Tile, TileType, GRID_PX, MAP_HEIGHT, MAP_WIDTH};
use crate::monsters::Monster;
use crate::player::Player;
use crate::resources::UiFont;
use crate::viewshed::Viewshed;
use crate::RunState;

use super::resources::{DebugMode, DebugState, GodMode};

// ============================================================================
// Components
// ============================================================================

#[derive(Component)]
pub struct FovOverlay;

#[derive(Component)]
pub struct TileInfoOverlay;

#[derive(Component)]
pub struct StateInspector;

// ============================================================================
// Toggle Systems
// ============================================================================

pub fn toggle_debug_mode(keyboard: Res<ButtonInput<KeyCode>>, mut debug: ResMut<DebugMode>) {
    if keyboard.just_pressed(KeyCode::F12) {
        debug.enabled = !debug.enabled;
        if debug.enabled {
            info!("Debug mode enabled");
        } else {
            info!("Debug mode disabled");
        }
    }
}

pub fn toggle_debug_overlays(keyboard: Res<ButtonInput<KeyCode>>, mut debug: ResMut<DebugMode>) {
    if keyboard.just_pressed(KeyCode::F1) {
        debug.show_fov_overlay = !debug.show_fov_overlay;
    }
    if keyboard.just_pressed(KeyCode::F3) {
        debug.show_tile_info = !debug.show_tile_info;
    }
    if keyboard.just_pressed(KeyCode::F4) {
        debug.show_inspector = !debug.show_inspector;
    }
    if keyboard.just_pressed(KeyCode::Backquote) {
        debug.show_console = !debug.show_console;
    }
}

// ============================================================================
// FOV Overlay (F1)
// ============================================================================

pub fn update_fov_overlay(
    mut commands: Commands,
    debug: Res<DebugMode>,
    window: Query<&Window>,
    player_query: Query<&Viewshed, With<Player>>,
    overlay_query: Query<Entity, With<FovOverlay>>,
) {
    // Despawn existing overlays
    for entity in &overlay_query {
        commands.entity(entity).despawn();
    }

    if !debug.show_fov_overlay {
        return;
    }

    let Ok(window) = window.get_single() else {
        return;
    };

    let Ok(viewshed) = player_query.get_single() else {
        return;
    };

    let half_width = window.width() / 2.0;
    let half_height = window.height() / 2.0;

    // Spawn overlay for each visible tile
    for &(x, y) in &viewshed.visible_tiles {
        let screen_x = (x as f32) * GRID_PX.x + (GRID_PX.x / 2.0) - half_width;
        let screen_y = (y as f32) * -GRID_PX.y - (GRID_PX.y / 2.0) + half_height;

        commands.spawn((
            FovOverlay,
            Sprite {
                color: Color::srgba(0.0, 1.0, 1.0, 0.15),
                custom_size: Some(Vec2::new(GRID_PX.x, GRID_PX.y)),
                ..default()
            },
            Transform::from_xyz(screen_x, screen_y, 5.0),
        ));
    }
}

// ============================================================================
// Tile Info Overlay (F3)
// ============================================================================

pub fn update_tile_info_overlay(
    mut commands: Commands,
    debug: Res<DebugMode>,
    map: Res<Map>,
    window: Query<&Window>,
    camera_query: Query<(&Camera, &GlobalTransform)>,
    font: Res<UiFont>,
    overlay_query: Query<Entity, With<TileInfoOverlay>>,
    tile_query: Query<Entity, With<Tile>>,
) {
    // Despawn existing overlay
    for entity in &overlay_query {
        commands.entity(entity).despawn();
    }

    if !debug.show_tile_info {
        return;
    }

    let Ok(window) = window.get_single() else {
        return;
    };
    let Ok((camera, camera_transform)) = camera_query.get_single() else {
        return;
    };
    let Some(cursor_pos) = window.cursor_position() else {
        return;
    };
    let Ok(world_pos) = camera.viewport_to_world_2d(camera_transform, cursor_pos) else {
        return;
    };

    let half_width = window.width() / 2.0;
    let half_height = window.height() / 2.0;

    // Convert world position to map coordinates
    let grid_x = ((world_pos.x + half_width) / GRID_PX.x).floor() as i32;
    let grid_y = ((-world_pos.y + half_height) / GRID_PX.y).floor() as i32;

    // Bounds check
    if grid_x < 0 || grid_x >= MAP_WIDTH as i32 || grid_y < 0 || grid_y >= MAP_HEIGHT as i32 {
        return;
    }

    let idx = map.xy_idx(grid_x, grid_y);
    let tile_type = match map.tiles[idx] {
        TileType::Floor => "Floor",
        TileType::Wall => "Wall",
        TileType::DownStairs => "Stairs",
    };
    let blocked = map.blocked_tiles[idx];

    // Count entities excluding Tile entities
    let entity_count = map.tile_content[idx]
        .iter()
        .filter(|e| tile_query.get(**e).is_err())
        .count();

    let info_text = format!(
        "({}, {}) {} B:{} E:{}",
        grid_x,
        grid_y,
        tile_type,
        if blocked { "Y" } else { "N" },
        entity_count
    );

    // Spawn text near cursor
    commands.spawn((
        TileInfoOverlay,
        Text2d::new(info_text),
        TextFont {
            font: font.0.clone(),
            font_size: 14.0,
            ..default()
        },
        TextColor(Color::WHITE),
        Transform::from_xyz(world_pos.x + 20.0, world_pos.y + 10.0, 100.0),
    ));
}

// ============================================================================
// State Inspector (F4)
// ============================================================================

pub fn update_state_inspector(
    mut commands: Commands,
    debug: Res<DebugMode>,
    debug_state: Res<DebugState>,
    god_mode: Res<GodMode>,
    state: Res<State<RunState>>,
    map: Res<Map>,
    font: Res<UiFont>,
    player_query: Query<(&Position, &CombatStats, &HungerClock), With<Player>>,
    monster_query: Query<Entity, With<Monster>>,
    item_query: Query<Entity, With<Item>>,
    inspector_query: Query<Entity, With<StateInspector>>,
) {
    // Despawn existing inspector
    for entity in &inspector_query {
        commands.entity(entity).despawn_recursive();
    }

    if !debug.show_inspector {
        return;
    }

    let state_name = format!("{:?}", state.get());
    let depth = map.depth;

    let (player_info, hp_info, hunger_info) = if let Ok((pos, stats, hunger)) = player_query.get_single() {
        (
            format!("({}, {})", pos.x, pos.y),
            format!("{}/{}", stats.hp, stats.max_hp),
            format!("{:?} ({})", hunger.state, hunger.duration),
        )
    } else {
        ("N/A".to_string(), "N/A".to_string(), "N/A".to_string())
    };

    let monster_count = monster_query.iter().count();
    let item_count = item_query.iter().count();
    let god_mode_str = if god_mode.0 { "ON" } else { "OFF" };
    let no_fog_str = if debug_state.no_fog { "ON" } else { "OFF" };

    let text_font = TextFont {
        font: font.0.clone(),
        font_size: 14.0,
        ..default()
    };

    commands
        .spawn((
            StateInspector,
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(10.0),
                top: Val::Px(10.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(8.0)),
                row_gap: Val::Px(4.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.1, 0.1, 0.1, 0.9)),
        ))
        .with_children(|parent| {
            // Title
            parent.spawn((
                Text::new("DEBUG INSPECTOR"),
                text_font.clone(),
                TextColor(Color::srgb(1.0, 1.0, 0.0)),
            ));

            // State
            parent.spawn((
                Text::new(format!("State: {}", state_name)),
                text_font.clone(),
                TextColor(Color::WHITE),
            ));

            // Depth
            parent.spawn((
                Text::new(format!("Depth: {}", depth)),
                text_font.clone(),
                TextColor(Color::WHITE),
            ));

            // Player
            parent.spawn((
                Text::new(format!("Player: {} HP {}", player_info, hp_info)),
                text_font.clone(),
                TextColor(Color::WHITE),
            ));

            // Hunger
            parent.spawn((
                Text::new(format!("Hunger: {}", hunger_info)),
                text_font.clone(),
                TextColor(Color::WHITE),
            ));

            // Entities
            parent.spawn((
                Text::new(format!("Monsters: {}  Items: {}", monster_count, item_count)),
                text_font.clone(),
                TextColor(Color::WHITE),
            ));

            // God Mode
            parent.spawn((
                Text::new(format!("God Mode: {}  No Fog: {}", god_mode_str, no_fog_str)),
                text_font.clone(),
                TextColor(if god_mode.0 || debug_state.no_fog {
                    Color::srgb(0.0, 1.0, 0.0)
                } else {
                    Color::WHITE
                }),
            ));

            // Key hints
            parent.spawn((
                Text::new("F1:FOV F3:Tile F4:This `:Console"),
                TextFont {
                    font: font.0.clone(),
                    font_size: 12.0,
                    ..default()
                },
                TextColor(Color::srgb(0.5, 0.5, 0.5)),
            ));
        });
}

// ============================================================================
// Map Reveal System
// ============================================================================

pub fn process_reveal_map(
    mut debug_state: ResMut<DebugState>,
    mut map: ResMut<Map>,
    mut tile_query: Query<(&Position, &mut Revealed), With<Tile>>,
) {
    if !debug_state.reveal_map {
        return;
    }

    // Mark all tiles as revealed in the map resource
    for i in 0..map.revealed_tiles.len() {
        map.revealed_tiles[i] = true;
    }

    // Update all Tile entities to show as explored
    for (pos, mut revealed) in &mut tile_query {
        let idx = map.xy_idx(pos.x, pos.y);
        if map.revealed_tiles[idx] {
            *revealed = Revealed(RevealedState::Explored);
        }
    }

    // Reset the flag
    debug_state.reveal_map = false;
}
