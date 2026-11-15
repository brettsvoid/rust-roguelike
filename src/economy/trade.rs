//! The trade simulation: pure functions over `TownState`, no Bevy types.
//!
//! Each simulated day one caravan leaves town. It goes to whichever
//! neighbouring town will take the most goods from the current stock, carries
//! up to that town's demand of each sector, and comes back with gold for the
//! treasury. Every gold of goods traded is a point of progress for its sector,
//! and a sector levels up when its progress reaches the next threshold. There
//! is no randomness, so a town replays identically.

use super::town::{NewsEntry, Sector, TownState};

// ============================================================================
// Tuning (P04 retunes these numbers without reading the logic below)
// ============================================================================
//
// Tuned against this assumed run: 5 days in the dungeon, selling 250 gold of
// base value on return (a longsword and a battleaxe, two health potions, a
// magic missile scroll and two rations). Two such runs of loot traded away
// level smithing up during the third delve.

/// What the town pays the party, as a percentage of base value at no stock.
pub const SELL_PCT: i32 = 40;
/// What the town charges the party, as a percentage of base value at no stock.
pub const BUY_PCT: i32 = 100;
/// Prices fall one percentage point per this much gold of a sector's stock...
pub const STOCK_PER_PRICE_PCT: i32 = 10;
/// ...down to this percentage of the full price. Must stay above `SELL_PCT`
/// (as a share of `BUY_PCT`) so selling is always cheaper than buying.
pub const PRICE_FLOOR_PCT: i32 = 50;
/// Neighbours pay this percentage of base value for goods a caravan carries.
pub const TRADE_RETURN_PCT: i32 = 60;
/// Progress needed to leave level 1, 2, 3 and so on. A sector past the end
/// of the table stops levelling.
pub const LEVEL_THRESHOLDS: [i32; 4] = [250, 600, 1200, 2000];
/// The news log keeps only this many of its newest entries.
pub const NEWS_KEEP: usize = 200;

pub struct Neighbour {
    pub name: &'static str,
    /// Gold of goods per sector one caravan can sell there.
    pub demand: &'static [(Sector, i32)],
}

pub const NEIGHBOURS: [Neighbour; 4] = [
    Neighbour {
        name: "Eastmarch",
        demand: &[(Sector::Smithing, 40), (Sector::Provisions, 10)],
    },
    Neighbour {
        name: "Saltmere",
        demand: &[
            (Sector::Alchemy, 25),
            (Sector::Textiles, 20),
            (Sector::Provisions, 20),
        ],
    },
    Neighbour {
        name: "Greyvale Abbey",
        demand: &[
            (Sector::Faith, 30),
            (Sector::Alchemy, 10),
            (Sector::Arcana, 10),
        ],
    },
    Neighbour {
        name: "Thornwick",
        demand: &[
            (Sector::Arcana, 30),
            (Sector::Smithing, 15),
            (Sector::Textiles, 10),
        ],
    },
];

impl Sector {
    pub fn name(self) -> &'static str {
        match self {
            Sector::Smithing => "Smithing",
            Sector::Alchemy => "Alchemy",
            Sector::Textiles => "Textiles",
            Sector::Provisions => "Provisions",
            Sector::Faith => "Faith",
            Sector::Arcana => "Arcana",
        }
    }

    /// What a caravan carries for this sector, as the news calls it.
    pub fn goods(self) -> &'static str {
        match self {
            Sector::Smithing => "arms and armour",
            Sector::Alchemy => "potions",
            Sector::Textiles => "cloth",
            Sector::Provisions => "food",
            Sector::Faith => "relics",
            Sector::Arcana => "scrolls and charms",
        }
    }
}

// ============================================================================
// Selling and prices (for vendors, E02)
// ============================================================================

/// The party sold `quantity` items of `base_value` each into `sector`.
pub fn record_sale(town: &mut TownState, sector: Sector, base_value: i32, quantity: u32) {
    town.sectors.get_mut(&sector).unwrap().stock += base_value * quantity as i32;
}

/// Percentage of full price left once `sector`'s stock is counted.
fn glut_pct(town: &TownState, sector: Sector) -> i32 {
    let stock = town.sectors[&sector].stock.max(0);
    (100 - stock / STOCK_PER_PRICE_PCT).max(PRICE_FLOOR_PCT)
}

/// Gold the town pays the party for one item. Rounds down.
#[allow(dead_code)] // first called by vendors (E02)
pub fn sell_price(town: &TownState, sector: Sector, base_value: i32) -> i32 {
    base_value * SELL_PCT * glut_pct(town, sector) / 10_000
}

