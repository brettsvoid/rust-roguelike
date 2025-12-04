use bevy::prelude::*;

use crate::{
    combat::CombatStats,
    components::{InBackpack, Name, Potion, WantsToDropItem, WantsToDrinkPotion, WantsToPickupItem},
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

pub fn potion_use_system(
    mut commands: Commands,
    mut gamelog: ResMut<GameLog>,
    drink_query: Query<(Entity, &WantsToDrinkPotion)>,
    potion_query: Query<(&Potion, &Name)>,
    mut stats_query: Query<&mut CombatStats>,
) {
    for (entity, wants_drink) in &drink_query {
        if let Ok((potion, potion_name)) = potion_query.get(wants_drink.potion) {
            if let Ok(mut stats) = stats_query.get_mut(entity) {
                stats.hp = (stats.hp + potion.heal_amount).min(stats.max_hp);
                gamelog.entries.push(format!(
                    "You drink the {}, healing {} hp.",
                    potion_name.name, potion.heal_amount
                ));
            }
            // Despawn the potion entity
            commands.entity(wants_drink.potion).despawn();
        }
        // Remove the intent component
        commands.entity(entity).remove::<WantsToDrinkPotion>();
    }
}

pub fn item_drop_system(
    mut commands: Commands,
    mut gamelog: ResMut<GameLog>,
    drop_query: Query<(Entity, &WantsToDropItem, &Position)>,
    name_query: Query<&Name>,
) {
    for (entity, wants_drop, dropper_pos) in &drop_query {
        // Remove from backpack
        commands.entity(wants_drop.item).remove::<InBackpack>();

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
