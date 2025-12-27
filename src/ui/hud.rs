use bevy::prelude::*;

use crate::combat::CombatStats;
use crate::components::{HungerClock, HungerState};
use crate::gamelog::GameLog;
use crate::map::Map;
use crate::player::Player;
use crate::resources::UiFont;
use crate::RunState;

use super::components::{DepthText, GameLogText, HealthBar, HealthText, HungerText};

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        // HUD updates only during actual gameplay
        let in_gameplay = not(in_state(RunState::MainMenu))
            .and(not(in_state(RunState::GameOver)))
            .and(not(in_state(RunState::MapGeneration)))
            .and(not(in_state(RunState::MapBuilderSelect)));

        app.add_systems(Startup, setup_hud).add_systems(
            Update,
            (
                update_health_bar,
                update_depth,
                update_hunger_display,
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

            // Depth display
            parent.spawn((
                Text::new("Depth: 1"),
                TextFont {
                    font: font.0.clone(),
                    font_size: 16.0,
                    ..default()
                },
                TextColor(Color::srgb(1.0, 1.0, 0.0)), // Yellow
                DepthText,
            ));

            // Hunger display
            parent.spawn((
                Text::new(""),
                TextFont {
                    font: font.0.clone(),
                    font_size: 16.0,
                    ..default()
                },
                TextColor(Color::srgb(0.0, 1.0, 0.0)), // Green (will be updated)
                HungerText,
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

fn update_depth(map: Res<Map>, mut depth_text_query: Query<&mut Text, With<DepthText>>) {
    if let Ok(mut text) = depth_text_query.get_single_mut() {
        **text = format!("Depth: {}", map.depth);
    }
}

fn update_hunger_display(
    player_query: Query<&HungerClock, With<Player>>,
    mut hunger_text_query: Query<(&mut Text, &mut TextColor), With<HungerText>>,
) {
    if let Ok(hunger) = player_query.get_single() {
        if let Ok((mut text, mut color)) = hunger_text_query.get_single_mut() {
            match hunger.state {
                HungerState::WellFed => {
                    **text = "Well Fed".to_string();
                    color.0 = Color::srgb(0.0, 1.0, 0.0); // Green
                }
                HungerState::Normal => {
                    **text = "".to_string(); // Don't display when normal
                }
                HungerState::Hungry => {
                    **text = "Hungry".to_string();
                    color.0 = Color::srgb(1.0, 0.65, 0.0); // Orange
                }
                HungerState::Starving => {
                    **text = "Starving!".to_string();
                    color.0 = Color::srgb(1.0, 0.0, 0.0); // Red
                }
            }
        }
    }
}

fn update_game_log(game_log: Res<GameLog>, mut log_text_query: Query<&mut Text, With<GameLogText>>) {
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
