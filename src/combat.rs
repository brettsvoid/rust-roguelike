use bevy::prelude::*;

use crate::components::{DefenseBonus, Equipped, HungerClock, HungerState, InBackpack, Item, MeleePowerBonus, Name};
use crate::debug::GodMode;
use crate::gamelog::GameLog;
use crate::economy::Gold;
use crate::map::{Map, Position};
use crate::particle::ParticleBuilder;
use crate::party::{next_unit, ActiveUnit, PartyMember};
use crate::raws::{spawn_named_item, RAWS};
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
    mut particle_builder: ResMut<ParticleBuilder>,
    query: Query<(Entity, &WantsToMelee, &Name, &CombatStats)>,
    targets: Query<(&Name, &CombatStats, &Position)>,
    melee_bonus_query: Query<(&Equipped, &MeleePowerBonus)>,
    defense_bonus_query: Query<(&Equipped, &DefenseBonus)>,
    hunger_query: Query<&HungerClock>,
) {
    for (entity, wants_melee, name, stats) in &query {
        if stats.hp <= 0 {
            continue;
        }

        if let Ok((target_name, target_stats, target_pos)) = targets.get(wants_melee.target) {
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

            // Add +1 power if well fed
            if let Ok(hunger) = hunger_query.get(entity) {
                if hunger.state == HungerState::WellFed {
                    offense_bonus += 1;
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

                // Spawn hit particle
                particle_builder.request(
                    target_pos.x,
                    target_pos.y,
                    "!",
                    Color::srgb(1.0, 0.5, 0.0), // Orange
                    200.0,
                );
            }
        }

        commands.entity(entity).remove::<WantsToMelee>();
    }
}

pub fn damage_system(
    mut commands: Commands,
    god_mode: Res<GodMode>,
    mut map: ResMut<Map>,
    mut query: Query<(Entity, &mut CombatStats, &SufferDamage)>,
    position_query: Query<&Position>,
    party: Query<(), With<PartyMember>>,
) {
    for (entity, mut stats, damage) in &mut query {
        // God mode spares the whole party
        if god_mode.0 && party.contains(entity) {
            commands.entity(entity).remove::<SufferDamage>();
            continue;
        }
        stats.hp -= damage.amount.iter().sum::<i32>();

        // Mark tile as bloody
        if let Ok(pos) = position_query.get(entity) {
            let idx = map.xy_idx(pos.x, pos.y);
            map.bloodstains.insert(idx);
        }

        commands.entity(entity).remove::<SufferDamage>();
    }
}

/// Despawns everything at 0 hp. A fallen hero drops what they carry and the
/// selection moves on; the run ends when the last one falls.
pub fn delete_the_dead(
    mut commands: Commands,
    mut log: ResMut<GameLog>,
    mut next_state: ResMut<NextState<RunState>>,
    mut active: ResMut<ActiveUnit>,
    query: Query<(Entity, &CombatStats, &Name, Option<&Position>, Option<&Gold>, Option<&PartyMember>)>,
    carried: Query<(Entity, Option<&InBackpack>, Option<&Equipped>), With<Item>>,
) {
    let mut party = Vec::new();
    for (entity, stats, name, pos, gold, member) in &query {
        if let Some(member) = member {
            party.push((member.slot, entity, stats.hp > 0));
        }
        if stats.hp > 0 {
            continue;
        }
        log.entries.push(format!("{} is dead", name.name));
        commands.entity(entity).despawn();

        if let (Some(pos), Some(&gold)) = (pos, gold) {
            let raws = RAWS.lock().unwrap();
            if let Some(pile) = spawn_named_item(&raws, &mut commands, "Gold", pos.x, pos.y) {
                commands.entity(pile).insert(gold);
            }
        }
        if let (Some(&Position { x, y }), Some(_)) = (pos, member) {
            let owned = |owner: Option<Entity>| owner == Some(entity);
            for (item, backpack, equipped) in &carried {
                if owned(backpack.map(|b| b.owner)) || owned(equipped.map(|e| e.owner)) {
                    commands
                        .entity(item)
                        .remove::<(InBackpack, Equipped)>()
                        .insert((Position { x, y }, Visibility::Inherited));
                }
            }
        }
    }

    if party.iter().any(|&(_, entity, alive)| !alive && active.0 == Some(entity)) {
        active.0 = next_unit(&party, active.0);
    }
    if !party.is_empty() && !party.iter().any(|&(_, _, alive)| alive) {
        log.entries.push("Your whole party is dead".to_string());
        // Delete save file on death (permadeath)
        saveload::delete_save_file();
        next_state.set_if_different(RunState::GameOver);
    }
}

#[cfg(test)]
mod tests {
    use bevy::ecs::system::RunSystemOnce;

    use super::*;

    fn hero(world: &mut World, slot: usize, hp: i32) -> Entity {
        world
            .spawn((
                PartyMember { slot },
                Name { name: format!("Hero {slot}") },
                Position { x: slot as i32, y: 0 },
                CombatStats {
                    max_hp: 30,
                    hp,
                    defense: 0,
                    power: 0,
                },
            ))
            .id()
    }

    fn game_over_pending(world: &World) -> bool {
        matches!(world.resource::<NextState<RunState>>(), NextState::PendingIfDifferent(RunState::GameOver))
    }

    #[test]
    fn a_fallen_hero_drops_their_kit_and_the_run_ends_with_the_last() {
        let mut world = World::new();
        world.insert_resource(GameLog { entries: Vec::new() });
        world.init_resource::<NextState<RunState>>();
        let first = hero(&mut world, 0, 0);
        let second = hero(&mut world, 1, 12);
        world.insert_resource(ActiveUnit(Some(first)));
        let dagger = world.spawn((Item, InBackpack { owner: first })).id();

        world.run_system_once(delete_the_dead).unwrap();

        assert!(world.get_entity(first).is_err(), "the fallen hero leaves play");
        assert_eq!(world.resource::<ActiveUnit>().0, Some(second));
        let dropped = world.get::<Position>(dagger).map(|p| (p.x, p.y));
        assert_eq!(dropped, Some((0, 0)));
        assert!(world.get::<InBackpack>(dagger).is_none());
        assert!(!game_over_pending(&world), "one hero still stands");

        world.get_mut::<CombatStats>(second).unwrap().hp = 0;
        world.run_system_once(delete_the_dead).unwrap();

        assert!(game_over_pending(&world));
        assert_eq!(world.resource::<ActiveUnit>().0, None);
        assert!(world.resource::<GameLog>().entries.contains(&"Your whole party is dead".to_string()));
    }
}
