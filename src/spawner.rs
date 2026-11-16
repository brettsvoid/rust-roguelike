use bevy::prelude::*;
use rand::RngExt;

use crate::{
    abilities::HeroClass,
    combat::CombatStats,
    components::{HungerClock, HungerState, Name, RenderableBundle},
    entity_render::EntityShape,
    experience::{level_up, Experience},
    map::{Position, MAP_WIDTH},
    party::{PartyMember, HEROES},
    raws::{get_spawn_table_for_depth, spawn_class_kit, spawn_named_entity, Class, RAWS},
    rng::GameRng,
    shapes::Rect,
    viewshed::Viewshed,
};

const MAX_MONSTERS: i32 = 4;
const MAX_ITEMS: i32 = 2;

/// A fresh hero for `slot` at `(x, y)`, with the class's starting stats.
pub fn party_member(slot: usize, x: i32, y: i32, class: &Class) -> impl Bundle {
    let (name, colour, _) = HEROES[slot];
    (
        PartyMember { slot },
        Experience::at(1),
        Name { name: name.to_string() },
        HeroClass(class.name.clone()),
        Position { x, y },
        class_stats(class),
        Viewshed {
            range: 8,
            ..default()
        },
        HungerClock {
            state: HungerState::WellFed,
            duration: 200,
        },
        RenderableBundle::new(colour.into(), EntityShape::Creature),
    )
}

/// A level 1 hero's stats for `class`.
fn class_stats(class: &Class) -> CombatStats {
    let stats = &class.stats;
    CombatStats {
        max_hp: stats.max_hp,
        hp: stats.max_hp,
        armour: stats.armour,
        power: stats.power,
        movement: stats.movement,
        aim: stats.aim,
        defence: stats.defence,
    }
}

/// Spawns one hero per tile, in slot order, each with their class's gear and
/// abilities, and returns them.
pub fn spawn_party(commands: &mut Commands, tiles: &[(i32, i32)]) -> Vec<Entity> {
    let raws = RAWS.lock().unwrap();
    tiles
        .iter()
        .enumerate()
        .map(|(slot, &(x, y))| {
            let class = raws.class(HEROES[slot].2).expect("every hero's class is in the raws");
            let hero = commands.spawn(party_member(slot, x, y, class)).id();
            spawn_class_kit(&raws, commands, hero, class);
            hero
        })
        .collect()
}

/// A recruit hired into the empty `slot`: a fresh hero of `class` named
/// `name`, raised to `level`, with the class's gear and abilities.
pub fn spawn_recruit(commands: &mut Commands, slot: usize, (x, y): (i32, i32), name: &str, class: &str, level: i32) -> Entity {
    let raws = RAWS.lock().unwrap();
    let class = raws.class(class).expect("every recruit's class is in the raws");
    let mut stats = class_stats(class);
    for _ in 1..level {
        level_up(&mut stats, class.stats.max_hp);
    }
    let hero = commands.spawn(party_member(slot, x, y, class)).id();
    commands.entity(hero).insert((Name { name: name.to_string() }, stats, Experience::at(level)));
    spawn_class_kit(&raws, commands, hero, class);
    hero
}

pub fn spawn_room(
    commands: &mut Commands,
    rng: &mut GameRng,
    room: &Rect,
    monster_id: &mut usize,
    map_depth: i32,
) {
    // Calculate spawn counts based on depth
    let max_monsters_roll = (MAX_MONSTERS + 3) + map_depth - 3;
    let num_monsters = if max_monsters_roll > 0 {
        rng.0.random_range(1..=max_monsters_roll).max(0)
    } else {
        0
    };

    let max_items_roll = (MAX_ITEMS + 3) + map_depth - 3;
    let num_items = if max_items_roll > 0 {
        rng.0.random_range(1..=max_items_roll).max(0)
    } else {
        0
    };

    // Get spawn table from JSON data
    let raws = RAWS.lock().unwrap();
    let spawn_table = get_spawn_table_for_depth(&raws, map_depth);

    // Generate all spawn points
    let total_spawns = num_monsters + num_items;
    let mut spawn_points: Vec<(i32, i32)> = Vec::new();

    for _ in 0..total_spawns {
        let mut added = false;
        let mut attempts = 0;
        while !added && attempts < 20 {
            let x = rng.0.random_range(room.x1 + 1..=room.x2);
            let y = rng.0.random_range(room.y1 + 1..=room.y2);
            if !spawn_points.contains(&(x, y)) {
                spawn_points.push((x, y));
                added = true;
            }
            attempts += 1;
        }
    }

    // Spawn entities using weighted table
    for (x, y) in spawn_points.iter() {
        if let Some(entity_name) = spawn_table.roll(rng) {
            // Check if it's a monster (needs monster_id)
            let is_monster = raws.mob_index.contains_key(&entity_name);
            if is_monster {
                spawn_named_entity(&raws, commands, &entity_name, *x, *y, Some(*monster_id));
                *monster_id += 1;
            } else {
                spawn_named_entity(&raws, commands, &entity_name, *x, *y, None);
            }
        }
    }
}

