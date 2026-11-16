//! The shop screen (E02). Clicking a vendor in town walks the active hero
//! beside them and opens it. Tab switches between buying from the vendor and
//! selling from the active hero's backpack (and, at the barkeep, hiring a
//! recruit into a fallen hero's slot), a letter trades that line and Escape
//! closes it. The trade rules are `economy::shop`; this module moves the item
//! entities and spawns the hired hero.

use std::collections::HashSet;

use bevy::input::keyboard::KeyboardInput;
use bevy::input::ButtonState;
use bevy::prelude::*;

use crate::bystander::Bystander;
use crate::components::{BlocksTile, Door, InBackpack, Item, Name};
use crate::debug::DebugMode;
use crate::economy::shop::{self, buys, recruits, Recruit};
use crate::economy::town::{TownState, Vendor};
use crate::economy::trade::{buy_price, sell_price};
use crate::economy::Purse;
use crate::gamelog::GameLog;
use crate::map::{Map, Position};
use crate::party::{spawn_tiles, ActiveUnit, PartyMember};
use crate::picking::{self, HoveredTile};
use crate::pathfinding::a_star_explored_avoiding;
use crate::player::{self, WalkPath};
use crate::raws::{self, RAWS};
use crate::spawner::spawn_recruit;
use crate::RunState;

use crate::ui::panel::{
    close_panels, get_selected_index, handle_pagination_input, page_hints, page_of, show, spawn_panel, Hint, Panel,
    PanelSpec, Placement, Row, Tab,
};
use crate::ui::resources::MenuPage;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ShopMode {
    Buy,
    Sell,
    Hire,
}

impl ShopMode {
    /// The mode Tab switches to: the barkeep also hires.
    fn next(self, vendor: Vendor) -> Self {
        match self {
            ShopMode::Buy => ShopMode::Sell,
            ShopMode::Sell if vendor == Vendor::Barkeep => ShopMode::Hire,
            _ => ShopMode::Buy,
        }
    }

    /// The tabs a vendor's shop has, in Tab order.
    fn tabs(vendor: Vendor) -> &'static [ShopMode] {
        if vendor == Vendor::Barkeep {
            &[ShopMode::Buy, ShopMode::Sell, ShopMode::Hire]
        } else {
            &[ShopMode::Buy, ShopMode::Sell]
        }
    }

    /// How many Tab presses it takes from `self` to `to`.
    fn presses_to(self, to: ShopMode, vendor: Vendor) -> usize {
        let tabs = Self::tabs(vendor);
        let at = |m| tabs.iter().position(|&t| t == m).unwrap_or(0);
        (at(to) + tabs.len() - at(self)) % tabs.len()
    }
}

/// The open shop: whose it is, which way the trade goes and the last result.
#[derive(Resource, Debug)]
pub struct Shop {
    pub vendor: Vendor,
    pub mode: ShopMode,
    pub message: String,
}

impl Shop {
    pub fn open(vendor: Vendor) -> Self {
        Shop {
            vendor,
            mode: ShopMode::Buy,
            message: String::new(),
        }
    }
}

impl Default for Shop {
    fn default() -> Self {
        Shop::open(Vendor::Blacksmith)
    }
}

pub struct ShopPlugin;

impl Plugin for ShopPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Shop>()
            .add_systems(OnEnter(RunState::ShowShop), spawn_shop)
            .add_systems(OnExit(RunState::ShowShop), close_panels)
            .add_systems(
                Update,
                (
                    click_vendor
                        .run_if(in_state(RunState::AwaitingInput))
                        .after(picking::update_hovered_tile)
                        .before(player::handle_player_input),
                    (handle_shop_input, refresh_shop)
                        .chain()
                        .run_if(in_state(RunState::ShowShop)),
                ),
            );
    }
}

