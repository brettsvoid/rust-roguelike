use bevy::prelude::*;

use crate::combat::CombatStats;
use crate::components::{HungerClock, Item};
use crate::map::{Map, Position, Revealed, RevealedState, Tile, TileType};
use crate::monsters::Monster;
use crate::picking::{HoveredTile, TileMarks, FULL_TILE};
use crate::party::{ActiveUnit, PartyMember};
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
    map: Res<Map>,
    marks: Res<TileMarks>,
    party: Query<&Viewshed, With<PartyMember>>,
    overlay_query: Query<Entity, With<FovOverlay>>,
) {
    // Despawn existing overlays
    for entity in &overlay_query {
        commands.entity(entity).despawn();
    }

    if !debug.show_fov_overlay {
        return;
    }

    // Mark each visible tile
    for &cell in party.iter().flat_map(|v| v.visible_tiles.iter()) {
        commands.spawn((FovOverlay, marks.mark(&marks.area, &map, cell, FULL_TILE, 0.0)));
    }
}

// ============================================================================
// Tile Info Overlay (F3)
// ============================================================================

pub fn update_tile_info_overlay(
    mut commands: Commands,
    debug: Res<DebugMode>,
    map: Res<Map>,
    hovered: Res<HoveredTile>,
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

    let (Some(cursor_pos), Some((grid_x, grid_y))) = (hovered.cursor, hovered.tile) else {
        return;
    };

    let idx = map.xy_idx(grid_x, grid_y);
    let tile_type = match map.tiles[idx] {
        TileType::Floor => "Floor",
        TileType::Wall => "Wall",
        TileType::DownStairs => "Stairs",
        TileType::UpStairs => "UpStairs",
        TileType::Road => "Road",
        TileType::Grass => "Grass",
        TileType::ShallowWater => "ShallowWater",
        TileType::DeepWater => "DeepWater",
        TileType::WoodFloor => "WoodFloor",
        TileType::Bridge => "Bridge",
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
        Text::new(info_text),
        TextFont {
            font: font.0.clone().into(),
            font_size: FontSize::Px(14.0),
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(cursor_pos.x + 20.0),
            top: Val::Px(cursor_pos.y - 24.0),
            ..default()
        },
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
    active: Res<ActiveUnit>,
    party: Query<(&Position, &CombatStats, &HungerClock), With<PartyMember>>,
    monster_query: Query<Entity, With<Monster>>,
    item_query: Query<Entity, With<Item>>,
    inspector_query: Query<Entity, With<StateInspector>>,
) {
    // Despawn existing inspector
    for entity in &inspector_query {
        commands.entity(entity).despawn();
    }

    if !debug.show_inspector {
        return;
    }

    let state_name = format!("{:?}", state.get());
    let depth = map.depth;

    let (player_info, hp_info, hunger_info) = if let Some((pos, stats, hunger)) = active.0.and_then(|unit| party.get(unit).ok()) {
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
        font: font.0.clone().into(),
        font_size: FontSize::Px(14.0),
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

            // Active unit
            parent.spawn((
                Text::new(format!("Active: {} HP {}", player_info, hp_info)),
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
                    font: font.0.clone().into(),
                    font_size: FontSize::Px(12.0),
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
