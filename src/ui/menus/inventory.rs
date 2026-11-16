//! The inventory (use an item), drop and remove-equipment menus: one panel,
//! filled from the active hero's backpack or worn gear. A letter, a click on
//! the row or Enter on the picked row acts on that item.

use bevy::input::keyboard::KeyboardInput;
use bevy::input::ButtonState;
use bevy::prelude::*;

use crate::abilities::HeroClass;
use crate::components::{Equipped, InBackpack, Item, Name, Ranged, WantsToDropItem, WantsToRemoveItem, WantsToUseItem};
use crate::party::ActiveUnit;
use crate::ui::TargetingInfo;
use crate::RunState;

use crate::ui::panel::{
    close_panels, get_selected_index, handle_pagination_input, page_hints, page_of, show, spawn_panel, Hint, Panel,
    PanelSpec, Placement, Row,
};
use crate::ui::resources::MenuPage;

pub struct InventoryPlugin;

impl Plugin for InventoryPlugin {
    fn build(&self, app: &mut App) {
        let in_menu = in_state(RunState::ShowInventory)
            .or_else(in_state(RunState::ShowDropItem))
            .or_else(in_state(RunState::ShowRemoveItem));
        for state in [RunState::ShowInventory, RunState::ShowDropItem, RunState::ShowRemoveItem] {
            app.add_systems(OnEnter(state), open_item_menu).add_systems(OnExit(state), close_panels);
        }
        app.add_systems(
            Update,
            (
                handle_inventory_input.run_if(in_state(RunState::ShowInventory)),
                handle_drop_input.run_if(in_state(RunState::ShowDropItem)),
                handle_remove_input.run_if(in_state(RunState::ShowRemoveItem)),
                refresh_item_menu.run_if(in_menu),
            ),
        );
    }
}

/// Which of the three menus is open.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ItemMenu {
    Use,
    Drop,
    Remove,
}

impl ItemMenu {
    fn of(state: &RunState) -> Option<Self> {
        match state {
            RunState::ShowInventory => Some(ItemMenu::Use),
            RunState::ShowDropItem => Some(ItemMenu::Drop),
            RunState::ShowRemoveItem => Some(ItemMenu::Remove),
            _ => None,
        }
    }
}

/// The menu for `items` (names, in the order the handlers see them).
fn item_menu_spec(menu: ItemMenu, hero: &str, items: &[String], page: usize) -> PanelSpec {
    let (title, verb, empty) = match menu {
        ItemMenu::Use => ("Inventory", "Use", "The pack is empty."),
        ItemMenu::Drop => ("Drop an item", "Drop", "Nothing to drop."),
        ItemMenu::Remove => ("Remove equipment", "Remove", "Nothing worn to remove."),
    };
    let (shown, page) = page_of(items, page);
    let mut hints = Vec::new();
    if !shown.is_empty() {
        hints.push(Hint::rows(shown.len(), verb));
    }
    hints.extend(page_hints(page));
    hints.push(Hint::close());
    PanelSpec {
        title: title.into(),
        aside: hero.into(),
        width: 440.0,
        rows: shown.into_iter().map(|label| Row { label, detail: String::new() }).collect(),
        empty: empty.into(),
        page,
        hints,
        ..default()
    }
}

/// "Aldric, Fighter".
fn hero_title(unit: Option<(&Name, Option<&HeroClass>)>) -> String {
    match unit {
        Some((name, Some(class))) => format!("{}, {}", name.name, class.0),
        Some((name, None)) => name.name.clone(),
        None => String::new(),
    }
}

/// The hero's backpack, or worn gear for the remove menu, in a fixed order.
fn items_of(
    menu: ItemMenu,
    hero: Entity,
    backpack: &Query<(Entity, &InBackpack, &Name), With<Item>>,
    equipped: &Query<(Entity, &Equipped, &Name), With<Item>>,
) -> Vec<(Entity, String)> {
    let mut items: Vec<(Entity, String)> = match menu {
        ItemMenu::Remove => equipped.iter().filter(|(_, e, _)| e.owner == hero).map(|(i, _, n)| (i, n.name.clone())).collect(),
        _ => backpack.iter().filter(|(_, b, _)| b.owner == hero).map(|(i, _, n)| (i, n.name.clone())).collect(),
    };
    items.sort_by_key(|(entity, _)| *entity);
    items
}

fn open_item_menu(mut commands: Commands, mut menu_page: ResMut<MenuPage>) {
    menu_page.0 = 0;
    spawn_panel(&mut commands, Placement::Modal);
}

fn refresh_item_menu(
    state: Res<State<RunState>>,
    menu_page: Res<MenuPage>,
    active: Res<ActiveUnit>,
    names: Query<(&Name, Option<&HeroClass>)>,
    backpack: Query<(Entity, &InBackpack, &Name), With<Item>>,
    equipped: Query<(Entity, &Equipped, &Name), With<Item>>,
    mut panels: Query<(&Panel, &mut PanelSpec)>,
) {
    let (Some(menu), Some(hero)) = (ItemMenu::of(state.get()), active.0) else {
        return;
    };
    let items: Vec<String> = items_of(menu, hero, &backpack, &equipped).into_iter().map(|(_, n)| n).collect();
    let title = hero_title(names.get(hero).ok());
    show(&mut panels, item_menu_spec(menu, &title, &items, menu_page.0));
}

