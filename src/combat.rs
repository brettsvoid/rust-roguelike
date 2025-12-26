use bevy::prelude::*;

use crate::components::{DefenseBonus, Equipped, MeleePowerBonus, Name};
use crate::debug::GodMode;
use crate::gamelog::GameLog;
use crate::player::Player;
use crate::saveload;
use crate::RunState;

#[derive(Component, Debug)]
pub struct CombatStats {
    pub max_hp: i32,
    pub hp: i32,
    pub defense: i32,
    pub power: i32,
}

#[derive(Component, Debug)]
pub struct WantsToMelee {
    pub target: Entity,
}

#[derive(Component, Debug, Default)]
pub struct SufferDamage {
    pub amount: Vec<i32>,
}

impl SufferDamage {
    pub fn new_damage(commands: &mut Commands, victim: Entity, amount: i32) {
        commands.entity(victim).entry::<SufferDamage>().or_default().and_modify(move |mut d| {
            d.amount.push(amount);
        });
    }
}

pub fn melee_combat_system(
    mut commands: Commands,
    mut log: ResMut<GameLog>,
    query: Query<(Entity, &WantsToMelee, &Name, &CombatStats)>,
    targets: Query<(&Name, &CombatStats)>,
    melee_bonus_query: Query<(&Equipped, &MeleePowerBonus)>,
    defense_bonus_query: Query<(&Equipped, &DefenseBonus)>,
) {
    for (entity, wants_melee, name, stats) in &query {
        if stats.hp <= 0 {
            continue;
        }

        if let Ok((target_name, target_stats)) = targets.get(wants_melee.target) {
            if target_stats.hp <= 0 {
                continue;
            }

            // Calculate attacker's power bonus from equipment
            let mut offense_bonus = 0;
            for (equipped, bonus) in &melee_bonus_query {
                if equipped.owner == entity {
                    offense_bonus += bonus.power;
                }
            }

            // Calculate defender's defense bonus from equipment
            let mut defense_bonus = 0;
            for (equipped, bonus) in &defense_bonus_query {
                if equipped.owner == wants_melee.target {
                    defense_bonus += bonus.defense;
                }
            }

            let damage = i32::max(
                0,
                (stats.power + offense_bonus) - (target_stats.defense + defense_bonus),
            );

            if damage == 0 {
                log.entries.push(format!(
                    "{} is unable to hurt {}",
                    name.name, target_name.name
                ));
            } else {
                log.entries.push(format!(
                    "{} hits {} for {} hp",
                    name.name, target_name.name, damage
                ));
                SufferDamage::new_damage(&mut commands, wants_melee.target, damage);
            }
        }

        commands.entity(entity).remove::<WantsToMelee>();
    }
}

pub fn damage_system(
    mut commands: Commands,
    god_mode: Res<GodMode>,
    mut query: Query<(Entity, &mut CombatStats, &SufferDamage)>,
    player_query: Query<Entity, With<Player>>,
) {
    let player = player_query.get_single().ok();

    for (entity, mut stats, damage) in &mut query {
        // Skip damage to player if god mode is on
        if god_mode.0 && Some(entity) == player {
            commands.entity(entity).remove::<SufferDamage>();
            continue;
        }
        stats.hp -= damage.amount.iter().sum::<i32>();
        commands.entity(entity).remove::<SufferDamage>();
    }
}

pub fn delete_the_dead(
    mut commands: Commands,
    mut log: ResMut<GameLog>,
    mut next_state: ResMut<NextState<RunState>>,
    query: Query<(Entity, &CombatStats, &Name), Without<Player>>,
    player_query: Query<&CombatStats, With<Player>>,
) {
    for (entity, stats, name) in &query {
        if stats.hp <= 0 {
            log.entries.push(format!("{} is dead", name.name));
            commands.entity(entity).despawn();
        }
    }

    if let Ok(player_stats) = player_query.get_single() {
        if player_stats.hp <= 0 {
            log.entries.push("You are dead".to_string());
            // Delete save file on death (permadeath)
            saveload::delete_save_file();
            // Transition to game over screen
            next_state.set(RunState::GameOver);
        }
    }
}
