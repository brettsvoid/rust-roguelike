use bevy::{color::palettes, prelude::*};
use rand::Rng;

use crate::{
    combat::CombatStats,
    components::{HungerClock, HungerState, Name, RenderOrder, RenderableBundle},
    map::{Position, MAP_WIDTH},
    player::Player,
    raws::{get_spawn_table_for_depth, spawn_named_entity, RAWS},
    rng::GameRng,
    shapes::Rect,
    viewshed::Viewshed,
};

const MAX_MONSTERS: i32 = 4;
const MAX_ITEMS: i32 = 2;

pub fn spawn_player(commands: &mut Commands, font: &TextFont, x: i32, y: i32) {
    commands.spawn((
        Player,
        Name {
            name: "Player".to_string(),
        },
        Position { x, y },
        CombatStats {
            max_hp: 30,
            hp: 30,
            defense: 2,
            power: 5,
        },
        Viewshed {
            range: 8,
            ..default()
        },
        HungerClock {
            state: HungerState::WellFed,
            duration: 200,
        },
        RenderableBundle::new(
            "☺",
            palettes::basic::YELLOW.into(),
            palettes::basic::BLACK.into(),
            RenderOrder::PLAYER,
            font,
        ),
    ));
}

pub fn spawn_room(
    commands: &mut Commands,
    rng: &mut GameRng,
    font: &TextFont,
    room: &Rect,
    monster_id: &mut usize,
    map_depth: i32,
) {
    // Calculate spawn counts based on depth
    let max_monsters_roll = (MAX_MONSTERS + 3) + (map_depth - 1) - 3;
    let num_monsters = if max_monsters_roll > 0 {
        rng.0.gen_range(1..=max_monsters_roll).max(0)
    } else {
        0
    };

    let max_items_roll = (MAX_ITEMS + 3) + (map_depth - 1) - 3;
    let num_items = if max_items_roll > 0 {
        rng.0.gen_range(1..=max_items_roll).max(0)
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
            let x = rng.0.gen_range(room.x1 + 1..=room.x2);
            let y = rng.0.gen_range(room.y1 + 1..=room.y2);
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
                spawn_named_entity(&raws, commands, font, &entity_name, *x, *y, Some(*monster_id));
                *monster_id += 1;
            } else {
                spawn_named_entity(&raws, commands, font, &entity_name, *x, *y, None);
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
    font: &TextFont,
    tiles: &[usize],
    monster_id: &mut usize,
    map_depth: i32,
) {
    if tiles.is_empty() {
        return;
    }

    // Calculate spawn counts based on depth and region size
    let area_factor = (tiles.len() as f32 / 50.0).min(1.0); // Scale by region size
    let max_monsters_roll = (((MAX_MONSTERS + 3) + (map_depth - 1) - 3) as f32 * area_factor) as i32;
    let num_monsters = if max_monsters_roll > 0 {
        rng.0.gen_range(0..=max_monsters_roll)
    } else {
        0
    };

    let max_items_roll = (((MAX_ITEMS + 3) + (map_depth - 1) - 3) as f32 * area_factor) as i32;
    let num_items = if max_items_roll > 0 {
        rng.0.gen_range(0..=max_items_roll)
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
            let idx = tiles[rng.0.gen_range(0..tiles.len())];
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
                spawn_named_entity(&raws, commands, font, &entity_name, x, y, Some(*monster_id));
                *monster_id += 1;
            } else {
                spawn_named_entity(&raws, commands, font, &entity_name, x, y, None);
            }
        }
    }
}
