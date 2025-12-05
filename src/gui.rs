use bevy::input::keyboard::KeyboardInput;
use bevy::input::ButtonState;
use bevy::prelude::*;

use crate::combat::CombatStats;
use crate::components::{AreaOfEffect, InBackpack, Item, Name, Ranged, Targeting, WantsToDropItem, WantsToUseItem};
use crate::distance::DistanceAlg;
use crate::gamelog::GameLog;
use crate::map::{Map, Position, TileType, GRID_PX, MAP_HEIGHT, MAP_WIDTH};
use crate::monsters::Monster;
use crate::player::Player;
use crate::resources::UiFont;
use crate::{RunState, TargetingInfo};

pub struct GuiPlugin;

impl Plugin for GuiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_gui)
            .add_systems(Update, (update_health_bar, update_game_log, update_tooltip))
            .add_systems(OnEnter(RunState::ShowInventory), spawn_inventory_menu)
            .add_systems(OnExit(RunState::ShowInventory), despawn_inventory_menu)
            .add_systems(
                Update,
                handle_inventory_input.run_if(in_state(RunState::ShowInventory)),
            )
            .add_systems(OnEnter(RunState::ShowDropItem), spawn_drop_menu)
            .add_systems(OnExit(RunState::ShowDropItem), despawn_drop_menu)
            .add_systems(
                Update,
                handle_drop_input.run_if(in_state(RunState::ShowDropItem)),
            )
            .add_systems(OnEnter(RunState::ShowTargeting), (spawn_targeting_ui, spawn_target_borders, spawn_range_indicator))
            .add_systems(OnExit(RunState::ShowTargeting), despawn_targeting_ui)
            .add_systems(
                Update,
                (update_target_highlight, handle_targeting)
                    .chain()
                    .run_if(in_state(RunState::ShowTargeting)),
            );
    }
}

#[derive(Component)]
struct HealthText;

#[derive(Component)]
struct HealthBar;

#[derive(Component)]
struct GameLogText;

#[derive(Component)]
struct Tooltip;

#[derive(Component)]
struct CursorHighlight;

#[derive(Component)]
struct InventoryMenu;

#[derive(Component)]
struct DropMenu;

#[derive(Component)]
struct TargetingMenu;

#[derive(Component)]
struct TargetHighlight;

#[derive(Component)]
struct TargetBorder;

#[derive(Component)]
struct RangeIndicator;

fn setup_gui(mut commands: Commands, font: Res<UiFont>) {
    // Bottom panel
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(0.0),
                left: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Px(112.0), // 7 rows * 16px
                padding: UiRect::all(Val::Px(8.0)),
                column_gap: Val::Px(8.0),
                ..default()
            },
            BackgroundColor(Color::srgb(0.15, 0.15, 0.15)),
        ))
        .with_children(|parent| {
            // HP label and value
            parent.spawn((
                Text::new("HP: 30 / 30"),
                TextFont {
                    font: font.0.clone(),
                    font_size: 16.0,
                    ..default()
                },
                TextColor(Color::srgb(1.0, 1.0, 0.0)), // Yellow
                HealthText,
            ));

            // Health bar container (background)
            parent
                .spawn((
                    Node {
                        width: Val::Px(200.0),
                        height: Val::Px(16.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.3, 0.0, 0.0)), // Dark red background
                ))
                .with_children(|bar_parent| {
                    // Health bar fill
                    bar_parent.spawn((
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Percent(100.0),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(1.0, 0.0, 0.0)), // Red fill
                        HealthBar,
                    ));
                });

            // Game log container
            parent
                .spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        height: Val::Percent(100.0),
                        flex_grow: 1.0,
                        overflow: Overflow::clip(),
                        ..default()
                    },
                ))
                .with_children(|log_parent| {
                    // Game log text (shows last 5 messages)
                    log_parent.spawn((
                        Text::new(""),
                        TextFont {
                            font: font.0.clone(),
                            font_size: 14.0,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                        GameLogText,
                    ));
                });
        });
}

