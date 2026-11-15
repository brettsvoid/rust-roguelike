use bevy::prelude::*;

use crate::combat::CombatStats;
use crate::economy::town::{Sector, TownState};
use crate::economy::trade;
use crate::map::{Position, MAP_HEIGHT, MAP_WIDTH};
use crate::monsters::Monster;
use crate::player::Player;

use super::resources::{DebugState, GodMode};

/// Execute a debug command and return the result message
pub fn execute_command(
    input: &str,
    commands: &mut Commands,
    debug_state: &mut DebugState,
    god_mode: &mut GodMode,
    player_query: &mut Query<(Entity, &mut Position, &mut CombatStats), With<Player>>,
    monster_query: &Query<Entity, With<Monster>>,
    town: &mut ResMut<TownState>,
) -> String {
    let parts: Vec<&str> = input.split_whitespace().collect();
    let cmd = parts.first().map(|s| s.to_lowercase());

    match cmd.as_deref() {
        Some("help") => cmd_help(),
        Some("godmode") => cmd_godmode(god_mode),
        Some("reveal") => cmd_reveal(debug_state),
        Some("nofog") => cmd_nofog(debug_state),
        Some("heal") => cmd_heal(player_query),
        Some("kill_all") => cmd_kill_all(commands, monster_query),
        Some("teleport") => cmd_teleport(&parts, player_query),
        Some("spawn") => cmd_spawn(&parts, commands, player_query),
        Some("town") => cmd_town(&parts, town),
        Some(cmd) => format!("Unknown command: {}", cmd),
        None => String::new(),
    }
}

fn cmd_help() -> String {
    "Commands: spawn <item>, teleport <x> <y>, godmode, reveal, nofog, heal, kill_all, town sell <sector> <value>, town days <n>, town level <sector> <n>".to_string()
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

/// Drive the town without vendors or time passing: record a sale, run days
/// or set a sector's level outright.
pub(super) fn cmd_town(parts: &[&str], town: &mut ResMut<TownState>) -> String {
    match parts[1..] {
        ["level", sector, level] => {
            let sector = Sector::ALL
                .into_iter()
                .find(|s| s.name().eq_ignore_ascii_case(sector));
            match (sector, level.parse::<u32>()) {
                (Some(sector), Ok(level @ 1..=5)) => {
                    town.sectors.get_mut(&sector).unwrap().level = level;
                    format!("{} is now level {level}", sector.name())
                }
                _ => "Usage: town level <sector> <1-5>".to_string(),
            }
        }
        ["sell", sector, value] => {
            let sector = Sector::ALL
                .into_iter()
                .find(|s| s.name().eq_ignore_ascii_case(sector));
            match (sector, value.parse::<i32>()) {
                (Some(sector), Ok(value)) if value > 0 => {
                    trade::record_sale(town, sector, value, 1);
                    format!("Sold {value} gold of {} to the town", sector.goods())
                }
                _ => "Usage: town sell <smithing|alchemy|textiles|provisions|faith|arcana> <value>".to_string(),
            }
        }
        ["days", n] => match n.parse::<u32>() {
            Ok(n) => format!("{n} days passed, {} news", trade::simulate(town, n).len()),
            Err(_) => "Usage: town days <n>".to_string(),
        },
        _ => "Usage: town sell <sector> <value> | town days <n> | town level <sector> <n>".to_string(),
    }
}

fn cmd_heal(
    player_query: &mut Query<(Entity, &mut Position, &mut CombatStats), With<Player>>,
) -> String {
    if let Ok((_, _, mut stats)) = player_query.single_mut() {
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
                if let Ok((_, mut pos, _)) = player_query.single_mut() {
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
    player_query: &mut Query<(Entity, &mut Position, &mut CombatStats), With<Player>>,
) -> String {
    if parts.len() < 2 {
        return "Usage: spawn <item_name>".to_string();
    }
    let item_name = parts[1].to_lowercase();

    let Ok((_, pos, _)) = player_query.single_mut() else {
        return "No player found".to_string();
    };
    let (x, y) = (pos.x, pos.y);

    // Map aliases to actual item names in spawns.json
    let actual_name = match item_name.as_str() {
        "health_potion" | "potion" => Some("Health Potion"),
        "magic_missile" | "missile" => Some("Magic Missile Scroll"),
        "fireball" => Some("Fireball Scroll"),
        "confusion" => Some("Confusion Scroll"),
        "dagger" => Some("Dagger"),
        "shield" => Some("Shield"),
        "longsword" | "sword" => Some("Longsword"),
        "tower_shield" => Some("Tower Shield"),
        "rations" | "food" => Some("Rations"),
        "magic_map" | "map" => Some("Scroll of Magic Mapping"),
        "gold" => Some("Gold"),
        _ => None,
    };

    match actual_name {
        Some(name) => {
            let raws = crate::raws::RAWS.lock().unwrap();
            if crate::raws::spawn_named_entity(&raws, commands, name, x, y, None) {
                format!("Spawned {} at ({}, {})", name, x, y)
            } else {
                format!("Failed to spawn: {}", name)
            }
        }
        None => format!("Unknown item: {}", item_name),
    }
}
