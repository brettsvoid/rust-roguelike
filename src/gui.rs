use bevy::input::keyboard::KeyboardInput;
use bevy::input::ButtonState;
use bevy::prelude::*;

use crate::combat::CombatStats;
use crate::components::{InBackpack, Item, Name, Potion, WantsToDrinkPotion, WantsToDropItem};
use crate::gamelog::GameLog;
use crate::map::{Map, Position, GRID_PX, MAP_HEIGHT, MAP_WIDTH};
use crate::player::Player;
use crate::resources::UiFont;
use crate::RunState;

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
    player_query: Query<Entity, With<Player>>,
    backpack_query: Query<(Entity, &InBackpack), With<Potion>>,
) {
    let Ok(player_entity) = player_query.get_single() else {
        return;
    };

    // Collect player's potions
    let potions: Vec<Entity> = backpack_query
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
            KeyCode::KeyA => try_use_potion(&mut commands, &potions, 0, player_entity, &mut next_state),
            KeyCode::KeyB => try_use_potion(&mut commands, &potions, 1, player_entity, &mut next_state),
            KeyCode::KeyC => try_use_potion(&mut commands, &potions, 2, player_entity, &mut next_state),
            KeyCode::KeyD => try_use_potion(&mut commands, &potions, 3, player_entity, &mut next_state),
            KeyCode::KeyE => try_use_potion(&mut commands, &potions, 4, player_entity, &mut next_state),
            KeyCode::KeyF => try_use_potion(&mut commands, &potions, 5, player_entity, &mut next_state),
            _ => {}
        }
    }
}

fn try_use_potion(
    commands: &mut Commands,
    potions: &[Entity],
    index: usize,
    player_entity: Entity,
    next_state: &mut ResMut<NextState<RunState>>,
) {
    if let Some(&potion) = potions.get(index) {
        commands.entity(player_entity).insert(WantsToDrinkPotion { potion });
        next_state.set(RunState::PlayerTurn);
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
