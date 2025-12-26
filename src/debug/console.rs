use bevy::input::keyboard::KeyboardInput;
use bevy::input::ButtonState;
use bevy::prelude::*;

use crate::combat::CombatStats;
use crate::map::Position;
use crate::monsters::Monster;
use crate::player::Player;
use crate::resources::UiFont;

use super::commands::execute_command;
use super::resources::{DebugMode, DebugState, GodMode};

// ============================================================================
// Components
// ============================================================================

#[derive(Component)]
pub struct DebugConsole;

#[derive(Component)]
struct ConsoleInput;

#[derive(Component)]
struct ConsoleOutput;

// ============================================================================
// Systems
// ============================================================================

pub fn update_console(
    mut commands: Commands,
    debug: Res<DebugMode>,
    debug_state: Res<DebugState>,
    font: Res<UiFont>,
    console_query: Query<Entity, With<DebugConsole>>,
) {
    // Despawn existing console
    for entity in &console_query {
        commands.entity(entity).despawn_recursive();
    }

    if !debug.show_console {
        return;
    }

    let text_font = TextFont {
        font: font.0.clone(),
        font_size: 14.0,
        ..default()
    };

    commands
        .spawn((
            DebugConsole,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(25.0),
                top: Val::Percent(20.0),
                width: Val::Percent(50.0),
                height: Val::Px(300.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.05, 0.1, 0.95)),
        ))
        .with_children(|parent| {
            // Title
            parent.spawn((
                Text::new("DEBUG CONSOLE (Esc to close)"),
                text_font.clone(),
                TextColor(Color::srgb(1.0, 1.0, 0.0)),
            ));

            // Output area
            parent
                .spawn((
                    ConsoleOutput,
                    Node {
                        flex_direction: FlexDirection::Column,
                        flex_grow: 1.0,
                        overflow: Overflow::clip(),
                        ..default()
                    },
                ))
                .with_children(|output| {
                    // Show last N output lines
                    let start = debug_state.console_output.len().saturating_sub(10);
                    for line in debug_state.console_output.iter().skip(start) {
                        output.spawn((
                            Text::new(line.clone()),
                            TextFont {
                                font: font.0.clone(),
                                font_size: 12.0,
                                ..default()
                            },
                            TextColor(Color::srgb(0.8, 0.8, 0.8)),
                        ));
                    }
                });

            // Input line
            parent.spawn((
                ConsoleInput,
                Text::new(format!("> {}_", debug_state.console_input)),
                text_font.clone(),
                TextColor(Color::srgb(0.0, 1.0, 0.0)),
            ));

            // Help hint
            parent.spawn((
                Text::new("Commands: spawn, teleport, godmode, reveal, nofog, heal, kill_all, help"),
                TextFont {
                    font: font.0.clone(),
                    font_size: 11.0,
                    ..default()
                },
                TextColor(Color::srgb(0.4, 0.4, 0.4)),
            ));
        });
}

