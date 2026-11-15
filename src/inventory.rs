use bevy::prelude::*;

use crate::{
    combat::{
        chebyshev, hit_chance, odds, roll_attack, roll_damage, spell_damage, CombatStats, Outcome, SufferDamage, DEFAULT_AIM,
        LEVEL_GROUND, SPELL_AIM_BONUS,
    },
    components::{
        AreaOfEffect, CausesConfusion, Confusion, Consumable, Equippable, Equipped, HungerClock,
        HungerState, InBackpack, InflictsDamage, MagicMapper, Name, ProvidesFood, ProvidesHealing,
        Ranged, WantsToDropItem, WantsToPickupItem, WantsToRemoveItem, WantsToUseItem, Weapon,
    },
    distance::DistanceAlg,
    economy::{Gold, Purse},
    gamelog::GameLog,
    cover::exposure,
    map::{Map, Position},
    effects::{EffectKind, Effects},
    rng::GameRng,
    PendingMagicMap,
};

pub fn item_collection_system(
    mut commands: Commands,
    mut gamelog: ResMut<GameLog>,
    mut purse: ResMut<Purse>,
    pickup_query: Query<(Entity, &WantsToPickupItem)>,
    name_query: Query<&Name>,
    gold_query: Query<&Gold>,
) {
    for (entity, wants_pickup) in &pickup_query {
        commands.entity(entity).remove::<WantsToPickupItem>();

        if let Ok(gold) = gold_query.get(wants_pickup.item) {
            purse.gold += gold.amount;
            commands.entity(wants_pickup.item).despawn();
            gamelog
                .entries
                .push(format!("You pick up {} gold.", gold.amount));
            continue;
        }

        // Remove position and hide item so it's no longer on the map
        commands
            .entity(wants_pickup.item)
            .remove::<Position>()
            .insert(Visibility::Hidden);

        // Add InBackpack component to mark it as in inventory
        commands.entity(wants_pickup.item).insert(InBackpack {
            owner: wants_pickup.collected_by,
        });

        // Log the pickup
        if let Ok(name) = name_query.get(wants_pickup.item) {
            gamelog
                .entries
                .push(format!("You pick up the {}.", name.name));
        }
    }
}

