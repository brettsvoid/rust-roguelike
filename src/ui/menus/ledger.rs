//! The town ledger (E08): each sector's level and progress, what the next
//! level brings, the caravan routes and the latest news. Opened with T in town.
//! The panel is `ledger_spec`, a pure function of `TownState`.

use bevy::prelude::*;

use crate::economy::town::{Sector, SectorState, TownState, STALLS};
use crate::economy::trade::{LEVEL_THRESHOLDS, NEIGHBOURS};
use crate::RunState;

use crate::ui::panel::{close_panels, escape_to_play, show, spawn_panel, BodyLine, Hint, Panel, PanelSpec, Placement};

/// Six entries would push the panel into the bottom bar at the default window size.
const NEWS_SHOWN: usize = 5;

pub struct LedgerPlugin;

impl Plugin for LedgerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(RunState::ShowLedger), |mut commands: Commands| {
            spawn_panel(&mut commands, Placement::Modal);
        })
        .add_systems(OnExit(RunState::ShowLedger), close_panels)
        .add_systems(Update, (escape_to_play, refresh_ledger).run_if(in_state(RunState::ShowLedger)));
    }
}

fn refresh_ledger(town: Res<TownState>, mut panels: Query<(&Panel, &mut PanelSpec)>) {
    show(&mut panels, ledger_spec(&town));
}

/// What reaching `level` brings a sector, from `STALLS`: the vendor who
/// stocks that tier, or opens with it.
fn unlock_text(sector: Sector, level: u32) -> String {
    let tier = level as i32;
    let unlocks: Vec<_> = STALLS
        .iter()
        .filter(|(_, s, tiers)| *s == sector && tiers.contains(&tier))
        .map(|(vendor, _, tiers)| {
            if *tiers.start() == tier {
                format!("{} opens with tier {tier}", vendor.name())
            } else {
                format!("tier {tier} at the {}", vendor.name().to_lowercase())
            }
        })
        .collect();
    if unlocks.is_empty() {
        "nothing new".to_string()
    } else {
        unlocks.join(", ")
    }
}

fn ledger_spec(town: &TownState) -> PanelSpec {
    let mut body = vec![BodyLine::Heading("Sectors".into())];
    body.extend(town.sectors.iter().map(|(&sector, state)| sector_meter(sector, state)));

    body.push(BodyLine::Heading("Trade routes".into()));
    for n in &NEIGHBOURS {
        let wants: Vec<_> = n.demand.iter().map(|&(s, q)| format!("{} {q}", s.goods())).collect();
        body.push(BodyLine::Text(format!("{} wants {}", n.name, wants.join(", "))));
    }

    body.push(BodyLine::Heading("News".into()));
    if town.news.is_empty() {
        body.push(BodyLine::Text("No news yet. Sell goods in town and caravans will take them to market.".into()));
    }
    let newest_first = town.news.iter().rev().take(NEWS_SHOWN);
    body.extend(newest_first.map(|n| BodyLine::Text(format!("Day {}: {}", n.day, n.text))));

    PanelSpec {
        title: "Town Ledger".into(),
        aside: format!("Day {}   Treasury {} gold   Town level {}", town.day, town.treasury, town.level()),
        width: 760.0,
        body,
        hints: vec![Hint::close()],
        ..default()
    }
}

/// `Smithing Lv 2`, a bar at 150/600, `150/600   60g waiting   Next: tier 3 at the blacksmith`.
fn sector_meter(sector: Sector, state: &SectorState) -> BodyLine {
    let label = format!("{:<11} Lv {}", sector.name(), state.level);
    let waiting = if state.stock > 0 { format!("   {}g waiting", state.stock) } else { String::new() };
    match LEVEL_THRESHOLDS.get(state.level as usize - 1) {
        Some(&needed) => BodyLine::Meter {
            label,
            fraction: state.progress.clamp(0, needed) as f32 / needed as f32,
            detail: format!("{}/{needed}{waiting}   Next: {}", state.progress, unlock_text(sector, state.level + 1)),
        },
        None => BodyLine::Meter { label, fraction: 1.0, detail: format!("Max level{waiting}") },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::economy::town::NewsEntry;

    #[test]
    fn a_sector_meter_shows_progress_stock_and_the_next_unlock() {
        let state = SectorState { stock: 60, level: 2, progress: 150 };
        assert_eq!(
            sector_meter(Sector::Smithing, &state),
            BodyLine::Meter {
                label: "Smithing    Lv 2".into(),
                fraction: 0.25,
                detail: "150/600   60g waiting   Next: tier 3 at the blacksmith".into(),
            }
        );
        let maxed = SectorState { stock: 0, level: 5, progress: 40 };
        assert_eq!(
            sector_meter(Sector::Faith, &maxed),
            BodyLine::Meter { label: "Faith       Lv 5".into(), fraction: 1.0, detail: "Max level".into() }
        );
    }

    #[test]
    fn unlocks_name_the_vendor_from_the_stalls() {
        assert_eq!(unlock_text(Sector::Smithing, 2), "tier 2 at the blacksmith");
        assert_eq!(unlock_text(Sector::Arcana, 2), "tier 2 at the alchemist");
        assert_eq!(unlock_text(Sector::Arcana, 3), "Enchanter opens with tier 3");
        assert_eq!(unlock_text(Sector::Arcana, 4), "tier 4 at the enchanter");
        assert_eq!(unlock_text(Sector::Faith, 6), "nothing new");
    }

    #[test]
    fn a_fresh_town_ledger() {
        let spec = ledger_spec(&TownState::fresh(1));
        assert_eq!(spec.aside, "Day 0   Treasury 0 gold   Town level 1");
        assert_eq!(spec.body[0], BodyLine::Heading("Sectors".into()));
        assert!(spec.body.contains(&BodyLine::Meter {
            label: "Arcana      Lv 1".into(),
            fraction: 0.0,
            detail: "0/250   Next: tier 2 at the alchemist".into(),
        }));
        assert!(spec.body.contains(&BodyLine::Text("Eastmarch wants arms and armour 40, food 10".into())));
        assert_eq!(
            spec.body.last(),
            Some(&BodyLine::Text("No news yet. Sell goods in town and caravans will take them to market.".into()))
        );
    }

    #[test]
    fn news_shows_the_newest_few_first() {
        let mut town = TownState::fresh(1);
        town.news = (1..=8).map(|day| NewsEntry { day, text: format!("event {day}") }).collect();
        let spec = ledger_spec(&town);
        let news = spec.body.iter().skip_while(|l| **l != BodyLine::Heading("News".into())).skip(1);
        let news: Vec<_> = news.cloned().collect();
        let want: Vec<_> = (4..=8).rev().map(|d| BodyLine::Text(format!("Day {d}: event {d}"))).collect();
        assert_eq!(news, want);
    }
}
