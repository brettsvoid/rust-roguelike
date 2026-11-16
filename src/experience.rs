//! Experience and levels (C11). Every hostile that dies gives its `XpValue`
//! (raws `xp`, else `mob_xp`) to each living hero in full. Crossing a
//! threshold (`xp_for`) raises the hero a level: more hit points (a fifth of
//! the class's base, healed at once) and more aim.

use bevy::prelude::*;

use crate::abilities::HeroClass;
use crate::combat::{CombatStats, Outcome};
use crate::components::Name;
use crate::gamelog::GameLog;
use crate::map::Position;
use crate::party::PartyMember;
use crate::presentation::{Beat, Presentation};
use crate::raws::RAWS;

/// Total experience between level 1 and 2; each later level costs one step more.
pub const XP_STEP: i32 = 50;
/// Aim gained per level.
pub const LEVEL_AIM: i32 = 2;

/// A hero's level and the experience gathered so far (a running total).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Experience {
    pub level: i32,
    pub xp: i32,
}

impl Experience {
    /// A hero starting at `level`, with just the experience it takes.
    pub fn at(level: i32) -> Self {
        Experience { level, xp: xp_for(level) }
    }
}

/// The experience a monster gives when it dies.
#[derive(Component, Clone, Copy, Debug)]
pub struct XpValue(pub i32);

/// What a monster without a raws `xp` is worth: half its hit points plus its power.
pub fn mob_xp(max_hp: i32, power: i32) -> i32 {
    max_hp / 2 + power
}

/// The total experience needed to reach `level`: 0, 50, 150, 300, 500, ...
pub fn xp_for(level: i32) -> i32 {
    XP_STEP * (level - 1) * level / 2
}

/// The level `xp` total experience reaches.
pub fn level_for(xp: i32) -> i32 {
    (1..).find(|&level| xp_for(level + 1) > xp).unwrap()
}

/// One level's gains on `stats`, for a class whose base hit points are `class_max_hp`.
pub fn level_up(stats: &mut CombatStats, class_max_hp: i32) {
    let hp = class_max_hp / 5;
    stats.max_hp += hp;
    stats.hp += hp;
    stats.aim += LEVEL_AIM;
}

/// Adds `xp` to every living hero, logging it, and raises those who cross a
/// threshold, each with a floater over them. Run through
/// `Commands::run_system_cached_with` (deaths, the `xp` console command).
pub fn grant_xp(
    In(xp): In<i32>,
    mut log: ResMut<GameLog>,
    mut presentation: ResMut<Presentation>,
    mut heroes: Query<(&Name, &Position, Option<&HeroClass>, &mut CombatStats, &mut Experience), With<PartyMember>>,
) {
    log.entries.push(format!("The party gains {xp} experience."));
    let raws = RAWS.lock().unwrap();
    for (name, pos, class, mut stats, mut experience) in &mut heroes {
        if stats.hp <= 0 {
            continue;
        }
        experience.xp += xp;
        let base_hp = class.and_then(|c| raws.class(&c.0)).map_or(stats.max_hp, |c| c.stats.max_hp);
        while experience.level < level_for(experience.xp) {
            experience.level += 1;
            level_up(&mut stats, base_hp);
            log.entries.push(format!("{} reaches level {}.", name.name, experience.level));
            presentation.push(Beat::Floater { at: (pos.x, pos.y), outcome: Outcome::LevelUp, damage: experience.level });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thresholds_grow_by_one_step_a_level() {
        assert_eq!((1..=5).map(xp_for).collect::<Vec<_>>(), vec![0, 50, 150, 300, 500]);
        assert_eq!([0, 49, 50, 149, 150, 499, 500].map(level_for), [1, 1, 2, 2, 3, 4, 5]);
        assert_eq!(Experience::at(3), Experience { level: 3, xp: 150 });
    }

    #[test]
    fn monsters_are_worth_half_their_hit_points_plus_their_power() {
        assert_eq!(mob_xp(16, 4), 12); // Goblin
        assert_eq!(mob_xp(2, 1), 2); // Rat
        assert_eq!(mob_xp(70, 15), 50); // Demon Lord
    }

    #[test]
    fn a_level_adds_a_fifth_of_the_class_hit_points_and_aim() {
        let mut stats = CombatStats { max_hp: 34, hp: 20, armour: 2, power: 3, movement: 4, aim: 65, defence: 5 };
        level_up(&mut stats, 34);
        assert_eq!((stats.max_hp, stats.hp, stats.aim, stats.power), (40, 26, 67, 3));
    }
}