fn update_health_bar(
    player_query: Query<&CombatStats, With<Player>>,
    mut health_text_query: Query<&mut Text, With<HealthText>>,
    mut health_bar_query: Query<&mut Node, With<HealthBar>>,
) {
    if let Ok(stats) = player_query.get_single() {
        // Update text
        if let Ok(mut text) = health_text_query.get_single_mut() {
            **text = format!("HP: {} / {}", stats.hp, stats.max_hp);
        }

        // Update bar width
        if let Ok(mut node) = health_bar_query.get_single_mut() {
            let percent = (stats.hp as f32 / stats.max_hp as f32) * 100.0;
            node.width = Val::Percent(percent.max(0.0));
        }
    }
}

fn update_game_log(
    game_log: Res<GameLog>,
    mut log_text_query: Query<&mut Text, With<GameLogText>>,
) {
    if let Ok(mut text) = log_text_query.get_single_mut() {
        // Show last 5 messages, newest at bottom
        let messages: Vec<&str> = game_log
            .entries
            .iter()
            .rev()
            .take(5)
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();

        **text = messages.join("\n");
    }
}

fn update_tooltip(
    mut commands: Commands,
    window: Query<&Window>,
    camera_query: Query<(&Camera, &GlobalTransform)>,
    map: Res<Map>,
    font: Res<UiFont>,
    entities_query: Query<(&Position, &Name)>,
    tooltip_query: Query<Entity, With<Tooltip>>,
    highlight_query: Query<Entity, With<CursorHighlight>>,
) {
    // Remove existing tooltip and highlight
    for entity in &tooltip_query {
        commands.entity(entity).despawn();
    }
    for entity in &highlight_query {
        commands.entity(entity).despawn();
    }

    let Ok(window) = window.get_single() else {
        return;
    };

    let Some(cursor_position) = window.cursor_position() else {
        return;
    };

    // Convert screen position to world position
    let Ok((camera, camera_transform)) = camera_query.get_single() else {
        return;
    };

    let Ok(world_pos) = camera.viewport_to_world_2d(camera_transform, cursor_position) else {
        return;
    };

    // Convert world position to map coordinates
    let half_width = window.width() / 2.0;
    let half_height = window.height() / 2.0;

    let map_x = ((world_pos.x + half_width) / GRID_PX.x).floor() as i32;
    let map_y = ((-world_pos.y + half_height) / GRID_PX.y).floor() as i32;

    // Check bounds
    if map_x < 0 || map_x >= MAP_WIDTH as i32 || map_y < 0 || map_y >= MAP_HEIGHT as i32 {
        return;
    }

    // Spawn cursor highlight (magenta background on the tile)
    let half_width = window.width() / 2.0;
    let half_height = window.height() / 2.0;
    commands.spawn((
        Sprite {
            color: Color::srgba(1.0, 0.0, 1.0, 0.3), // Magenta with transparency
            custom_size: Some(Vec2::new(GRID_PX.x, GRID_PX.y)),
            ..default()
        },
        Transform::from_xyz(
            (map_x as f32) * GRID_PX.x + (GRID_PX.x / 2.0) - half_width,
            (map_y as f32) * -GRID_PX.y - (GRID_PX.y / 2.0) + half_height,
            0.5, // Between tiles and entities
        ),
        CursorHighlight,
    ));

    // Check if tile is visible
    let idx = map.xy_idx(map_x, map_y);
    if !map.visible_tiles[idx] {
        return;
    }

    // Find entities at this position
    let mut tooltip_names: Vec<String> = Vec::new();
    for (pos, name) in &entities_query {
        if pos.x == map_x && pos.y == map_y {
            tooltip_names.push(name.name.clone());
        }
    }

    if tooltip_names.is_empty() {
        return;
    }

    // Spawn tooltip
    let tooltip_text = tooltip_names.join(", ");
    let on_right_side = cursor_position.x > window.width() / 2.0;

    commands.spawn((
        Text::new(if on_right_side {
            format!("{} <-", tooltip_text)
        } else {
            format!("-> {}", tooltip_text)
        }),
        TextFont {
            font: font.0.clone(),
            font_size: 14.0,
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            left: if on_right_side {
                Val::Px(cursor_position.x - 150.0)
            } else {
                Val::Px(cursor_position.x + 15.0)
            },
            top: Val::Px(cursor_position.y),
            ..default()
        },
        BackgroundColor(Color::srgb(0.3, 0.3, 0.3)),
        Tooltip,
    ));
}