/// A click on an open vendor walks the active hero beside them, then opens
/// their shop; from beside them it opens at once. The click is taken, so
/// the player's own input does not also walk onto the vendor.
#[allow(clippy::too_many_arguments)]
pub fn click_vendor(
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    hovered: Res<HoveredTile>,
    debug_mode: Res<DebugMode>,
    map: Res<Map>,
    town: Res<TownState>,
    active: Res<ActiveUnit>,
    mut walk: ResMut<WalkPath>,
    mut shop: ResMut<Shop>,
    mut next_state: ResMut<NextState<RunState>>,
    party: Query<&Position, With<PartyMember>>,
    vendors: Query<(&Position, &Name), With<Bystander>>,
    obstacles: Query<&Position, (With<BlocksTile>, Without<Door>, Without<Bystander>, Without<PartyMember>)>,
) {
    let Some(from) = active.0.and_then(|unit| party.get(unit).ok()).map(|p| (p.x, p.y)) else {
        return;
    };
    let mut open = |vendor| {
        *shop = Shop::open(vendor);
        next_state.set_if_different(RunState::ShowShop);
    };
    if walk.cells.is_empty() && walk.trade_with.is_some() {
        open(walk.trade_with.take().unwrap());
        return;
    }
    if debug_mode.show_console || !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    let Some(target) = hovered.tile else {
        return;
    };
    let Some(vendor) = vendors
        .iter()
        .find(|(pos, _)| (pos.x, pos.y) == target)
        .and_then(|(_, name)| Vendor::from_name(&name.name))
        .filter(|vendor| town.vendor_stock.contains_key(vendor))
    else {
        return;
    };
    mouse.clear_just_pressed(MouseButton::Left);
    *walk = WalkPath::default();
    if (target.0 - from.0).abs() <= 1 && (target.1 - from.1).abs() <= 1 {
        open(vendor);
        return;
    }
    // Walk round fixed obstacles, such as a stall's counter, to the nearest
    // free cell beside them. Doors open and people swap places on the way.
    let obstacles: HashSet<usize> = obstacles.iter().map(|p| map.xy_idx(p.x, p.y)).collect();
    let start = map.xy_idx(from.0, from.1);
    let path = (-1..=1)
        .flat_map(|dx| (-1..=1).map(move |dy| (target.0 + dx, target.1 + dy)))
        .filter(|&(x, y)| (0..map.width).contains(&x) && (0..map.height).contains(&y))
        .map(|(x, y)| map.xy_idx(x, y))
        .filter(|&end| !map.blocked_tiles[end])
        .filter_map(|end| a_star_explored_avoiding(&map, start, end, |i| obstacles.contains(&i)))
        .min_by_key(Vec::len);
    if let Some(path) = path {
        let width = map.width as usize;
        walk.cells = path.into_iter().skip(1).map(|i| ((i % width) as i32, (i / width) as i32)).collect();
        walk.trade_with = Some(vendor);
    }
}

/// What a line trades.
enum Deal {
    Buy(raws::Item),
    /// The raws item and its backpack entity.
    Sell(raws::Item, Entity),
    Hire(Recruit),
}

/// One tradeable line and the row shown.
struct Line {
    deal: Deal,
    row: Row,
}

/// The vendor's shelf when buying, what the vendor will buy from the hero's
/// backpack when selling, or the recruits for hire, each with its price now.
fn shop_lines(
    shop: &Shop,
    town: &TownState,
    hero: Entity,
    backpack: &Query<(Entity, &InBackpack, &Name), With<Item>>,
    party: &Query<(&PartyMember, &Name, &Position)>,
) -> Vec<Line> {
    let raws = RAWS.lock().unwrap();
    let find = |name: &str| raws.item_index.get(name).map(|&i| raws.raws.items[i].clone());
    match shop.mode {
        ShopMode::Buy => town
            .vendor_stock
            .get(&shop.vendor)
            .into_iter()
            .flatten()
            .filter_map(|(name, &left)| {
                let item = find(name)?;
                let price = buy_price(town, item.sector?, item.base_value);
                Some(Line {
                    row: Row { label: name.clone(), detail: format!("{price} gold   {left} left") },
                    deal: Deal::Buy(item),
                })
            })
            .collect(),
        ShopMode::Sell => backpack
            .iter()
            .filter(|(_, pack, _)| pack.owner == hero)
            .filter_map(|(entity, _, name)| {
                let item = find(&name.name).filter(|item| buys(shop.vendor, item))?;
                let price = sell_price(town, item.sector?, item.base_value);
                Some(Line {
                    row: Row { label: name.name.clone(), detail: format!("{price} gold") },
                    deal: Deal::Sell(item, entity),
                })
            })
            .collect(),
        ShopMode::Hire => {
            let names: Vec<&str> = party.iter().map(|(_, name, _)| name.name.as_str()).collect();
            recruits(town, &names)
                .into_iter()
                .map(|recruit| Line {
                    row: Row {
                        label: format!("{}, {} level {}", recruit.name, recruit.class, recruit.level),
                        detail: format!("{} gold", recruit.price),
                    },
                    deal: Deal::Hire(recruit),
                })
                .collect()
        }
    }
}