/// Damage scrolls roll damage through `combat::roll_damage`, ignoring armour.
/// A single-target one also rolls to hit like a shot, with `SPELL_AIM_BONUS`;
/// an area one hits everything in its radius. Confusion always lands.
pub fn item_use_system(
    mut commands: Commands,
    mut gamelog: ResMut<GameLog>,
    (mut effects, mut rng, map): (ResMut<Effects>, ResMut<GameRng>, Res<Map>),
    mut pending_magic_map: ResMut<PendingMagicMap>,
    use_query: Query<(Entity, &WantsToUseItem, &Position)>,
    consumable_query: Query<&Consumable>,
    healing_query: Query<(&ProvidesHealing, &Name)>,
    damage_query: Query<(&InflictsDamage, &Name, Option<&Ranged>)>,
    confusion_query: Query<(&CausesConfusion, &Name)>,
    food_mapper_query: Query<(Option<&ProvidesFood>, Option<&MagicMapper>)>,
    aoe_query: Query<&AreaOfEffect>,
    equippable_query: Query<(&Equippable, &Name)>,
    equipped_query: Query<(Entity, &Equipped, &Name)>,
    mut stats_query: Query<&mut CombatStats>,
    mut hunger_query: Query<&mut HungerClock>,
    target_query: Query<(Entity, &Position, &Name), With<CombatStats>>,
) {
    for (entity, wants_use, user_pos) in &use_query {
        // Handle equippable items
        if let Ok((equippable, item_name)) = equippable_query.get(wants_use.item) {
            let target_slot = equippable.slot;

            // Find and unequip any item in the same slot owned by this entity
            for (equipped_entity, equipped, equipped_name) in &equipped_query {
                if equipped.owner == entity && equipped.slot == target_slot {
                    // Unequip: remove Equipped, add InBackpack
                    commands.entity(equipped_entity).remove::<Equipped>();
                    commands
                        .entity(equipped_entity)
                        .insert(InBackpack { owner: entity });
                    gamelog
                        .entries
                        .push(format!("You unequip the {}.", equipped_name.name));
                }
            }

            // Equip the new item
            commands.entity(wants_use.item).remove::<InBackpack>();
            commands.entity(wants_use.item).insert(Equipped {
                owner: entity,
                slot: target_slot,
            });
            gamelog
                .entries
                .push(format!("You equip the {}.", item_name.name));

            // Remove the intent and continue to next item
            commands.entity(entity).remove::<WantsToUseItem>();
            continue;
        }
        // Apply healing if the item provides it
        if let Ok((healing, item_name)) = healing_query.get(wants_use.item) {
            if let Ok(mut stats) = stats_query.get_mut(entity) {
                stats.hp = (stats.hp + healing.heal_amount).min(stats.max_hp);
                gamelog.entries.push(format!(
                    "You drink the {}, healing {} hp.",
                    item_name.name, healing.heal_amount
                ));

                effects.play(EffectKind::Healing, user_pos.x, user_pos.y);
            }
        }

        // Apply food or magic mapper effects
        if let Ok((food, mapper)) = food_mapper_query.get(wants_use.item) {
            if food.is_some() {
                if let Ok(mut hunger) = hunger_query.get_mut(entity) {
                    hunger.state = HungerState::WellFed;
                    hunger.duration = 200;
                    gamelog.entries.push("You eat the rations.".to_string());
                }
            }
            if mapper.is_some() {
                gamelog
                    .entries
                    .push("The map is revealed to your mind!".to_string());
                pending_magic_map.0 = true;
            }
        }

        // Apply effects if the item has a target position
        if let Some((target_x, target_y)) = wants_use.target {
            // Apply damage if the item inflicts it
            if let Ok((inflicts, item_name, ranged)) = damage_query.get(wants_use.item) {
                let damage = spell_damage(inflicts.damage);
                // Check if this is an AoE item
                if let Ok(aoe) = aoe_query.get(wants_use.item) {
                    let radius = aoe.radius as f32;
                    effects.play(EffectKind::Fireball { radius }, target_x, target_y);

                    // Find all entities within the AoE radius
                    let mut hit_count = 0;
                    for (target_entity, pos, target_name) in &target_query {
                        let distance = DistanceAlg::Euclidean.distance2d(
                            Vec2::new(target_x as f32, target_y as f32),
                            Vec2::new(pos.x as f32, pos.y as f32),
                        );
                        if distance <= aoe.radius as f32 {
                            let amount = roll_damage(&mut rng, damage, 0, 0, false);
                            SufferDamage::new_damage(&mut commands, target_entity, amount);
                            gamelog.entries.push(format!(
                                "{} hits {} for {} hp.",
                                item_name.name, target_name.name, amount
                            ));
                            hit_count += 1;
                        }
                    }
                    if hit_count == 0 {
                        gamelog
                            .entries
                            .push(format!("{} explodes, but hits nothing.", item_name.name));
                    }
                } else {
                    // Single target - find entity at exact position
                    let from = (user_pos.x, user_pos.y);
                    let caster = target_query.get(entity).map_or("You", |(.., name)| name.name.as_str());
                    let aim = stats_query.get(entity).map_or(DEFAULT_AIM, |s| s.aim) + SPELL_AIM_BONUS;
                    // The targeting screen has already checked the range.
                    let range = ranged.map_or(1, |r| r.range);
                    let spell = Weapon { damage, range, optimal_range: range, crit_chance: 0 };
                    for (target_entity, pos, target_name) in &target_query {
                        if pos.x == target_x && pos.y == target_y {
                            effects.play(EffectKind::MagicMissile { from }, pos.x, pos.y);
                            let defence = stats_query.get(target_entity).map_or(0, |s| s.defence);
                            let distance = chebyshev(from, (pos.x, pos.y));
                            let cover = exposure(&map, (pos.x, pos.y), from);
                            let chance = hit_chance(aim, defence, distance, &spell, cover.aim_penalty(), LEVEL_GROUND);
                            let odds = odds(chance, cover);
                            let (item, target) = (&item_name.name, &target_name.name);
                            if roll_attack(&mut rng, chance, spell.crit_chance) == Outcome::Miss {
                                gamelog.entries.push(format!("{caster} casts {item} at {target}: miss ({odds})"));
                            } else {
                                let amount = roll_damage(&mut rng, damage, 0, 0, false);
                                SufferDamage::new_damage(&mut commands, target_entity, amount);
                                gamelog
                                    .entries
                                    .push(format!("{caster} casts {item} at {target}: hit for {amount} ({odds})"));
                            }
                            break;
                        }
                    }
                }
            }

            // Apply confusion if the item causes it
            if let Ok((causes_confusion, item_name)) = confusion_query.get(wants_use.item) {
                // Find entity at target position
                for (target_entity, pos, target_name) in &target_query {
                    if pos.x == target_x && pos.y == target_y {
                        commands.entity(target_entity).insert(Confusion {
                            turns: causes_confusion.turns,
                        });
                        gamelog.entries.push(format!(
                            "You use {} on {}, confusing them for {} turns.",
                            item_name.name, target_name.name, causes_confusion.turns
                        ));

                        effects.play(EffectKind::Confusion, pos.x, pos.y);
                        break;
                    }
                }
            }
        }

        // If consumable, destroy the item
        if consumable_query.get(wants_use.item).is_ok() {
            commands.entity(wants_use.item).despawn();
        }

        // Remove the intent component
        commands.entity(entity).remove::<WantsToUseItem>();
    }
}

