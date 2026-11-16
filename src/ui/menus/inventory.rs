use bevy::input::keyboard::KeyboardInput;
use bevy::input::ButtonState;
use bevy::prelude::*;

use crate::abilities::HeroClass;
use crate::components::{Equipped, InBackpack, Item, Name, Ranged, WantsToDropItem, WantsToRemoveItem, WantsToUseItem};
use crate::party::ActiveUnit;
use crate::resources::UiFont;
use crate::ui::TargetingInfo;
use crate::RunState;

use crate::ui::menu::{
    build_menu_text, get_selected_index, handle_pagination_input, ModalMenu,
    ModalMenuBuilder, ModalMenuContainer, ModalMenuText,
};
use crate::ui::resources::MenuPage;

/// Marker for inventory menu (use items)
#[derive(Component)]
pub struct InventoryMenuMarker;

/// Marker for drop menu
#[derive(Component)]
pub struct DropMenuMarker;

/// Marker for remove equipment menu
#[derive(Component)]
pub struct RemoveMenuMarker;

pub struct InventoryPlugin;

impl Plugin for InventoryPlugin {
    fn build(&self, app: &mut App) {
        app
            // Inventory (use items)
            .add_systems(OnEnter(RunState::ShowInventory), spawn_inventory_menu)
            .add_systems(OnExit(RunState::ShowInventory), despawn_inventory_menu)
            .add_systems(
                Update,
                handle_inventory_input.run_if(in_state(RunState::ShowInventory)),
            )
            // Drop items
            .add_systems(OnEnter(RunState::ShowDropItem), spawn_drop_menu)
            .add_systems(OnExit(RunState::ShowDropItem), despawn_drop_menu)
            .add_systems(
                Update,
                handle_drop_input.run_if(in_state(RunState::ShowDropItem)),
            )
            // Remove equipment
            .add_systems(OnEnter(RunState::ShowRemoveItem), spawn_remove_menu)
            .add_systems(OnExit(RunState::ShowRemoveItem), despawn_remove_menu)
            .add_systems(
                Update,
                handle_remove_input.run_if(in_state(RunState::ShowRemoveItem)),
            );
    }
}

// ============================================================================
// Inventory Menu (Use Items)
// ============================================================================

fn spawn_inventory_menu(
    mut commands: Commands,
    font: Res<UiFont>,
    mut menu_page: ResMut<MenuPage>,
    active: Res<ActiveUnit>,
    names: Query<(&Name, Option<&HeroClass>)>,
    backpack_query: Query<(&InBackpack, &Name), With<Item>>,
) {
    menu_page.0 = 0;

    let Some(player_entity) = active.0 else {
        return;
    };

    let items: Vec<String> = backpack_query
        .iter()
        .filter(|(backpack, _)| backpack.owner == player_entity)
        .map(|(_, name)| name.name.clone())
        .collect();

    let menu = ModalMenuBuilder::new(titled("Inventory", names.get(player_entity).ok()))
        .items_with_index(items.iter().map(|s| s.as_str()))
        .paginated()
        .empty_message("Your inventory is empty.")
        .footer("(Press Escape to close)")
        .on_cancel(RunState::AwaitingInput)
        .build();

    commands.spawn((
        InventoryMenuMarker,
        menu.clone(),
    ));

    spawn_menu_ui(&mut commands, &font, &menu, &menu_page);
}

fn despawn_inventory_menu(
    mut commands: Commands,
    menu_query: Query<Entity, With<InventoryMenuMarker>>,
    container_query: Query<Entity, With<ModalMenuContainer>>,
) {
    for entity in &menu_query {
        commands.entity(entity).despawn();
    }
    for entity in &container_query {
        commands.entity(entity).despawn();
    }
}

