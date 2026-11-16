use bevy::prelude::*;

use crate::combat::CombatStats;
use crate::economy::town::{Sector, TownState};
use crate::economy::trade;
use crate::economy::Purse;
use crate::components::{Item, Name};
use crate::effects::{EffectKind, Effects};
use crate::levels::HEART_OF_THE_ABYSS;
use crate::map::{Map, Position, MAP_HEIGHT, MAP_WIDTH};
use crate::monsters::Monster;
use crate::party::PartyMember;

use super::resources::{DebugState, GodMode};

/// Execute a debug command and return the result message
pub fn execute_command(
    input: &str,
    commands: &mut Commands,
    debug_state: &mut DebugState,
    god_mode: &mut GodMode,
    party: &mut Query<(Entity, &mut Position, &mut CombatStats), With<PartyMember>>,
    active: Option<Entity>,
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
        Some("heal") => cmd_heal(party),
        Some("hurt") => cmd_hurt(&parts, party, active),
        Some("kill_all") => cmd_kill_all(commands, monster_query),
        Some("teleport") => cmd_teleport(&parts, party, active),
        Some("spawn") => cmd_spawn(&parts, commands, party, active),
        Some("place") => cmd_place(&parts, commands, party, active),
        Some("effect") => cmd_effect(&parts, commands, party, active),
        Some("town") => cmd_town(&parts, town),
        Some("gold") => cmd_gold(&parts, commands),
        Some(cmd) => format!("Unknown command: {}", cmd),
        None => String::new(),
    }
}

