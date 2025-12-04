use bevy::{color::palettes, prelude::*};
use rand::Rng;

use crate::{
    combat::CombatStats,
    components::{BlocksTile, Item, Name, Potion, RenderOrder, RenderableBundle},
    map::Position,
    monsters::Monster,
    player::Player,
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
        RenderableBundle::new("☺", palettes::basic::YELLOW.into(), palettes::basic::BLACK.into(), RenderOrder::PLAYER, font),
    ));
}

pub fn spawn_room(commands: &mut Commands, rng: &mut GameRng, font: &TextFont, room: &Rect, monster_id: &mut usize) {
    let num_monsters = rng.0.gen_range(0..=MAX_MONSTERS);
    let num_items = rng.0.gen_range(0..=MAX_ITEMS);

    let mut spawn_points: Vec<(i32, i32)> = Vec::new();

    // Generate monster spawn points
    for _ in 0..num_monsters {
        let mut added = false;
        while !added {
            let x = rng.0.gen_range(room.x1 + 1..=room.x2);
            let y = rng.0.gen_range(room.y1 + 1..=room.y2);
            if !spawn_points.contains(&(x, y)) {
                spawn_points.push((x, y));
                added = true;
            }
        }
    }

    // Spawn monsters
    for (x, y) in spawn_points.iter() {
        spawn_random_monster(commands, rng, font, *x, *y, *monster_id);
        *monster_id += 1;
    }

    // Generate item spawn points
    let mut item_spawn_points: Vec<(i32, i32)> = Vec::new();
    for _ in 0..num_items {
        let mut added = false;
        while !added {
            let x = rng.0.gen_range(room.x1 + 1..=room.x2);
            let y = rng.0.gen_range(room.y1 + 1..=room.y2);
            if !spawn_points.contains(&(x, y)) && !item_spawn_points.contains(&(x, y)) {
                item_spawn_points.push((x, y));
                added = true;
            }
        }
    }

    // Spawn items
    for (x, y) in item_spawn_points.iter() {
        spawn_health_potion(commands, font, *x, *y);
    }
}

fn spawn_random_monster(commands: &mut Commands, rng: &mut GameRng, font: &TextFont, x: i32, y: i32, id: usize) {
    let roll = rng.0.gen_range(0..=1);
    match roll {
        0 => spawn_orc(commands, font, x, y, id),
        _ => spawn_goblin(commands, font, x, y, id),
    }
}

fn spawn_orc(commands: &mut Commands, font: &TextFont, x: i32, y: i32, id: usize) {
    spawn_monster(commands, font, x, y, "o", &format!("Orc #{}", id));
}

fn spawn_goblin(commands: &mut Commands, font: &TextFont, x: i32, y: i32, id: usize) {
    spawn_monster(commands, font, x, y, "g", &format!("Goblin #{}", id));
}

fn spawn_monster(commands: &mut Commands, font: &TextFont, x: i32, y: i32, glyph: &str, name: &str) {
    commands.spawn((
        Monster,
        Name {
            name: name.to_string(),
        },
        Position { x, y },
        BlocksTile,
        CombatStats {
            max_hp: 16,
            hp: 16,
            defense: 1,
            power: 4,
        },
        Viewshed {
            range: 8,
            ..default()
        },
        RenderableBundle::new(glyph, palettes::basic::RED.into(), palettes::basic::BLACK.into(), RenderOrder::MONSTER, font),
    ));
}

fn spawn_health_potion(commands: &mut Commands, font: &TextFont, x: i32, y: i32) {
    commands.spawn((
        Item,
        Potion { heal_amount: 8 },
        Name {
            name: "Health Potion".to_string(),
        },
        Position { x, y },
        RenderableBundle::new("¡", palettes::basic::FUCHSIA.into(), palettes::basic::BLACK.into(), RenderOrder::ITEM, font),
    ));
}