pub fn item_drop_system(
    mut commands: Commands,
    mut gamelog: ResMut<GameLog>,
    drop_query: Query<(Entity, &WantsToDropItem, &Position)>,
    name_query: Query<&Name>,
) {
    for (entity, wants_drop, dropper_pos) in &drop_query {
        // Remove from backpack and make visible again
        commands
            .entity(wants_drop.item)
            .remove::<InBackpack>()
            .insert(Visibility::Inherited);

        // Add position to place item on ground at dropper's location
        commands.entity(wants_drop.item).insert(Position {
            x: dropper_pos.x,
            y: dropper_pos.y,
        });

        // Log the drop
        if let Ok(name) = name_query.get(wants_drop.item) {
            gamelog.entries.push(format!("You drop the {}.", name.name));
        }

        // Remove intent component
        commands.entity(entity).remove::<WantsToDropItem>();
    }
}

pub fn item_remove_system(
    mut commands: Commands,
    mut gamelog: ResMut<GameLog>,
    remove_query: Query<(Entity, &WantsToRemoveItem)>,
    name_query: Query<&Name>,
) {
    for (entity, wants_remove) in &remove_query {
        // Unequip: remove Equipped, add InBackpack
        commands.entity(wants_remove.item).remove::<Equipped>();
        commands
            .entity(wants_remove.item)
            .insert(InBackpack { owner: entity });

        // Log the removal
        if let Ok(name) = name_query.get(wants_remove.item) {
            gamelog
                .entries
                .push(format!("You unequip the {}.", name.name));
        }

        // Remove intent component
        commands.entity(entity).remove::<WantsToRemoveItem>();
    }
}

#[cfg(test)]
mod tests {
    use bevy::ecs::system::RunSystemOnce;

    use super::*;
    use crate::components::Item;

    fn pickup(world: &mut World, item: Entity) {
        let player = world.spawn_empty().id();
        world.entity_mut(player).insert(WantsToPickupItem {
            collected_by: player,
            item,
        });
        world.run_system_once(item_collection_system).unwrap();
    }

    fn world_with_purse(gold: i32) -> World {
        let mut world = World::new();
        world.insert_resource(GameLog { entries: Vec::new() });
        world.insert_resource(Purse { gold });
        world
    }

    #[test]
    fn picking_up_gold_adds_it_to_the_purse() {
        let mut world = world_with_purse(3);
        let pile = world
            .spawn((Item, Gold { amount: 10 }, Position { x: 1, y: 1 }))
            .id();

        pickup(&mut world, pile);

        assert_eq!(world.resource::<Purse>().gold, 13);
        assert!(world.get_entity(pile).is_err(), "the pile is used up");
        assert_eq!(world.resource::<GameLog>().entries, vec!["You pick up 10 gold."]);
    }

    #[test]
    fn picking_up_an_item_leaves_the_purse_alone() {
        let mut world = world_with_purse(3);
        let dagger = world
            .spawn((Item, Name { name: "Dagger".into() }, Position { x: 1, y: 1 }))
            .id();

        pickup(&mut world, dagger);

        assert_eq!(world.resource::<Purse>().gold, 3);
        assert!(world.get::<InBackpack>(dagger).is_some());
    }
}