/// Spawn entities in a region defined by tile indices (for non-rectangular areas like caves)
// Toolbox: not called since spawning moved into the builder chain; useful if
// region-based spawning comes back.
#[allow(dead_code)]
pub fn spawn_region(
    commands: &mut Commands,
    rng: &mut GameRng,
    tiles: &[usize],
    monster_id: &mut usize,
    map_depth: i32,
) {
    if tiles.is_empty() {
        return;
    }

    // Calculate spawn counts based on depth and region size
    let area_factor = (tiles.len() as f32 / 50.0).min(1.0); // Scale by region size
    let max_monsters_roll = (((MAX_MONSTERS + 3) + map_depth - 3) as f32 * area_factor) as i32;
    let num_monsters = if max_monsters_roll > 0 {
        rng.0.random_range(0..=max_monsters_roll)
    } else {
        0
    };

    let max_items_roll = (((MAX_ITEMS + 3) + map_depth - 3) as f32 * area_factor) as i32;
    let num_items = if max_items_roll > 0 {
        rng.0.random_range(0..=max_items_roll)
    } else {
        0
    };

    // Get spawn table from JSON data
    let raws = RAWS.lock().unwrap();
    let spawn_table = get_spawn_table_for_depth(&raws, map_depth);

    // Generate all spawn points
    let total_spawns = num_monsters + num_items;
    let mut spawn_points: Vec<usize> = Vec::new();

    for _ in 0..total_spawns {
        let mut attempts = 0;
        while attempts < 20 {
            let idx = tiles[rng.0.random_range(0..tiles.len())];
            if !spawn_points.contains(&idx) {
                spawn_points.push(idx);
                break;
            }
            attempts += 1;
        }
    }

    // Spawn entities using weighted table
    for idx in spawn_points.iter() {
        let x = (*idx % MAP_WIDTH) as i32;
        let y = (*idx / MAP_WIDTH) as i32;
        if let Some(entity_name) = spawn_table.roll(rng) {
            // Check if it's a monster (needs monster_id)
            let is_monster = raws.mob_index.contains_key(&entity_name);
            if is_monster {
                spawn_named_entity(&raws, commands, &entity_name, x, y, Some(*monster_id));
                *monster_id += 1;
            } else {
                spawn_named_entity(&raws, commands, &entity_name, x, y, None);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy::ecs::system::RunSystemOnce;

    use super::*;
    use crate::abilities::HeroAbility;
    use crate::components::{Equipped, InBackpack};

    #[test]
    fn a_new_party_is_one_of_each_class_with_its_stats_gear_and_abilities() {
        crate::raws::load_raws();
        let mut world = World::new();
        world
            .run_system_once(|mut commands: Commands| {
                spawn_party(&mut commands, &[(0, 0), (1, 0), (2, 0), (3, 0)]);
            })
            .unwrap();
        // Sorted: query order follows archetypes, not spawn order.
        let names = |world: &mut World, owned: &dyn Fn(&World, Entity) -> bool| -> Vec<String> {
            let mut q = world.query::<(Entity, &Name)>();
            let mut names: Vec<_> = q.iter(world).filter(|(e, _)| owned(world, *e)).map(|(_, n)| n.name.clone()).collect();
            names.sort();
            names
        };
        let mut heroes = world.query::<(Entity, &PartyMember, &HeroClass, &CombatStats)>();
        let mut heroes: Vec<_> = heroes
            .iter(&world)
            .map(|(e, m, c, s)| (m.slot, e, c.0.clone(), (s.hp, s.power, s.armour, s.aim, s.defence, s.movement)))
            .collect();
        heroes.sort_by_key(|h| h.0);
        let kits: Vec<_> = heroes
            .into_iter()
            .map(|(_, hero, class, stats)| {
                let worn = names(&mut world, &|w, e| w.get::<Equipped>(e).is_some_and(|q| q.owner == hero));
                let packed = names(&mut world, &|w, e| w.get::<InBackpack>(e).is_some_and(|b| b.owner == hero));
                let abilities = names(&mut world, &|w, e| {
                    w.get::<HeroAbility>(e).is_some() && w.get::<ChildOf>(e).is_some_and(|c| c.parent() == hero)
                });
                (class, stats, worn, packed, abilities)
            })
            .collect();
        let s = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(
            kits,
            vec![
                ("Fighter".into(), (34, 3, 2, 65, 5, 4), s(&["Longsword", "Shield"]), s(&[]), s(&["Cleave", "Second Wind"])),
                ("Ranger".into(), (28, 2, 1, 75, 10, 5), s(&["Shortbow"]), s(&["Dagger"]), s(&["Aimed Shot", "Snare"])),
                ("Mage".into(), (22, 1, 0, 70, 5, 4), s(&["Dagger"]), s(&[]), s(&["Fireball", "Magic Missile"])),
                ("Cleric".into(), (30, 2, 2, 65, 0, 4), s(&["Blessed Mace"]), s(&[]), s(&["Heal", "Smite"])),
            ]
        );
    }
}