fn shop_spec(shop: &Shop, gold: i32, hero: &str, rows: &[Row], page: usize) -> PanelSpec {
    let vendor = shop.vendor.name();
    let tabs = ShopMode::tabs(shop.vendor)
        .iter()
        .map(|&mode| Tab {
            label: match mode {
                ShopMode::Buy => "Buy".to_string(),
                ShopMode::Sell => format!("Sell from {hero}'s pack"),
                ShopMode::Hire => "Hire".to_string(),
            },
            active: mode == shop.mode,
            keys: vec![KeyCode::Tab; shop.mode.presses_to(mode, shop.vendor)],
        })
        .collect();
    let (empty, verb) = match shop.mode {
        ShopMode::Buy => (format!("The {} has nothing left to sell.", vendor.to_lowercase()), "Buy"),
        ShopMode::Sell => (format!("Nothing in {hero}'s pack that the {} buys.", vendor.to_lowercase()), "Sell"),
        ShopMode::Hire => ("Nobody is looking for work.".to_string(), "Hire"),
    };
    let (shown, page) = page_of(rows, page);
    let mut hints = Vec::new();
    if !shown.is_empty() {
        hints.push(Hint::rows(shown.len(), verb));
    }
    hints.push(Hint::new("Tab", "Next tab", KeyCode::Tab));
    hints.extend(page_hints(page));
    hints.push(Hint::close());
    PanelSpec {
        title: vendor.to_string(),
        aside: format!("Purse {gold} gold"),
        width: 560.0,
        tabs,
        rows: shown,
        empty,
        message: shop.message.clone(),
        page,
        hints,
        ..default()
    }
}

/// A bought item, made real and put straight into the hero's backpack.
fn give_item(commands: &mut Commands, name: &str, hero: Entity) {
    let raws = RAWS.lock().unwrap();
    if let Some(item) = raws::spawn_named_item(&raws, commands, name, 0, 0) {
        commands
            .entity(item)
            .remove::<Position>()
            .insert((InBackpack { owner: hero }, Visibility::Hidden));
    }
}

/// The panel is filled in by `refresh_shop` the same frame.
fn spawn_shop(mut commands: Commands, mut menu_page: ResMut<MenuPage>) {
    menu_page.0 = 0;
    spawn_panel(&mut commands, Placement::Modal);
}

/// One trade per frame, so the next is chosen from the updated lists.
#[allow(clippy::too_many_arguments)]
pub fn handle_shop_input(
    mut commands: Commands,
    mut evr_kbd: MessageReader<KeyboardInput>,
    mut next_state: ResMut<NextState<RunState>>,
    mut shop: ResMut<Shop>,
    mut menu_page: ResMut<MenuPage>,
    mut town: ResMut<TownState>,
    mut purse: ResMut<Purse>,
    mut gamelog: ResMut<GameLog>,
    active: Res<ActiveUnit>,
    map: Res<Map>,
    backpack: Query<(Entity, &InBackpack, &Name), With<Item>>,
    party: Query<(&PartyMember, &Name, &Position)>,
) {
    let Some(hero) = active.0 else {
        return;
    };
    for ev in evr_kbd.read() {
        if ev.state != ButtonState::Pressed {
            continue;
        }
        match ev.key_code {
            KeyCode::Escape => {
                next_state.set_if_different(RunState::AwaitingInput);
                return;
            }
            KeyCode::Tab => {
                shop.mode = shop.mode.next(shop.vendor);
                shop.message.clear();
                menu_page.0 = 0;
            }
            key => {
                let lines = shop_lines(&shop, &town, hero, &backpack, &party);
                if handle_pagination_input(key, &mut menu_page, lines.len()) {
                    continue;
                }
                let Some(line) = get_selected_index(key, &menu_page, lines.len()).map(|i| &lines[i])
                else {
                    continue;
                };
                let vendor = shop.vendor;
                let result = match &line.deal {
                    Deal::Buy(item) => shop::buy(&mut town, &mut purse.gold, vendor, item)
                        .inspect(|_| give_item(&mut commands, &item.name, hero)),
                    Deal::Sell(item, entity) => shop::sell(&mut town, &mut purse.gold, vendor, item)
                        .inspect(|_| commands.entity(*entity).despawn()),
                    Deal::Hire(recruit) => {
                        let taken: Vec<usize> = party.iter().map(|(member, ..)| member.slot).collect();
                        let from = party.get(hero).map_or((0, 0), |(_, _, p)| (p.x, p.y));
                        shop::hire(&mut purse.gold, &taken, recruit).map(|(slot, message)| {
                            let at = beside(&map, from, &party);
                            spawn_recruit(&mut commands, slot, at, recruit.name, recruit.class, recruit.level);
                            message
                        })
                    }
                };
                shop.message = result.unwrap_or_else(|refusal| refusal);
                gamelog.entries.push(shop.message.clone());
                return;
            }
        }
    }
}

