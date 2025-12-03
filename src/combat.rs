use bevy::prelude::*;

use crate::components::Name;
use crate::player::Player;

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
    query: Query<(Entity, &WantsToMelee, &Name, &CombatStats)>,
    targets: Query<(&Name, &CombatStats)>,
) {
    for (entity, wants_melee, name, stats) in &query {
        if stats.hp <= 0 {
            continue;
        }

        if let Ok((target_name, target_stats)) = targets.get(wants_melee.target) {
            if target_stats.hp <= 0 {
                continue;
            }

            let damage = i32::max(0, stats.power - target_stats.defense);

            if damage == 0 {
                println!("{} is unable to hurt {}", name.name, target_name.name);
            } else {
                println!("{} hits {} for {} hp", name.name, target_name.name, damage);
                SufferDamage::new_damage(&mut commands, wants_melee.target, damage);
            }
        }

        commands.entity(entity).remove::<WantsToMelee>();
    }
}

pub fn damage_system(
    mut commands: Commands,
    mut query: Query<(Entity, &mut CombatStats, &SufferDamage)>,
) {
    for (entity, mut stats, damage) in &mut query {
        stats.hp -= damage.amount.iter().sum::<i32>();
        commands.entity(entity).remove::<SufferDamage>();
    }
}

pub fn delete_the_dead(
    mut commands: Commands,
    query: Query<(Entity, &CombatStats, &Name), Without<Player>>,
    player_query: Query<&CombatStats, With<Player>>,
) {
    for (entity, stats, name) in &query {
        if stats.hp <= 0 {
            println!("{} is dead", name.name);
            commands.entity(entity).despawn();
        }
    }

    if let Ok(player_stats) = player_query.get_single() {
        if player_stats.hp <= 0 {
            println!("You are dead");
        }
    }
}