pub fn handle_inventory_input(
    mut commands: Commands,
    mut evr_kbd: MessageReader<KeyboardInput>,
    mut next_state: ResMut<NextState<RunState>>,
    mut targeting_info: ResMut<TargetingInfo>,
    mut menu_page: ResMut<MenuPage>,
    active: Res<ActiveUnit>,
    backpack_query: Query<(Entity, &InBackpack), With<Item>>,
    ranged_query: Query<&Ranged>,
    menu_query: Query<&ModalMenu, With<InventoryMenuMarker>>,
    mut text_query: Query<&mut Text, With<ModalMenuText>>,
) {
    let Some(player_entity) = active.0 else {
        return;
    };

    let Ok(menu) = menu_query.single() else {
        return;
    };

    let items: Vec<Entity> = backpack_query
        .iter()
        .filter(|(_, backpack)| backpack.owner == player_entity)
        .map(|(entity, _)| entity)
        .collect();

    let total_items = items.len();

    for ev in evr_kbd.read() {
        if ev.state != ButtonState::Pressed {
            continue;
        }

        // Handle escape
        if ev.key_code == KeyCode::Escape {
            next_state.set_if_different(RunState::AwaitingInput);
            return;
        }

        // Handle pagination
        if handle_pagination_input(ev.key_code, &mut menu_page, total_items) {
            // Update text in place
            if let Ok(mut text) = text_query.single_mut() {
                **text = build_menu_text(menu, &menu_page);
            }
            continue;
        }

        // Handle item selection
        if let Some(index) = get_selected_index(ev.key_code, &menu_page, total_items) {
            if let Some(&item) = items.get(index) {
                // Check if item is ranged - if so, enter targeting mode
                if let Ok(ranged) = ranged_query.get(item) {
                    targeting_info.range = ranged.range;
                    targeting_info.item = Some(item);
                    next_state.set_if_different(RunState::ShowTargeting);
                } else {
                    // Non-ranged item, use immediately
                    commands.entity(player_entity).insert(WantsToUseItem { item, target: None });
                    next_state.set_if_different(RunState::PartyAction);
                }
            }
        }
    }
}

// ============================================================================
// Drop Menu
// ============================================================================

fn spawn_drop_menu(
    mut commands: Commands,
    font: Res<UiFont>,
    mut menu_page: ResMut<MenuPage>,
    active: Res<ActiveUnit>,
    names: Query<(&Name, Option<&HeroClass>)>,
    backpack_query: Query<(&InBackpack, &Name), With<Item>>,
) {
    menu_page.0 = 0;

    let Some(player_entity) = active.0 else {
        return;
    };

    let items: Vec<String> = backpack_query
        .iter()
        .filter(|(backpack, _)| backpack.owner == player_entity)
        .map(|(_, name)| name.name.clone())
        .collect();

    let menu = ModalMenuBuilder::new(titled("Drop which item?", names.get(player_entity).ok()))
        .items_with_index(items.iter().map(|s| s.as_str()))
        .paginated()
        .empty_message("Nothing to drop.")
        .footer("(Press Escape to close)")
        .on_cancel(RunState::AwaitingInput)
        .build();

    commands.spawn((
        DropMenuMarker,
        menu.clone(),
    ));

    spawn_menu_ui(&mut commands, &font, &menu, &menu_page);
}

fn despawn_drop_menu(
    mut commands: Commands,
    menu_query: Query<Entity, With<DropMenuMarker>>,
    container_query: Query<Entity, With<ModalMenuContainer>>,
) {
    for entity in &menu_query {
        commands.entity(entity).despawn();
    }
    for entity in &container_query {
        commands.entity(entity).despawn();
    }
}

fn handle_drop_input(
    mut commands: Commands,
    mut evr_kbd: MessageReader<KeyboardInput>,
    mut next_state: ResMut<NextState<RunState>>,
    mut menu_page: ResMut<MenuPage>,
    active: Res<ActiveUnit>,
    backpack_query: Query<(Entity, &InBackpack), With<Item>>,
    menu_query: Query<&ModalMenu, With<DropMenuMarker>>,
    mut text_query: Query<&mut Text, With<ModalMenuText>>,
) {
    let Some(player_entity) = active.0 else {
        return;
    };

    let Ok(menu) = menu_query.single() else {
        return;
    };

    let items: Vec<Entity> = backpack_query
        .iter()
        .filter(|(_, backpack)| backpack.owner == player_entity)
        .map(|(entity, _)| entity)
        .collect();

    let total_items = items.len();

    for ev in evr_kbd.read() {
        if ev.state != ButtonState::Pressed {
            continue;
        }

        // Handle escape
        if ev.key_code == KeyCode::Escape {
            next_state.set_if_different(RunState::AwaitingInput);
            return;
        }

        // Handle pagination
        if handle_pagination_input(ev.key_code, &mut menu_page, total_items) {
            if let Ok(mut text) = text_query.single_mut() {
                **text = build_menu_text(menu, &menu_page);
            }
            continue;
        }

        // Handle item selection
        if let Some(index) = get_selected_index(ev.key_code, &menu_page, total_items) {
            if let Some(&item) = items.get(index) {
                commands.entity(player_entity).insert(WantsToDropItem { item });
                next_state.set_if_different(RunState::PartyAction);
            }
        }
    }
}