/// Gold the town charges the party for one item. Rounds up, so it is never
/// below `sell_price`'s floor-rounded figure.
#[allow(dead_code)] // first called by vendors (E02)
pub fn buy_price(town: &TownState, sector: Sector, base_value: i32) -> i32 {
    (base_value * BUY_PCT * glut_pct(town, sector) + 9_999) / 10_000
}

// ============================================================================
// The simulation (run on return to town, E07)
// ============================================================================

/// Advance the town by `days`. Returns the news written on the way, which is
/// also appended to `town.news`.
pub fn simulate(town: &mut TownState, days: u32) -> Vec<NewsEntry> {
    let mut news = Vec::new();
    for _ in 0..days {
        town.day += 1;
        send_caravan(town, &mut news);
    }
    town.news.extend(news.iter().cloned());
    let excess = town.news.len().saturating_sub(NEWS_KEEP);
    town.news.drain(..excess);
    news
}

fn send_caravan(town: &mut TownState, news: &mut Vec<NewsEntry>) {
    let cargo_for = |n: &Neighbour| -> Vec<(Sector, i32)> {
        n.demand
            .iter()
            .map(|&(s, want)| (s, want.min(town.sectors[&s].stock)))
            .filter(|&(_, q)| q > 0)
            .collect()
    };
    let total = |cargo: &[(Sector, i32)]| cargo.iter().map(|(_, q)| q).sum::<i32>();
    // min_by_key keeps the first of equals, so ties go to the earlier town.
    let Some((neighbour, cargo)) = NEIGHBOURS
        .iter()
        .map(|n| (n, cargo_for(n)))
        .min_by_key(|(_, cargo)| -total(cargo))
        .filter(|(_, cargo)| !cargo.is_empty())
    else {
        return;
    };

    let earned = total(&cargo) * TRADE_RETURN_PCT / 100;
    town.treasury += earned;
    let carried: Vec<_> = cargo
        .iter()
        .map(|&(s, q)| format!("{q} gold of {}", s.goods()))
        .collect();
    let day = town.day;
    news.push(NewsEntry {
        day,
        text: format!(
            "A caravan took {} to {} and came back with {earned} gold.",
            join_and(&carried),
            neighbour.name
        ),
    });

    for (sector, q) in cargo {
        let state = town.sectors.get_mut(&sector).unwrap();
        state.stock -= q;
        state.progress += q;
        while let Some(&needed) = LEVEL_THRESHOLDS.get(state.level as usize - 1) {
            if state.progress < needed {
                break;
            }
            state.progress -= needed;
            state.level += 1;
            news.push(NewsEntry {
                day,
                text: format!(
                    "{} grew to level {} thanks to the trade with {}.",
                    sector.name(),
                    state.level,
                    neighbour.name
                ),
            });
        }
    }
}