/// The nearest free walkable cell beside `from` for a recruit to stand on.
fn beside(map: &Map, from: (i32, i32), party: &Query<(&PartyMember, &Name, &Position)>) -> (i32, i32) {
    spawn_tiles(map, from, 25)
        .into_iter()
        .skip(1)
        .find(|&(x, y)| !map.blocked_tiles[map.xy_idx(x, y)] && !party.iter().any(|(_, _, p)| (p.x, p.y) == (x, y)))
        .unwrap_or(from)
}

fn refresh_shop(
    shop: Res<Shop>,
    town: Res<TownState>,
    purse: Res<Purse>,
    menu_page: Res<MenuPage>,
    active: Res<ActiveUnit>,
    names: Query<&Name>,
    backpack: Query<(Entity, &InBackpack, &Name), With<Item>>,
    party: Query<(&PartyMember, &Name, &Position)>,
    mut panels: Query<(&Panel, &mut PanelSpec)>,
) {
    let Some(hero) = active.0 else {
        return;
    };
    let hero_name = names.get(hero).map_or("Hero", |n| n.name.as_str());
    let rows: Vec<Row> = shop_lines(&shop, &town, hero, &backpack, &party).into_iter().map(|line| line.row).collect();
    show(&mut panels, shop_spec(&shop, purse.gold, hero_name, &rows, menu_page.0));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tabs_count_the_presses_to_reach_them() {
        assert_eq!(ShopMode::Buy.presses_to(ShopMode::Sell, Vendor::Blacksmith), 1);
        assert_eq!(ShopMode::Sell.presses_to(ShopMode::Buy, Vendor::Blacksmith), 1);
        assert_eq!(ShopMode::Buy.presses_to(ShopMode::Hire, Vendor::Barkeep), 2);
        assert_eq!(ShopMode::Hire.presses_to(ShopMode::Sell, Vendor::Barkeep), 2);
        assert_eq!(ShopMode::Sell.presses_to(ShopMode::Sell, Vendor::Barkeep), 0);
    }

    #[test]
    fn the_shop_panel_shows_the_purse_tabs_rows_and_last_trade() {
        let mut shop = Shop::open(Vendor::Barkeep);
        shop.mode = ShopMode::Sell;
        shop.message = "Sold the Dagger for 4 gold.".into();
        let rows = vec![Row { label: "Dagger".into(), detail: "4 gold".into() }];
        let spec = shop_spec(&shop, 40, "Aldric", &rows, 0);
        assert_eq!(spec.aside, "Purse 40 gold");
        let tabs: Vec<_> = spec.tabs.iter().map(|t| (t.label.as_str(), t.active, t.keys.len())).collect();
        assert_eq!(tabs, [("Buy", false, 2), ("Sell from Aldric's pack", true, 0), ("Hire", false, 1)]);
        assert_eq!(spec.rows, rows);
        assert_eq!(spec.message, "Sold the Dagger for 4 gold.");
        assert_eq!(spec.hints[0].action, "Sell");
        assert!(shop_spec(&Shop::open(Vendor::Blacksmith), 0, "Aldric", &[], 0).empty.contains("nothing left"));
    }
}