/// The item a key picks, after Escape and page turns are dealt with.
fn picked(
    evr_kbd: &mut MessageReader<KeyboardInput>,
    next_state: &mut NextState<RunState>,
    menu_page: &mut MenuPage,
    items: &[(Entity, String)],
) -> Option<Entity> {
    for ev in evr_kbd.read() {
        if ev.state != ButtonState::Pressed {
            continue;
        }
        if ev.key_code == KeyCode::Escape {
            next_state.set_if_different(RunState::AwaitingInput);
            return None;
        }
        if handle_pagination_input(ev.key_code, menu_page, items.len()) {
            continue;
        }
        if let Some(index) = get_selected_index(ev.key_code, menu_page, items.len()) {
            return Some(items[index].0);
        }
    }
    None
}

pub fn handle_inventory_input(
    mut commands: Commands,
    mut evr_kbd: MessageReader<KeyboardInput>,
    mut next_state: ResMut<NextState<RunState>>,
    mut targeting_info: ResMut<TargetingInfo>,
    mut menu_page: ResMut<MenuPage>,
    active: Res<ActiveUnit>,
    backpack: Query<(Entity, &InBackpack, &Name), With<Item>>,
    equipped: Query<(Entity, &Equipped, &Name), With<Item>>,
    ranged_query: Query<&Ranged>,
) {
    let Some(hero) = active.0 else {
        return;
    };
    let items = items_of(ItemMenu::Use, hero, &backpack, &equipped);
    let Some(item) = picked(&mut evr_kbd, &mut next_state, &mut menu_page, &items) else {
        return;
    };
    // A ranged item is aimed first; anything else is used at once.
    if let Ok(ranged) = ranged_query.get(item) {
        targeting_info.range = ranged.range;
        targeting_info.item = Some(item);
        next_state.set_if_different(RunState::ShowTargeting);
    } else {
        commands.entity(hero).insert(WantsToUseItem { item, target: None });
        next_state.set_if_different(RunState::PartyAction);
    }
}

fn handle_drop_input(
    mut commands: Commands,
    mut evr_kbd: MessageReader<KeyboardInput>,
    mut next_state: ResMut<NextState<RunState>>,
    mut menu_page: ResMut<MenuPage>,
    active: Res<ActiveUnit>,
    backpack: Query<(Entity, &InBackpack, &Name), With<Item>>,
    equipped: Query<(Entity, &Equipped, &Name), With<Item>>,
) {
    let Some(hero) = active.0 else {
        return;
    };
    let items = items_of(ItemMenu::Drop, hero, &backpack, &equipped);
    if let Some(item) = picked(&mut evr_kbd, &mut next_state, &mut menu_page, &items) {
        commands.entity(hero).insert(WantsToDropItem { item });
        next_state.set_if_different(RunState::PartyAction);
    }
}

fn handle_remove_input(
    mut commands: Commands,
    mut evr_kbd: MessageReader<KeyboardInput>,
    mut next_state: ResMut<NextState<RunState>>,
    mut menu_page: ResMut<MenuPage>,
    active: Res<ActiveUnit>,
    backpack: Query<(Entity, &InBackpack, &Name), With<Item>>,
    equipped: Query<(Entity, &Equipped, &Name), With<Item>>,
) {
    let Some(hero) = active.0 else {
        return;
    };
    let items = items_of(ItemMenu::Remove, hero, &backpack, &equipped);
    if let Some(item) = picked(&mut evr_kbd, &mut next_state, &mut menu_page, &items) {
        commands.entity(hero).insert(WantsToRemoveItem { item });
        next_state.set_if_different(RunState::PartyAction);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::panel::BodyLine;

    #[test]
    fn the_inventory_lists_the_items_with_their_keys_and_hints() {
        let items = vec!["Health Potion".to_string(), "Dagger".to_string()];
        let spec = item_menu_spec(ItemMenu::Use, "Aldric, Fighter", &items, 0);
        assert_eq!(spec.title, "Inventory");
        assert_eq!(spec.aside, "Aldric, Fighter");
        assert_eq!(spec.rows.iter().map(|r| r.label.as_str()).collect::<Vec<_>>(), ["Health Potion", "Dagger"]);
        let hints: Vec<_> = spec.hints.iter().map(|h| (h.cap.as_str(), h.action.as_str())).collect();
        assert_eq!(hints, [("a-b", "Use"), ("Esc", "Close")]);
        assert_eq!(spec.body, Vec::<BodyLine>::new());
    }

    #[test]
    fn a_long_pack_pages_and_an_empty_one_says_so() {
        let items: Vec<String> = (0..12).map(|i| format!("Item {i}")).collect();
        let spec = item_menu_spec(ItemMenu::Drop, "", &items, 1);
        assert_eq!(spec.rows.len(), 2);
        assert_eq!(spec.page, Some((1, 2)));
        assert!(spec.hints.iter().any(|h| h.action == "Next"));
        let empty = item_menu_spec(ItemMenu::Remove, "", &[], 0);
        assert!(empty.rows.is_empty());
        assert_eq!(empty.empty, "Nothing worn to remove.");
        assert_eq!(empty.hints.len(), 1);
    }
}