fn spawn_inventory_menu(
    mut commands: Commands,
    font: Res<UiFont>,
    player_query: Query<Entity, With<Player>>,
    backpack_query: Query<(&InBackpack, &Name), With<Item>>,
) {
    let Ok(player_entity) = player_query.get_single() else {
        return;
    };

    // Collect inventory items
    let inventory: Vec<&str> = backpack_query
        .iter()
        .filter(|(backpack, _)| backpack.owner == player_entity)
        .map(|(_, name)| name.name.as_str())
        .collect();

    // Build inventory text
    let inventory_text = if inventory.is_empty() {
        "Your inventory is empty.\n\n(Press Escape to close)".to_string()
    } else {
        let items: Vec<String> = inventory
            .iter()
            .enumerate()
            .map(|(i, name)| format!("({}) {}", (b'a' + i as u8) as char, name))
            .collect();
        format!(
            "Inventory\n\n{}\n\n(Press Escape to close)",
            items.join("\n")
        )
    };

    // Spawn centered inventory menu
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        InventoryMenu,
    )).with_children(|parent| {
        parent.spawn((
            Node {
                padding: UiRect::all(Val::Px(20.0)),
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            BorderColor(Color::WHITE),
            BackgroundColor(Color::srgb(0.1, 0.1, 0.1)),
        )).with_children(|menu| {
            menu.spawn((
                Text::new(inventory_text),
                TextFont {
                    font: font.0.clone(),
                    font_size: 16.0,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
        });
    });
}

fn despawn_inventory_menu(
    mut commands: Commands,
    menu_query: Query<Entity, With<InventoryMenu>>,
) {
    for entity in &menu_query {
        commands.entity(entity).despawn_recursive();
    }
}

fn handle_inventory_input(
    mut commands: Commands,
    mut evr_kbd: EventReader<KeyboardInput>,
    mut next_state: ResMut<NextState<RunState>>,
    mut targeting_info: ResMut<TargetingInfo>,
    player_query: Query<Entity, With<Player>>,
    backpack_query: Query<(Entity, &InBackpack), With<Item>>,
    ranged_query: Query<&Ranged>,
) {
    let Ok(player_entity) = player_query.get_single() else {
        return;
    };

    // Collect player's items
    let items: Vec<Entity> = backpack_query
        .iter()
        .filter(|(_, backpack)| backpack.owner == player_entity)
        .map(|(entity, _)| entity)
        .collect();

    for ev in evr_kbd.read() {
        if ev.state == ButtonState::Released {
            continue;
        }

        match ev.key_code {
            KeyCode::Escape => {
                next_state.set(RunState::AwaitingInput);
            }
            KeyCode::KeyA => try_use_item(&mut commands, &items, 0, player_entity, &mut next_state, &mut targeting_info, &ranged_query),
            KeyCode::KeyB => try_use_item(&mut commands, &items, 1, player_entity, &mut next_state, &mut targeting_info, &ranged_query),
            KeyCode::KeyC => try_use_item(&mut commands, &items, 2, player_entity, &mut next_state, &mut targeting_info, &ranged_query),
            KeyCode::KeyD => try_use_item(&mut commands, &items, 3, player_entity, &mut next_state, &mut targeting_info, &ranged_query),
            KeyCode::KeyE => try_use_item(&mut commands, &items, 4, player_entity, &mut next_state, &mut targeting_info, &ranged_query),
            KeyCode::KeyF => try_use_item(&mut commands, &items, 5, player_entity, &mut next_state, &mut targeting_info, &ranged_query),
            _ => {}
        }
    }
}

fn try_use_item(
    commands: &mut Commands,
    items: &[Entity],
    index: usize,
    player_entity: Entity,
    next_state: &mut ResMut<NextState<RunState>>,
    targeting_info: &mut ResMut<TargetingInfo>,
    ranged_query: &Query<&Ranged>,
) {
    if let Some(&item) = items.get(index) {
        // Check if item is ranged - if so, enter targeting mode
        if let Ok(ranged) = ranged_query.get(item) {
            targeting_info.range = ranged.range;
            targeting_info.item = Some(item);
            next_state.set(RunState::ShowTargeting);
        } else {
            // Non-ranged item, use immediately
            commands.entity(player_entity).insert(WantsToUseItem { item, target: None });
            next_state.set(RunState::PlayerTurn);
        }
    }
}

fn spawn_drop_menu(
    mut commands: Commands,
    font: Res<UiFont>,
    player_query: Query<Entity, With<Player>>,
    backpack_query: Query<(&InBackpack, &Name), With<Item>>,
) {
    let Ok(player_entity) = player_query.get_single() else {
        return;
    };

    // Collect inventory items
    let inventory: Vec<&str> = backpack_query
        .iter()
        .filter(|(backpack, _)| backpack.owner == player_entity)
        .map(|(_, name)| name.name.as_str())
        .collect();

    // Build drop menu text
    let menu_text = if inventory.is_empty() {
        "Nothing to drop.\n\n(Press Escape to close)".to_string()
    } else {
        let items: Vec<String> = inventory
            .iter()
            .enumerate()
            .map(|(i, name)| format!("({}) {}", (b'a' + i as u8) as char, name))
            .collect();
        format!(
            "Drop which item?\n\n{}\n\n(Press Escape to close)",
            items.join("\n")
        )
    };

    // Spawn centered drop menu
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        DropMenu,
    )).with_children(|parent| {
        parent.spawn((
            Node {
                padding: UiRect::all(Val::Px(20.0)),
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            BorderColor(Color::WHITE),
            BackgroundColor(Color::srgb(0.1, 0.1, 0.1)),
        )).with_children(|menu| {
            menu.spawn((
                Text::new(menu_text),
                TextFont {
                    font: font.0.clone(),
                    font_size: 16.0,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
        });
    });
}

fn despawn_drop_menu(
    mut commands: Commands,
    menu_query: Query<Entity, With<DropMenu>>,
) {
    for entity in &menu_query {
        commands.entity(entity).despawn_recursive();
    }
}

fn handle_drop_input(
    mut commands: Commands,
    mut evr_kbd: EventReader<KeyboardInput>,
    mut next_state: ResMut<NextState<RunState>>,
    player_query: Query<Entity, With<Player>>,
    backpack_query: Query<(Entity, &InBackpack), With<Item>>,
) {
    let Ok(player_entity) = player_query.get_single() else {
        return;
    };

    // Collect player's items
    let items: Vec<Entity> = backpack_query
        .iter()
        .filter(|(_, backpack)| backpack.owner == player_entity)
        .map(|(entity, _)| entity)
        .collect();

    for ev in evr_kbd.read() {
        if ev.state == ButtonState::Released {
            continue;
        }

        match ev.key_code {
            KeyCode::Escape => {
                next_state.set(RunState::AwaitingInput);
            }
            KeyCode::KeyA => try_drop_item(&mut commands, &items, 0, player_entity, &mut next_state),
            KeyCode::KeyB => try_drop_item(&mut commands, &items, 1, player_entity, &mut next_state),
            KeyCode::KeyC => try_drop_item(&mut commands, &items, 2, player_entity, &mut next_state),
            KeyCode::KeyD => try_drop_item(&mut commands, &items, 3, player_entity, &mut next_state),
            KeyCode::KeyE => try_drop_item(&mut commands, &items, 4, player_entity, &mut next_state),
            KeyCode::KeyF => try_drop_item(&mut commands, &items, 5, player_entity, &mut next_state),
            _ => {}
        }
    }
}

fn try_drop_item(
    commands: &mut Commands,
    items: &[Entity],
    index: usize,
    player_entity: Entity,
    next_state: &mut ResMut<NextState<RunState>>,
) {
    if let Some(&item) = items.get(index) {
        commands.entity(player_entity).insert(WantsToDropItem { item });
        next_state.set(RunState::PlayerTurn);
    }
}

fn spawn_targeting_ui(
    mut commands: Commands,
    font: Res<UiFont>,
    targeting_info: Res<TargetingInfo>,
) {
    let range = targeting_info.range;

    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            left: Val::Px(10.0),
            padding: UiRect::all(Val::Px(10.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.1, 0.1, 0.1, 0.9)),
        TargetingMenu,
    )).with_children(|parent| {
        parent.spawn((
            Text::new(format!("Select Target (Range: {})\nClick on a target or press Escape to cancel", range)),
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

        let distance = DistanceAlg::Pythagoras.distance2d(
            Vec2::new(player_pos.x as f32, player_pos.y as f32),
            Vec2::new(monster_pos.x as f32, monster_pos.y as f32),
        );

        if distance <= targeting_info.range as f32 {
            let center_x = (monster_pos.x as f32) * GRID_PX.x + (GRID_PX.x / 2.0) - half_width;
            let center_y = (monster_pos.y as f32) * -GRID_PX.y - (GRID_PX.y / 2.0) + half_height;

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

            let distance = DistanceAlg::Pythagoras.distance2d(
                Vec2::new(player_pos.x as f32, player_pos.y as f32),
                Vec2::new(tile_x as f32, tile_y as f32),
            );

            // Only draw on tiles that are within range
            if distance > range as f32 {
                continue;
            }

            let center_x = (tile_x as f32) * GRID_PX.x + (GRID_PX.x / 2.0) - half_width;
            let center_y = (tile_y as f32) * -GRID_PX.y - (GRID_PX.y / 2.0) + half_height;

            // Check neighbors - only draw border if neighbor is out of range AND is a floor tile
            // (don't draw borders against walls)

            // Top neighbor
            if is_valid_floor(tile_x, tile_y - 1) {
                let top_dist = DistanceAlg::Pythagoras.distance2d(
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
                let bottom_dist = DistanceAlg::Pythagoras.distance2d(
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
                let left_dist = DistanceAlg::Pythagoras.distance2d(
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
                let right_dist = DistanceAlg::Pythagoras.distance2d(
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

    // Convert world position to map coordinates
    let half_width = window.width() / 2.0;
    let half_height = window.height() / 2.0;

    let map_x = ((world_pos.x + half_width) / GRID_PX.x).floor() as i32;
    let map_y = ((-world_pos.y + half_height) / GRID_PX.y).floor() as i32;

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
    let distance = DistanceAlg::Pythagoras.distance2d(
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
            let has_monster = monster_query
                .iter()
                .any(|pos| pos.x == map_x && pos.y == map_y);

            if has_monster {
                commands.spawn((
                    Sprite {
                        color: Color::srgba(0.0, 1.0, 1.0, 0.4), // Cyan with transparency
                        custom_size: Some(Vec2::new(GRID_PX.x, GRID_PX.y)),
                        ..default()
                    },
                    Transform::from_xyz(
                        (map_x as f32) * GRID_PX.x + (GRID_PX.x / 2.0) - half_width,
                        (map_y as f32) * -GRID_PX.y - (GRID_PX.y / 2.0) + half_height,
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

                        if tile_x < 0 || tile_x >= MAP_WIDTH as i32 || tile_y < 0 || tile_y >= MAP_HEIGHT as i32 {
                            continue;
                        }

                        let tile_distance = DistanceAlg::Pythagoras.distance2d(
                            Vec2::new(map_x as f32, map_y as f32),
                            Vec2::new(tile_x as f32, tile_y as f32),
                        );

                        if tile_distance <= radius as f32 {
                            commands.spawn((
                                Sprite {
                                    color: Color::srgba(1.0, 0.5, 0.0, 0.4), // Orange for AoE
                                    custom_size: Some(Vec2::new(GRID_PX.x, GRID_PX.y)),
                                    ..default()
                                },
                                Transform::from_xyz(
                                    (tile_x as f32) * GRID_PX.x + (GRID_PX.x / 2.0) - half_width,
                                    (tile_y as f32) * -GRID_PX.y - (GRID_PX.y / 2.0) + half_height,
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

    // Convert world position to map coordinates
    let half_width = window.width() / 2.0;
    let half_height = window.height() / 2.0;

    let map_x = ((world_pos.x + half_width) / GRID_PX.x).floor() as i32;
    let map_y = ((-world_pos.y + half_height) / GRID_PX.y).floor() as i32;

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
    let distance = DistanceAlg::Pythagoras.distance2d(
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