fn cmd_help() -> String {
    "Commands: spawn <item> (portal, heart, ...), effect <spark|fireball|missile|heal|confusion|blood>, place <dx> <dy> <raws name>, teleport <x> <y>, gold <n>, godmode, reveal, nofog, heal, hurt <n>, kill_all, town sell <sector> <value>, town days <n>, town level <sector> <n>".to_string()
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

/// Adds gold to the party's purse (a negative amount takes it away).
fn cmd_gold(parts: &[&str], commands: &mut Commands) -> String {
    match parts.get(1).map(|n| n.parse::<i32>()) {
        Some(Ok(amount)) => {
            commands.queue(move |world: &mut World| world.resource_mut::<Purse>().gold += amount);
            format!("Added {amount} gold to the purse")
        }
        _ => "Usage: gold <n>".to_string(),
    }
}

/// Takes `n` hit points off the active hero (never below 1), to try out healing.
fn cmd_hurt(
    parts: &[&str],
    party: &mut Query<(Entity, &mut Position, &mut CombatStats), With<PartyMember>>,
    active: Option<Entity>,
) -> String {
    let Some(Ok(n)) = parts.get(1).map(|n| n.parse::<i32>()) else {
        return "Usage: hurt <n>".to_string();
    };
    match active.and_then(|unit| party.get_mut(unit).ok()) {
        Some((.., mut stats)) => {
            stats.hp = (stats.hp - n).max(1);
            format!("Hurt to {}/{}", stats.hp, stats.max_hp)
        }
        None => "No active hero".to_string(),
    }
}

/// Heals the whole party.
fn cmd_heal(party: &mut Query<(Entity, &mut Position, &mut CombatStats), With<PartyMember>>) -> String {
    for (_, _, mut stats) in party.iter_mut() {
        stats.hp = stats.max_hp;
    }
    format!("Healed {} heroes", party.iter().count())
}

/// Drops every monster to 0 hp and lets `combat::delete_the_dead` take them,
/// as if they fell in a fight: each falls, leaves its gold and is logged.
fn cmd_kill_all(commands: &mut Commands, monster_query: &Query<Entity, With<Monster>>) -> String {
    let count = monster_query.iter().count();
    commands.queue(|world: &mut World| {
        let mut monsters = world.query_filtered::<&mut CombatStats, With<Monster>>();
        for mut stats in monsters.iter_mut(world) {
            stats.hp = 0;
        }
        if let Err(err) = world.run_system_cached(crate::combat::delete_the_dead) {
            warn!("kill_all: {err}");
        }
    });
    format!("Killed {} monsters", count)
}

fn cmd_teleport(
    parts: &[&str],
    party: &mut Query<(Entity, &mut Position, &mut CombatStats), With<PartyMember>>,
    active: Option<Entity>,
) -> String {
    if parts.len() < 3 {
        return "Usage: teleport <x> <y>".to_string();
    }
    let x = parts[1].parse::<i32>();
    let y = parts[2].parse::<i32>();
    match (x, y) {
        (Ok(x), Ok(y)) => {
            if x >= 0 && x < MAP_WIDTH as i32 && y >= 0 && y < MAP_HEIGHT as i32 {
                if let Some((_, mut pos, _)) = active.and_then(|unit| party.get_mut(unit).ok()) {
                    pos.x = x;
                    pos.y = y;
                    format!("Teleported to ({}, {})", x, y)
                } else {
                    "No active unit".to_string()
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
    party: &mut Query<(Entity, &mut Position, &mut CombatStats), With<PartyMember>>,
    active: Option<Entity>,
) -> String {
    if parts.len() < 2 {
        return "Usage: spawn <item_name>".to_string();
    }
    let item_name = parts[1].to_lowercase();

    let Some((_, pos, _)) = active.and_then(|unit| party.get(unit).ok()) else {
        return "No active unit".to_string();
    };
    let (x, y) = (pos.x, pos.y);

    // Map aliases to actual item names in spawns.json
    let actual_name = match item_name.as_str() {
        "health_potion" | "potion" => Some("Health Potion"),
        "magic_missile" | "missile" => Some("Magic Missile Scroll"),
        "fireball" => Some("Fireball Scroll"),
        "confusion" => Some("Confusion Scroll"),
        "dagger" => Some("Dagger"),
        "bow" | "shortbow" => Some("Shortbow"),
        "shield" => Some("Shield"),
        "longsword" | "sword" => Some("Longsword"),
        "tower_shield" => Some("Tower Shield"),
        "rations" | "food" => Some("Rations"),
        "magic_map" | "map" => Some("Scroll of Magic Mapping"),
        "gold" => Some("Gold"),
        "portal" | "town_portal" => Some("Town Portal"),
        "heart" => {
            // Not in the raws yet: a bare named item, enough to test the portal rule.
            commands.spawn((Item, Name { name: HEART_OF_THE_ABYSS.to_string() }, Position { x, y }));
            return format!("Spawned {HEART_OF_THE_ABYSS} at ({x}, {y})");
        }
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

/// Spawns any raws entry (prop, monster or item, by its exact name, spaces
/// allowed) at an offset from the active hero, to stage cover and fights.
fn cmd_place(
    parts: &[&str],
    commands: &mut Commands,
    party: &mut Query<(Entity, &mut Position, &mut CombatStats), With<PartyMember>>,
    active: Option<Entity>,
) -> String {
    let usage = "Usage: place <dx> <dy> <raws name>";
    let (Some(Ok(dx)), Some(Ok(dy))) = (parts.get(1).map(|n| n.parse::<i32>()), parts.get(2).map(|n| n.parse::<i32>()))
    else {
        return usage.to_string();
    };
    let name = parts.get(3..).unwrap_or_default().join(" ");
    let Some((_, pos, _)) = active.and_then(|unit| party.get(unit).ok()) else {
        return "No active unit".to_string();
    };
    let (x, y) = (pos.x + dx, pos.y + dy);
    let raws = crate::raws::RAWS.lock().unwrap();
    if crate::raws::spawn_named_entity(&raws, commands, &name, x, y, None) {
        format!("Placed {name} at ({x}, {y})")
    } else {
        format!("{usage} (no raws entry {name:?})")
    }
}

/// Plays an effect on the player's tile; the missile flies in from five tiles
/// east. `blood` stains the player's tile and the two east of it instead.
fn cmd_effect(
    parts: &[&str],
    commands: &mut Commands,
    party: &mut Query<(Entity, &mut Position, &mut CombatStats), With<PartyMember>>,
    active: Option<Entity>,
) -> String {
    let Some((_, pos, _)) = active.and_then(|unit| party.get(unit).ok()) else {
        return "No active unit".to_string();
    };
    let (x, y) = (pos.x, pos.y);
    let kind = match parts.get(1).copied() {
        Some("spark") => EffectKind::HitSpark,
        Some("fireball") => EffectKind::Fireball { radius: 3.0 },
        Some("missile") => EffectKind::MagicMissile { from: (x + 5, y) },
        Some("heal") => EffectKind::Healing,
        Some("confusion") => EffectKind::Confusion,
        Some("blood") => {
            commands.queue(move |world: &mut World| {
                let mut map = world.resource_mut::<Map>();
                for dx in 0..3 {
                    let idx = map.xy_idx(x + dx, y);
                    map.bloodstains.insert(idx);
                }
            });
            return format!("Bloodied ({x}, {y}) and two tiles east");
        }
        _ => return "Usage: effect <spark|fireball|missile|heal|confusion|blood>".to_string(),
    };
    commands.queue(move |world: &mut World| world.resource_mut::<Effects>().play(kind, x, y));
    format!("Playing {kind:?} at ({x}, {y})")
}