pub fn handle_console_input(
    mut commands: Commands,
    mut debug: ResMut<DebugMode>,
    mut debug_state: ResMut<DebugState>,
    mut god_mode: ResMut<GodMode>,
    mut evr_kbd: EventReader<KeyboardInput>,
    keyboard: Res<ButtonInput<KeyCode>>,
    font: Res<UiFont>,
    mut player_query: Query<(Entity, &mut Position, &mut CombatStats), With<Player>>,
    monster_query: Query<Entity, With<Monster>>,
) {
    if !debug.show_console {
        return;
    }

    let shift = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);

    for ev in evr_kbd.read() {
        // Only handle key presses (not releases)
        if ev.state == ButtonState::Released {
            continue;
        }

        match ev.key_code {
            // Close on escape
            KeyCode::Escape => {
                debug.show_console = false;
                return;
            }

            // Handle backspace
            KeyCode::Backspace => {
                debug_state.console_input.pop();
            }

            // Handle enter - execute command
            KeyCode::Enter => {
                if !debug_state.console_input.is_empty() {
                    let input = debug_state.console_input.clone();
                    debug_state.console_output.push(format!("> {}", input));
                    debug_state.command_history.push(input.clone());

                    // Parse and execute command
                    let output = execute_command(
                        &input,
                        &mut commands,
                        &mut debug_state,
                        &mut god_mode,
                        &font,
                        &mut player_query,
                        &monster_query,
                    );
                    debug_state.console_output.push(output);

                    debug_state.console_input.clear();
                }
            }

            // Handle space
            KeyCode::Space => {
                debug_state.console_input.push(' ');
            }

            // Letters
            KeyCode::KeyA => debug_state.console_input.push(if shift { 'A' } else { 'a' }),
            KeyCode::KeyB => debug_state.console_input.push(if shift { 'B' } else { 'b' }),
            KeyCode::KeyC => debug_state.console_input.push(if shift { 'C' } else { 'c' }),
            KeyCode::KeyD => debug_state.console_input.push(if shift { 'D' } else { 'd' }),
            KeyCode::KeyE => debug_state.console_input.push(if shift { 'E' } else { 'e' }),
            KeyCode::KeyF => debug_state.console_input.push(if shift { 'F' } else { 'f' }),
            KeyCode::KeyG => debug_state.console_input.push(if shift { 'G' } else { 'g' }),
            KeyCode::KeyH => debug_state.console_input.push(if shift { 'H' } else { 'h' }),
            KeyCode::KeyI => debug_state.console_input.push(if shift { 'I' } else { 'i' }),
            KeyCode::KeyJ => debug_state.console_input.push(if shift { 'J' } else { 'j' }),
            KeyCode::KeyK => debug_state.console_input.push(if shift { 'K' } else { 'k' }),
            KeyCode::KeyL => debug_state.console_input.push(if shift { 'L' } else { 'l' }),
            KeyCode::KeyM => debug_state.console_input.push(if shift { 'M' } else { 'm' }),
            KeyCode::KeyN => debug_state.console_input.push(if shift { 'N' } else { 'n' }),
            KeyCode::KeyO => debug_state.console_input.push(if shift { 'O' } else { 'o' }),
            KeyCode::KeyP => debug_state.console_input.push(if shift { 'P' } else { 'p' }),
            KeyCode::KeyQ => debug_state.console_input.push(if shift { 'Q' } else { 'q' }),
            KeyCode::KeyR => debug_state.console_input.push(if shift { 'R' } else { 'r' }),
            KeyCode::KeyS => debug_state.console_input.push(if shift { 'S' } else { 's' }),
            KeyCode::KeyT => debug_state.console_input.push(if shift { 'T' } else { 't' }),
            KeyCode::KeyU => debug_state.console_input.push(if shift { 'U' } else { 'u' }),
            KeyCode::KeyV => debug_state.console_input.push(if shift { 'V' } else { 'v' }),
            KeyCode::KeyW => debug_state.console_input.push(if shift { 'W' } else { 'w' }),
            KeyCode::KeyX => debug_state.console_input.push(if shift { 'X' } else { 'x' }),
            KeyCode::KeyY => debug_state.console_input.push(if shift { 'Y' } else { 'y' }),
            KeyCode::KeyZ => debug_state.console_input.push(if shift { 'Z' } else { 'z' }),

            // Numbers
            KeyCode::Digit0 => debug_state.console_input.push('0'),
            KeyCode::Digit1 => debug_state.console_input.push('1'),
            KeyCode::Digit2 => debug_state.console_input.push('2'),
            KeyCode::Digit3 => debug_state.console_input.push('3'),
            KeyCode::Digit4 => debug_state.console_input.push('4'),
            KeyCode::Digit5 => debug_state.console_input.push('5'),
            KeyCode::Digit6 => debug_state.console_input.push('6'),
            KeyCode::Digit7 => debug_state.console_input.push('7'),
            KeyCode::Digit8 => debug_state.console_input.push('8'),
            KeyCode::Digit9 => debug_state.console_input.push('9'),

            // Underscore (shift + minus) or minus
            KeyCode::Minus => {
                debug_state.console_input.push(if shift { '_' } else { '-' });
            }

            _ => {}
        }
    }
}