// ============================================================================
// Remove Equipment Menu
// ============================================================================

fn spawn_remove_menu(
    mut commands: Commands,
    font: Res<UiFont>,
    mut menu_page: ResMut<MenuPage>,
    active: Res<ActiveUnit>,
    names: Query<(&Name, Option<&HeroClass>)>,
    equipped_query: Query<(&Equipped, &Name), With<Item>>,
) {
    menu_page.0 = 0;

    let Some(player_entity) = active.0 else {
        return;
    };

    let items: Vec<String> = equipped_query
        .iter()
        .filter(|(equipped, _)| equipped.owner == player_entity)
        .map(|(_, name)| name.name.clone())
        .collect();

    let menu = ModalMenuBuilder::new(titled("Remove which item?", names.get(player_entity).ok()))
        .items_with_index(items.iter().map(|s| s.as_str()))
        .paginated()
        .empty_message("No equipment to remove.")
        .footer("(Press Escape to close)")
        .on_cancel(RunState::AwaitingInput)
        .build();

    commands.spawn((
        RemoveMenuMarker,
        menu.clone(),
    ));

    spawn_menu_ui(&mut commands, &font, &menu, &menu_page);
}

fn despawn_remove_menu(
    mut commands: Commands,
    menu_query: Query<Entity, With<RemoveMenuMarker>>,
    container_query: Query<Entity, With<ModalMenuContainer>>,
) {
    for entity in &menu_query {
        commands.entity(entity).despawn();
    }
    for entity in &container_query {
        commands.entity(entity).despawn();
    }
}

fn handle_remove_input(
    mut commands: Commands,
    mut evr_kbd: MessageReader<KeyboardInput>,
    mut next_state: ResMut<NextState<RunState>>,
    mut menu_page: ResMut<MenuPage>,
    active: Res<ActiveUnit>,
    equipped_query: Query<(Entity, &Equipped), With<Item>>,
    menu_query: Query<&ModalMenu, With<RemoveMenuMarker>>,
    mut text_query: Query<&mut Text, With<ModalMenuText>>,
) {
    let Some(player_entity) = active.0 else {
        return;
    };

    let Ok(menu) = menu_query.single() else {
        return;
    };

    let items: Vec<Entity> = equipped_query
        .iter()
        .filter(|(_, equipped)| equipped.owner == player_entity)
        .map(|(entity, _)| entity)
        .collect();

    let total_items = items.len();

    for ev in evr_kbd.read() {
        if ev.state != ButtonState::Pressed {
            continue;
        }

        // Handle escape
        if ev.key_code == KeyCode::Escape {
            next_state.set_if_different(RunState::AwaitingInput);
            return;
        }

        // Handle pagination
        if handle_pagination_input(ev.key_code, &mut menu_page, total_items) {
            if let Ok(mut text) = text_query.single_mut() {
                **text = build_menu_text(menu, &menu_page);
            }
            continue;
        }

        // Handle item selection
        if let Some(index) = get_selected_index(ev.key_code, &menu_page, total_items) {
            if let Some(&item) = items.get(index) {
                commands.entity(player_entity).insert(WantsToRemoveItem { item });
                next_state.set_if_different(RunState::PartyAction);
            }
        }
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// A menu title naming the unit it acts for.
/// "Aldric the Fighter: Inventory".
fn titled(title: &str, unit: Option<(&Name, Option<&HeroClass>)>) -> String {
    match unit {
        Some((name, Some(class))) => format!("{} the {}: {title}", name.name, class.0),
        Some((name, None)) => format!("{}: {title}", name.name),
        None => title.to_string(),
    }
}

fn spawn_menu_ui(commands: &mut Commands, font: &UiFont, menu: &ModalMenu, menu_page: &MenuPage) {
    let text_content = build_menu_text(menu, menu_page);
    let style = &menu.style;

    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            ModalMenuContainer,
        ))
        .with_children(|parent| {
            parent
                .spawn((
                    Node {
                        padding: UiRect::all(Val::Px(style.padding)),
                        border: UiRect::all(Val::Px(2.0)),
                        ..default()
                    },
                    BorderColor::all(style.border_color),
                    BackgroundColor(style.background_color),
                ))
                .with_children(|inner| {
                    inner.spawn((
                        Text::new(text_content),
                        TextFont {
                            font: font.0.clone().into(),
                            font_size: FontSize::Px(style.font_size),
                            ..default()
                        },
                        TextColor(style.text_color),
                        ModalMenuText,
                    ));
                });
        });
}
