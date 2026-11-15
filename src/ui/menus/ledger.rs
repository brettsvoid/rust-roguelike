//! The town ledger (E08): each sector's level and progress, what the next
//! level brings, the caravan routes and the latest news. Opened with T in town.
//! The text is a pure function of `TownState`, rebuilt whenever the town changes.

use bevy::prelude::*;

use crate::economy::town::{Sector, SectorState, TownState, STALLS};
use crate::economy::trade::{LEVEL_THRESHOLDS, NEIGHBOURS};
use crate::resources::UiFont;
use crate::RunState;

use crate::ui::menu::{
    build_menu_text, despawn_modal_menu, handle_modal_menu_input, spawn_modal_menu, ModalMenu,
    ModalMenuBuilder, ModalMenuContainer, ModalMenuText,
};
use crate::ui::resources::MenuPage;

/// Characters per line; the panel is sized by its text, so this bounds its width.
const WIDTH: usize = 74;
const BAR_WIDTH: usize = 12;
/// Six wrapped entries would reach the HUD at the default window size.
const NEWS_SHOWN: usize = 5;

pub struct LedgerPlugin;

impl Plugin for LedgerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(RunState::ShowLedger), spawn_ledger)
            .add_systems(OnExit(RunState::ShowLedger), despawn_ledger)
            .add_systems(
                Update,
                (
                    handle_modal_menu_input,
                    refresh_ledger.run_if(resource_changed::<TownState>),
                )
                    .run_if(in_state(RunState::ShowLedger)),
            );
    }
}

fn ledger_menu(town: &TownState) -> ModalMenu {
    ModalMenuBuilder::new("Town Ledger")
        .empty_message(ledger_text(town))
        .footer("(Press Escape to close)")
        .on_cancel(RunState::AwaitingInput)
        .font_size(14.0)
        .build()
}

fn spawn_ledger(
    mut commands: Commands,
    font: Res<UiFont>,
    town: Res<TownState>,
    menu_page: Res<MenuPage>,
) {
    spawn_modal_menu(&mut commands, &font, &ledger_menu(&town), &menu_page);
}

fn despawn_ledger(mut commands: Commands, menu_query: Query<Entity, With<ModalMenuContainer>>) {
    despawn_modal_menu(&mut commands, &menu_query);
}

