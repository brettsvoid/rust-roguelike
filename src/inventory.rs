use bevy::prelude::*;

use crate::{
    combat::{CombatStats, SufferDamage},
    components::{
        AreaOfEffect, CausesConfusion, Confusion, Consumable, Equippable, Equipped, InBackpack,
        InflictsDamage, Name, ProvidesHealing, WantsToDropItem, WantsToPickupItem,
        WantsToRemoveItem, WantsToUseItem,
    },
    distance::DistanceAlg,
    gamelog::GameLog,
    map::Position,
};

pub fn item_collection_system(
    mut commands: Commands,
    mut gamelog: ResMut<GameLog>,
    pickup_query: Query<(Entity, &WantsToPickupItem)>,
    name_query: Query<&Name>,
) {
    for (entity, wants_pickup) in &pickup_query {
        // Remove position and hide item so it's no longer on the map
        commands
            .entity(wants_pickup.item)
            .remove::<Position>()
            .insert(Visibility::Hidden);

        // Add InBackpack component to mark it as in inventory
        commands.entity(wants_pickup.item).insert(InBackpack {
            owner: wants_pickup.collected_by,
        });

        // Remove the intent component
        commands.entity(entity).remove::<WantsToPickupItem>();

        // Log the pickup
        if let Ok(name) = name_query.get(wants_pickup.item) {
            gamelog.entries.push(format!("You pick up the {}.", name.name));
        }
    }
}

pub fn item_use_system(
    mut commands: Commands,
    mut gamelog: ResMut<GameLog>,
    use_query: Query<(Entity, &WantsToUseItem)>,
    consumable_query: Query<&Consumable>,
    healing_query: Query<(&ProvidesHealing, &Name)>,
    damage_query: Query<(&InflictsDamage, &Name)>,
    confusion_query: Query<(&CausesConfusion, &Name)>,
    aoe_query: Query<&AreaOfEffect>,
    equippable_query: Query<(&Equippable, &Name)>,
    equipped_query: Query<(Entity, &Equipped, &Name)>,
    mut stats_query: Query<&mut CombatStats>,
    target_query: Query<(Entity, &Position, &Name), With<CombatStats>>,
) {
    for (entity, wants_use) in &use_query {
        // Handle equippable items
        if let Ok((equippable, item_name)) = equippable_query.get(wants_use.item) {
            let target_slot = equippable.slot;

            // Find and unequip any item in the same slot owned by this entity
            for (equipped_entity, equipped, equipped_name) in &equipped_query {
                if equipped.owner == entity && equipped.slot == target_slot {
                    // Unequip: remove Equipped, add InBackpack
                    commands.entity(equipped_entity).remove::<Equipped>();
                    commands.entity(equipped_entity).insert(InBackpack { owner: entity });
                    gamelog.entries.push(format!("You unequip the {}.", equipped_name.name));
                }
            }

            // Equip the new item
            commands.entity(wants_use.item).remove::<InBackpack>();
            commands.entity(wants_use.item).insert(Equipped {
                owner: entity,
                slot: target_slot,
            });
            gamelog.entries.push(format!("You equip the {}.", item_name.name));

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
            }
        }

        // Apply effects if the item has a target position
        if let Some((target_x, target_y)) = wants_use.target {
            // Apply damage if the item inflicts it
            if let Ok((inflicts, item_name)) = damage_query.get(wants_use.item) {
                // Check if this is an AoE item
                if let Ok(aoe) = aoe_query.get(wants_use.item) {
                    // Find all entities within the AoE radius
                    let mut hit_count = 0;
                    for (target_entity, pos, target_name) in &target_query {
                        let distance = DistanceAlg::Pythagoras.distance2d(
                            Vec2::new(target_x as f32, target_y as f32),
                            Vec2::new(pos.x as f32, pos.y as f32),
                        );
                        if distance <= aoe.radius as f32 {
                            SufferDamage::new_damage(&mut commands, target_entity, inflicts.damage);
                            gamelog.entries.push(format!(
                                "{} hits {} for {} hp.",
                                item_name.name, target_name.name, inflicts.damage
                            ));
                            hit_count += 1;
                        }
                    }
                    if hit_count == 0 {
                        gamelog.entries.push(format!("{} explodes, but hits nothing.", item_name.name));
                    }
                } else {
                    // Single target - find entity at exact position
                    for (target_entity, pos, target_name) in &target_query {
                        if pos.x == target_x && pos.y == target_y {
                            SufferDamage::new_damage(&mut commands, target_entity, inflicts.damage);
                            gamelog.entries.push(format!(
                                "You use {} on {}, inflicting {} hp.",
                                item_name.name, target_name.name, inflicts.damage
                            ));
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
            gamelog.entries.push(format!("You unequip the {}.", name.name));
        }

        // Remove intent component
        commands.entity(entity).remove::<WantsToRemoveItem>();
    }
}