/// "a", "a and b", "a, b and c".
fn join_and(parts: &[String]) -> String {
    match parts {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Sector::*;

    fn town_with(stock: &[(Sector, i32)]) -> TownState {
        let mut town = TownState::fresh(7);
        for &(s, q) in stock {
            town.sectors.get_mut(&s).unwrap().stock = q;
        }
        town
    }

    fn texts(news: &[NewsEntry]) -> Vec<(u32, &str)> {
        news.iter().map(|n| (n.day, n.text.as_str())).collect()
    }

    #[test]
    fn nothing_sold_changes_nothing_but_the_day() {
        for days in [0, 1, 30] {
            let mut town = TownState::fresh(7);
            town.treasury = 90;
            let news = simulate(&mut town, days);
            let mut expected = TownState::fresh(7);
            expected.treasury = 90;
            expected.day = days;
            assert_eq!((town, news), (expected, vec![]), "{days} days");
        }
    }

    #[test]
    fn a_sale_raises_its_sectors_stock() {
        let cases = [
            (Smithing, 60, 1, 60),
            (Alchemy, 25, 4, 100),
            (Arcana, 40, 0, 0),
        ];
        for (sector, value, qty, stock) in cases {
            let mut town = TownState::fresh(7);
            record_sale(&mut town, sector, value, qty);
            assert_eq!(town.sectors[&sector].stock, stock, "{sector:?}");
            let others = Sector::ALL.iter().filter(|&&s| s != sector);
            assert!(others.into_iter().all(|s| town.sectors[s].stock == 0));
        }
    }

    #[test]
    fn days_of_trade_move_stock_into_treasury_and_progress() {
        // (stock, days, stock after, progress after, treasury after)
        let cases = [
            (100, 1, 60, 40, 24),
            (100, 2, 20, 80, 48),
            (100, 3, 0, 100, 60),
            (100, 9, 0, 100, 60),
        ];
        for (stock, days, left, progress, treasury) in cases {
            let mut town = town_with(&[(Smithing, stock)]);
            simulate(&mut town, days);
            let s = &town.sectors[&Smithing];
            assert_eq!(
                (s.stock, s.progress, s.level, town.treasury, town.day),
                (left, progress, 1, treasury, days),
                "{days} days"
            );
        }
    }

    #[test]
    fn the_caravan_goes_where_demand_is_and_the_news_says_so() {
        let mut town = town_with(&[(Smithing, 100)]);
        let news = simulate(&mut town, 3);
        assert_eq!(
            texts(&news),
            [
                (
                    1,
                    "A caravan took 40 gold of arms and armour to Eastmarch and came back with 24 gold."
                ),
                (
                    2,
                    "A caravan took 40 gold of arms and armour to Eastmarch and came back with 24 gold."
                ),
                (
                    3,
                    "A caravan took 20 gold of arms and armour to Eastmarch and came back with 12 gold."
                ),
            ]
        );
        assert_eq!(town.news, news);

        let mut town = town_with(&[(Smithing, 20), (Arcana, 50), (Provisions, 5), (Faith, 5)]);
        assert_eq!(
            texts(&simulate(&mut town, 1)),
            [(
                1,
                "A caravan took 30 gold of scrolls and charms and 15 gold of arms and armour to Thornwick and came back with 27 gold."
            )]
        );
        assert_eq!(
            texts(&simulate(&mut town, 1)),
            [(
                2,
                "A caravan took 20 gold of scrolls and charms and 5 gold of arms and armour to Thornwick and came back with 15 gold."
            )]
        );
    }

    #[test]
    fn a_sector_levels_up_at_its_threshold() {
        // (progress before, stock, level after, progress after)
        let cases = [(209, 40, 1, 249), (210, 40, 2, 0), (240, 40, 2, 30)];
        for (before, stock, level, after) in cases {
            let mut town = town_with(&[(Smithing, stock)]);
            town.sectors.get_mut(&Smithing).unwrap().progress = before;
            let news = simulate(&mut town, 1);
            let s = &town.sectors[&Smithing];
            assert_eq!((s.level, s.progress), (level, after), "from {before}");
            let levelled = news
                .iter()
                .any(|n| n.text == "Smithing grew to level 2 thanks to the trade with Eastmarch.");
            assert_eq!(levelled, level == 2, "from {before}");
        }
    }

    #[test]
    fn a_sector_stops_levelling_past_the_last_threshold() {
        let mut town = town_with(&[(Smithing, 40)]);
        let s = town.sectors.get_mut(&Smithing).unwrap();
        s.level = LEVEL_THRESHOLDS.len() as u32 + 1;
        s.progress = 5000;
        simulate(&mut town, 1);
        assert_eq!(town.sectors[&Smithing].level, 5);
    }

    #[test]
    fn flooding_a_good_lowers_its_price() {
        // Longsword, base value 60. (stock, sell, buy)
        let cases = [
            (0, 24, 60),
            (100, 21, 54),
            (300, 16, 42),
            (500, 12, 30),
            (5000, 12, 30),
        ];
        for (stock, sell, buy) in cases {
            let town = town_with(&[(Smithing, stock)]);
            let prices = (
                sell_price(&town, Smithing, 60),
                buy_price(&town, Smithing, 60),
            );
            assert_eq!(prices, (sell, buy), "stock {stock}");
        }
        let other = town_with(&[(Smithing, 5000)]);
        assert_eq!(sell_price(&other, Alchemy, 25), 10, "other sectors unmoved");
    }

    #[test]
    fn selling_never_pays_more_than_buying_costs() {
        for base in [1, 2, 5, 15, 25, 40, 60, 90, 1000] {
            for stock in (0..=6000).step_by(7) {
                let before = town_with(&[(Arcana, stock)]);
                let sell = sell_price(&before, Arcana, base);
                let mut after = before.clone();
                record_sale(&mut after, Arcana, base, 1);
                let buy_now = buy_price(&before, Arcana, base);
                let buy_back = buy_price(&after, Arcana, base);
                assert!(
                    sell < buy_now && sell < buy_back,
                    "base {base}, stock {stock}: sell {sell}, buy {buy_now}/{buy_back}"
                );
            }
        }
    }

    #[test]
    fn simulating_in_two_steps_equals_one() {
        let start = {
            let mut t = town_with(&[(Smithing, 230), (Alchemy, 75), (Arcana, 120), (Faith, 40)]);
            t.sectors.get_mut(&Smithing).unwrap().progress = 200;
            t
        };
        for (a, b) in [(0, 0), (0, 5), (1, 1), (2, 7), (5, 3), (10, 10)] {
            let mut whole = start.clone();
            simulate(&mut whole, a + b);
            let mut split = start.clone();
            simulate(&mut split, a);
            simulate(&mut split, b);
            assert_eq!(whole, split, "{a} + {b}");
        }
    }

    #[test]
    fn the_news_log_keeps_only_the_newest_entries() {
        let mut town = town_with(&[(Smithing, 40 * (NEWS_KEEP as i32 + 5))]);
        // Past the last level, so each day writes exactly one line.
        town.sectors.get_mut(&Smithing).unwrap().level = 5;
        simulate(&mut town, NEWS_KEEP as u32 + 5);
        assert_eq!(town.news.len(), NEWS_KEEP);
        assert_eq!(town.news[0].day, 6);
    }

    /// The loot the tuning assumes one run sells: 250 gold of base value.
    fn sell_one_runs_loot(town: &mut TownState) {
        record_sale(town, Smithing, 60, 1);
        record_sale(town, Smithing, 90, 1);
        record_sale(town, Alchemy, 25, 2);
        record_sale(town, Arcana, 40, 1);
        record_sale(town, Provisions, 5, 2);
    }

    #[test]
    fn three_runs_of_loot_level_smithing_on_the_third_delve() {
        let mut town = TownState::fresh(7);
        let mut log = Vec::new();
        for run in 1..=3 {
            log.extend(simulate(&mut town, 5));
            assert_eq!(town.sectors[&Smithing].level, if run < 3 { 1 } else { 2 });
            sell_one_runs_loot(&mut town);
        }
        assert_eq!(
            texts(&log),
            [
                (
                    6,
                    "A caravan took 40 gold of arms and armour and 10 gold of food to Eastmarch and came back with 30 gold."
                ),
                (
                    7,
                    "A caravan took 30 gold of scrolls and charms and 15 gold of arms and armour to Thornwick and came back with 27 gold."
                ),
                (
                    8,
                    "A caravan took 40 gold of arms and armour to Eastmarch and came back with 24 gold."
                ),
                (
                    9,
                    "A caravan took 40 gold of arms and armour to Eastmarch and came back with 24 gold."
                ),
                (
                    10,
                    "A caravan took 25 gold of potions to Saltmere and came back with 15 gold."
                ),
                (
                    11,
                    "A caravan took 40 gold of arms and armour and 10 gold of food to Eastmarch and came back with 30 gold."
                ),
                (
                    12,
                    "A caravan took 30 gold of scrolls and charms and 15 gold of arms and armour to Thornwick and came back with 27 gold."
                ),
                (
                    13,
                    "A caravan took 40 gold of arms and armour to Eastmarch and came back with 24 gold."
                ),
                (
                    14,
                    "A caravan took 40 gold of arms and armour to Eastmarch and came back with 24 gold."
                ),
                (
                    14,
                    "Smithing grew to level 2 thanks to the trade with Eastmarch."
                ),
                (
                    15,
                    "A caravan took 20 gold of scrolls and charms and 15 gold of arms and armour to Thornwick and came back with 21 gold."
                ),
            ]
        );
    }

    /// Runs the real town plugin: a town that traded is saved to `town.json`
    /// and loads back identical.
    #[test]
    fn a_simulated_town_round_trips_through_the_file() {
        use super::super::town::{load_town, TownFile, TownPlugin};
        use bevy::prelude::*;

        let dir = std::env::temp_dir().join(format!("roguelike-trade-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("town.json");

        let mut app = App::new();
        app.add_plugins(TownPlugin);
        app.insert_resource(TownFile {
            path: path.clone(),
            writable: true,
        });
        app.update();

        {
            let mut town = app.world_mut().resource_mut::<TownState>();
            sell_one_runs_loot(&mut town);
            simulate(&mut town, 12);
        }
        app.update();

        let live = app.world().resource::<TownState>().clone();
        assert!(
            live.treasury > 0 && !live.news.is_empty(),
            "the town traded"
        );
        assert_eq!(load_town(&path, 0).0, live);
    }
}
