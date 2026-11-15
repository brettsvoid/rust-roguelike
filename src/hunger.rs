use bevy::prelude::*;

use crate::{
    combat::SufferDamage,
    components::{HungerClock, HungerState, Name},
    gamelog::GameLog,
    party::PartyMember,
};

pub fn hunger_system(
    mut commands: Commands,
    mut gamelog: ResMut<GameLog>,
    mut query: Query<(Entity, &Name, &mut HungerClock), With<PartyMember>>,
) {
    for (entity, name, mut clock) in &mut query {
        clock.duration -= 1;
        if clock.duration < 1 {
            match clock.state {
                HungerState::WellFed => {
                    clock.state = HungerState::Normal;
                    clock.duration = 200;
                    gamelog.entries.push(format!("{} is no longer well fed.", name.name));
                }
                HungerState::Normal => {
                    clock.state = HungerState::Hungry;
                    clock.duration = 200;
                    gamelog.entries.push(format!("{} is hungry.", name.name));
                }
                HungerState::Hungry => {
                    clock.state = HungerState::Starving;
                    clock.duration = 200;
                    gamelog.entries.push(format!("{} is starving!", name.name));
                }
                HungerState::Starving => {
                    // Deal 1 damage per turn
                    SufferDamage::new_damage(&mut commands, entity, 1);
                }
            }
        }
    }
}
