use bevy::prelude::*;

use crate::{
    combat::SufferDamage,
    components::{HungerClock, HungerState},
    gamelog::GameLog,
    player::Player,
};

pub fn hunger_system(
    mut commands: Commands,
    mut gamelog: ResMut<GameLog>,
    mut query: Query<(Entity, &mut HungerClock), With<Player>>,
) {
    for (entity, mut clock) in &mut query {
        clock.duration -= 1;
        if clock.duration < 1 {
            match clock.state {
                HungerState::WellFed => {
                    clock.state = HungerState::Normal;
                    clock.duration = 200;
                    gamelog
                        .entries
                        .push("You are no longer well fed.".to_string());
                }
                HungerState::Normal => {
                    clock.state = HungerState::Hungry;
                    clock.duration = 200;
                    gamelog.entries.push("You are hungry.".to_string());
                }
                HungerState::Hungry => {
                    clock.state = HungerState::Starving;
                    clock.duration = 200;
                    gamelog.entries.push("You are starving!".to_string());
                }
                HungerState::Starving => {
                    // Deal 1 damage per turn
                    SufferDamage::new_damage(&mut commands, entity, 1);
                }
            }
        }
    }
}