fn refresh_ledger(
    town: Res<TownState>,
    menu_page: Res<MenuPage>,
    mut text_query: Query<&mut Text, With<ModalMenuText>>,
) {
    if let Ok(mut text) = text_query.single_mut() {
        **text = build_menu_text(&ledger_menu(&town), &menu_page);
    }
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

fn ledger_text(town: &TownState) -> String {
    let mut out = format!(
        "Day {}   Treasury {} gold   Town level {}\n\nSECTORS\n",
        town.day,
        town.treasury,
        town.level()
    );
    for (&sector, state) in &town.sectors {
        out.push_str(&sector_line(sector, state));
        out.push('\n');
    }

    out.push_str("\nTRADE ROUTES\n");
    for n in &NEIGHBOURS {
        let wants: Vec<_> = n
            .demand
            .iter()
            .map(|&(s, q)| format!("{} {q}", s.goods()))
            .collect();
        out.push_str(&format!("{:<15} wants {}\n", n.name, wants.join(", ")));
    }

    out.push_str("\nNEWS\n");
    if town.news.is_empty() {
        out.push_str("No news yet. Sell goods in town and caravans will take them to market.");
    }
    let newest_first = town.news.iter().rev().take(NEWS_SHOWN);
    let lines: Vec<_> = newest_first
        .map(|n| wrap(&format!("Day {:<4} {}", n.day, n.text), WIDTH, 9))
        .collect();
    out.push_str(&lines.join("\n"));
    out
}

/// `Smithing    Lv 2 [###---------]  150/600   60g waiting   Next: tier 3 at the blacksmith`
fn sector_line(sector: Sector, state: &SectorState) -> String {
    let head = format!("{:<11} Lv {}", sector.name(), state.level);
    let waiting = if state.stock > 0 {
        format!("{}g waiting", state.stock)
    } else {
        String::new()
    };
    match LEVEL_THRESHOLDS.get(state.level as usize - 1) {
        Some(&needed) => format!(
            "{head} {} {:>4}/{needed:<4}  {waiting:<14}Next: {}",
            progress_bar(state.progress, needed),
            state.progress,
            unlock_text(sector, state.level + 1)
        ),
        None => format!("{head} [{}] max level  {waiting}", "#".repeat(BAR_WIDTH))
            .trim_end()
            .to_string(),
    }
}

fn progress_bar(progress: i32, needed: i32) -> String {
    let filled = (progress.clamp(0, needed) as usize * BAR_WIDTH) / needed as usize;
    format!("[{}{}]", "#".repeat(filled), "-".repeat(BAR_WIDTH - filled))
}

/// Word-wrap to `width` columns, indenting continuation lines by `indent`.
fn wrap(text: &str, width: usize, indent: usize) -> String {
    let mut out = String::new();
    let mut line_len = 0;
    for word in text.split(' ') {
        if line_len > 0 && line_len + 1 + word.len() > width {
            out.push('\n');
            out.push_str(&" ".repeat(indent));
            line_len = indent;
        } else if line_len > 0 {
            out.push(' ');
            line_len += 1;
        }
        out.push_str(word);
        line_len += word.len();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::economy::town::NewsEntry;

    #[test]
    fn progress_bar_fills_in_proportion_and_clamps() {
        assert_eq!(progress_bar(0, 250), "[------------]");
        assert_eq!(progress_bar(125, 250), "[######------]");
        assert_eq!(progress_bar(249, 250), "[###########-]");
        assert_eq!(progress_bar(900, 250), "[############]");
    }

    #[test]
    fn wrap_breaks_on_words_with_a_hanging_indent() {
        assert_eq!(wrap("short line", 20, 2), "short line");
        assert_eq!(
            wrap("Day 3    a caravan took food to Eastmarch", 21, 9),
            "Day 3    a caravan\n         took food to\n         Eastmarch"
        );
    }

    #[test]
    fn a_sector_line_shows_progress_stock_and_the_next_unlock() {
        let state = SectorState {
            stock: 60,
            level: 2,
            progress: 150,
        };
        assert_eq!(
            sector_line(Sector::Smithing, &state),
            "Smithing    Lv 2 [###---------]  150/600   60g waiting   Next: tier 3 at the blacksmith"
        );
        let maxed = SectorState {
            stock: 0,
            level: 5,
            progress: 40,
        };
        assert_eq!(
            sector_line(Sector::Faith, &maxed),
            "Faith       Lv 5 [############] max level"
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
        let text = ledger_text(&TownState::fresh(1));
        assert!(text.starts_with("Day 0   Treasury 0 gold   Town level 1\n\nSECTORS\n"));
        assert!(text.contains("Arcana      Lv 1 [------------]    0/250                 Next: tier 2 at the alchemist"));
        assert!(text.contains("Eastmarch       wants arms and armour 40, food 10\n"));
        assert!(text.ends_with("NEWS\nNo news yet. Sell goods in town and caravans will take them to market."));
    }

    #[test]
    fn news_shows_the_newest_few_first() {
        let mut town = TownState::fresh(1);
        town.news = (1..=8)
            .map(|day| NewsEntry {
                day,
                text: format!("event {day}"),
            })
            .collect();
        let text = ledger_text(&town);
        let news = text.split("NEWS\n").nth(1).unwrap();
        assert_eq!(
            news,
            "Day 8    event 8\nDay 7    event 7\nDay 6    event 6\n\
             Day 5    event 5\nDay 4    event 4"
        );
    }
}
