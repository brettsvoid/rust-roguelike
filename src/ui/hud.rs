use bevy::prelude::*;

use crate::combat::CombatStats;
use crate::components::{HungerClock, HungerState, Name};
use crate::economy::Purse;
use crate::entity_render::Renderable;
use crate::gamelog::GameLog;
use crate::map::Map;
use crate::party::{ActiveUnit, PartyMember, PARTY_SIZE};
use crate::picking::BlocksPicking;
use crate::resources::UiFont;
use crate::RunState;

use super::components::{DepthText, GameLogText, GoldText, PartyRow};

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        // HUD updates only during actual gameplay
        let in_gameplay = not(in_state(RunState::MainMenu))
            .and_then(not(in_state(RunState::GameOver)))
            .and_then(not(in_state(RunState::MapGeneration)))
            .and_then(not(in_state(RunState::MapBuilderSelect)));

        app.add_systems(Startup, setup_hud).add_systems(
            Update,
            (
                update_party_rows,
                update_depth,
                update_gold,
                update_game_log,
            )
                .run_if(in_gameplay),
        );
    }
}

fn setup_hud(mut commands: Commands, font: Res<UiFont>) {
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
            BlocksPicking,
        ))
        .with_children(|parent| {
            // One row per hero, in slot order
            parent
                .spawn(Node {
                    flex_direction: FlexDirection::Column,
                    // The game log grows into the rest; the rows keep their width.
                    flex_shrink: 0.0,
                    ..default()
                })
                .with_children(|rows| {
                    for slot in 0..PARTY_SIZE {
                        rows.spawn((
                            Text::new(""),
                            TextFont {
                                font: font.0.clone().into(),
                                font_size: FontSize::Px(16.0),
                                ..default()
                            },
                            PartyRow(slot),
                        ));
                    }
                });

            // Depth display
            parent.spawn((
                Text::new("Depth: 1"),
                TextFont {
                    font: font.0.clone().into(),
                    font_size: FontSize::Px(16.0),
                    ..default()
                },
                TextColor(Color::srgb(1.0, 1.0, 0.0)), // Yellow
                DepthText,
                Node { flex_shrink: 0.0, ..default() },
            ));

            parent.spawn((
                Text::new("Gold: 0"),
                TextFont {
                    font: font.0.clone().into(),
                    font_size: FontSize::Px(16.0),
                    ..default()
                },
                TextColor(Color::srgb(1.0, 0.84, 0.0)), // Gold
                GoldText,
                Node { flex_shrink: 0.0, ..default() },
            ));

            // Game log container
            parent
                .spawn(Node {
                    flex_direction: FlexDirection::Column,
                    height: Val::Percent(100.0),
                    flex_grow: 1.0,
                    overflow: Overflow::clip(),
                    ..default()
                })
                .with_children(|log_parent| {
                    // Game log text (shows last 5 messages)
                    log_parent.spawn((
                        Text::new(""),
                        TextFont {
                            font: font.0.clone().into(),
                            font_size: FontSize::Px(14.0),
                            ..default()
                        },
                        TextColor(Color::WHITE),
                        GameLogText,
                    ));
                });
        });
}

/// Each row shows its hero's hit points and hunger in the hero's colour; a
/// `>` marks the active unit. A fallen hero's row is blank.
fn update_party_rows(
    active: Res<ActiveUnit>,
    party: Query<(Entity, &PartyMember, &Name, &CombatStats, &HungerClock, &Renderable)>,
    mut rows: Query<(&PartyRow, &mut Text, &mut TextColor)>,
) {
    for (PartyRow(slot), mut text, mut colour) in &mut rows {
        let Some((entity, _, name, stats, hunger, renderable)) = party.iter().find(|m| m.1.slot == *slot) else {
            text.clear();
            continue;
        };
        let marker = if active.0 == Some(entity) { '>' } else { ' ' };
        **text = format!(
            "{marker} {:<7} HP {:>2}/{:<2} {}",
            name.name,
            stats.hp,
            stats.max_hp,
            hunger_label(hunger.state)
        );
        colour.0 = renderable.fg;
    }
}

fn hunger_label(state: HungerState) -> &'static str {
    match state {
        HungerState::WellFed => "Well Fed",
        HungerState::Normal => "",
        HungerState::Hungry => "Hungry",
        HungerState::Starving => "Starving!",
    }
}

fn update_depth(map: Res<Map>, mut depth_text_query: Query<&mut Text, With<DepthText>>) {
    if let Ok(mut text) = depth_text_query.single_mut() {
        **text = format!("Depth: {}", map.depth);
    }
}

fn update_gold(purse: Res<Purse>, mut gold_text_query: Query<&mut Text, With<GoldText>>) {
    if let Ok(mut text) = gold_text_query.single_mut() {
        **text = format!("Gold: {}", purse.gold);
    }
}

fn update_game_log(game_log: Res<GameLog>, mut log_text_query: Query<&mut Text, With<GameLogText>>) {
    if let Ok(mut text) = log_text_query.single_mut() {
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
