use bevy::prelude::*;

use crate::combat::CombatStats;
use crate::map::{Position, MAP_HEIGHT, MAP_WIDTH};
use crate::monsters::Monster;
use crate::player::Player;
use crate::resources::UiFont;

use super::resources::{DebugState, GodMode};

/// Execute a debug command and return the result message
pub fn execute_command(
    input: &str,
    commands: &mut Commands,
    debug_state: &mut DebugState,
    god_mode: &mut GodMode,
    font: &UiFont,
    player_query: &mut Query<(Entity, &mut Position, &mut CombatStats), With<Player>>,
    monster_query: &Query<Entity, With<Monster>>,
) -> String {
    let parts: Vec<&str> = input.trim().split_whitespace().collect();
    let cmd = parts.first().map(|s| s.to_lowercase());

    match cmd.as_deref() {
        Some("help") => cmd_help(),
        Some("godmode") => cmd_godmode(god_mode),
        Some("reveal") => cmd_reveal(debug_state),
        Some("nofog") => cmd_nofog(debug_state),
        Some("heal") => cmd_heal(player_query),
        Some("kill_all") => cmd_kill_all(commands, monster_query),
        Some("teleport") => cmd_teleport(&parts, player_query),
        Some("spawn") => cmd_spawn(&parts, commands, font, player_query),
        Some(cmd) => format!("Unknown command: {}", cmd),
        None => String::new(),
    }
}

fn cmd_help() -> String {
    "Commands: spawn <item>, teleport <x> <y>, godmode, reveal, nofog, heal, kill_all".to_string()
}

fn cmd_godmode(god_mode: &mut GodMode) -> String {
    god_mode.0 = !god_mode.0;
    format!("God mode: {}", if god_mode.0 { "ON" } else { "OFF" })
}

fn cmd_reveal(debug_state: &mut DebugState) -> String {
    debug_state.reveal_map = true;
    "Map revealed".to_string()
}

fn cmd_nofog(debug_state: &mut DebugState) -> String {
    debug_state.no_fog = !debug_state.no_fog;
    if debug_state.no_fog {
        debug_state.reveal_map = true; // Also reveal the map
    }
    format!("No fog: {}", if debug_state.no_fog { "ON" } else { "OFF" })
}

fn cmd_heal(
    player_query: &mut Query<(Entity, &mut Position, &mut CombatStats), With<Player>>,
) -> String {
    if let Ok((_, _, mut stats)) = player_query.get_single_mut() {
        stats.hp = stats.max_hp;
        format!("Healed to {}/{}", stats.hp, stats.max_hp)
    } else {
        "No player found".to_string()
    }
}

fn cmd_kill_all(commands: &mut Commands, monster_query: &Query<Entity, With<Monster>>) -> String {
    let count = monster_query.iter().count();
    for entity in monster_query.iter() {
        commands.entity(entity).despawn();
    }
    format!("Killed {} monsters", count)
}

fn cmd_teleport(
    parts: &[&str],
    player_query: &mut Query<(Entity, &mut Position, &mut CombatStats), With<Player>>,
) -> String {
    if parts.len() < 3 {
        return "Usage: teleport <x> <y>".to_string();
    }
    let x = parts[1].parse::<i32>();
    let y = parts[2].parse::<i32>();
    match (x, y) {
        (Ok(x), Ok(y)) => {
            if x >= 0 && x < MAP_WIDTH as i32 && y >= 0 && y < MAP_HEIGHT as i32 {
                if let Ok((_, mut pos, _)) = player_query.get_single_mut() {
                    pos.x = x;
                    pos.y = y;
                    format!("Teleported to ({}, {})", x, y)
                } else {
                    "No player found".to_string()
                }
            } else {
                "Position out of bounds".to_string()
            }
        }
        _ => "Invalid coordinates".to_string(),
    }
}

fn cmd_spawn(
    parts: &[&str],
    commands: &mut Commands,
    font: &UiFont,
    player_query: &mut Query<(Entity, &mut Position, &mut CombatStats), With<Player>>,
) -> String {
    if parts.len() < 2 {
        return "Usage: spawn <item_name>".to_string();
    }
    let item_name = parts[1].to_lowercase();

    let Ok((_, pos, _)) = player_query.get_single_mut() else {
        return "No player found".to_string();
    };
    let (x, y) = (pos.x, pos.y);

    let text_font = TextFont {
        font: font.0.clone(),
        font_size: 16.0,
        ..default()
    };

    match item_name.as_str() {
        "health_potion" | "potion" => {
            crate::spawner::spawn_health_potion(commands, &text_font, x, y);
            format!("Spawned Health Potion at ({}, {})", x, y)
        }
        "magic_missile" | "missile" => {
            crate::spawner::spawn_magic_missile_scroll(commands, &text_font, x, y);
            format!("Spawned Magic Missile Scroll at ({}, {})", x, y)
        }
        "fireball" => {
            crate::spawner::spawn_fireball_scroll(commands, &text_font, x, y);
            format!("Spawned Fireball Scroll at ({}, {})", x, y)
        }
        "confusion" => {
            crate::spawner::spawn_confusion_scroll(commands, &text_font, x, y);
            format!("Spawned Confusion Scroll at ({}, {})", x, y)
        }
        "dagger" => {
            crate::spawner::spawn_dagger(commands, &text_font, x, y);
            format!("Spawned Dagger at ({}, {})", x, y)
        }
        "shield" => {
            crate::spawner::spawn_shield(commands, &text_font, x, y);
            format!("Spawned Shield at ({}, {})", x, y)
        }
        "longsword" | "sword" => {
            crate::spawner::spawn_longsword(commands, &text_font, x, y);
            format!("Spawned Longsword at ({}, {})", x, y)
        }
        "tower_shield" => {
            crate::spawner::spawn_tower_shield(commands, &text_font, x, y);
            format!("Spawned Tower Shield at ({}, {})", x, y)
        }
        "rations" | "food" => {
            crate::spawner::spawn_rations(commands, &text_font, x, y);
            format!("Spawned Rations at ({}, {})", x, y)
        }
        "magic_map" | "map" => {
            crate::spawner::spawn_magic_mapping_scroll(commands, &text_font, x, y);
            format!("Spawned Magic Mapping Scroll at ({}, {})", x, y)
        }
        _ => format!("Unknown item: {}", item_name),
    }
}
