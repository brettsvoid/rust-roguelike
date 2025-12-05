use bevy::{color::palettes, prelude::*};
use rand::Rng;

use crate::{
    combat::CombatStats,
    components::{AreaOfEffect, BlocksTile, CausesConfusion, Consumable, InflictsDamage, Item, Name, ProvidesHealing, Ranged, RenderOrder, RenderableBundle, Targeting},
    map::Position,
    monsters::Monster,
    player::Player,
    rng::GameRng,
    shapes::Rect,
    viewshed::Viewshed,
};

const MAX_MONSTERS: i32 = 3;
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
        spawn_random_item(commands, rng, font, *x, *y);
    }
}

fn spawn_random_item(commands: &mut Commands, rng: &mut GameRng, font: &TextFont, x: i32, y: i32) {
    let roll = rng.0.gen_range(0..=3);
    match roll {
        0 => spawn_health_potion(commands, font, x, y),
        1 => spawn_magic_missile_scroll(commands, font, x, y),
        2 => spawn_fireball_scroll(commands, font, x, y),
        _ => spawn_confusion_scroll(commands, font, x, y),
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
        Consumable,
        ProvidesHealing { heal_amount: 8 },
        Name {
            name: "Health Potion".to_string(),
        },
        Position { x, y },
        RenderableBundle::new("¡", palettes::basic::FUCHSIA.into(), palettes::basic::BLACK.into(), RenderOrder::ITEM, font),
    ));
}

fn spawn_magic_missile_scroll(commands: &mut Commands, font: &TextFont, x: i32, y: i32) {
    commands.spawn((
        Item,
        Consumable,
        Ranged { range: 6 },
        InflictsDamage { damage: 8 },
        Targeting::SingleEntity,
        Name {
            name: "Magic Missile Scroll".to_string(),
        },
        Position { x, y },
        RenderableBundle::new(")", palettes::basic::AQUA.into(), palettes::basic::BLACK.into(), RenderOrder::ITEM, font),
    ));
}

fn spawn_fireball_scroll(commands: &mut Commands, font: &TextFont, x: i32, y: i32) {
    commands.spawn((
        Item,
        Consumable,
        Ranged { range: 6 },
        InflictsDamage { damage: 20 },
        AreaOfEffect { radius: 3 },
        Name {
            name: "Fireball Scroll".to_string(),
        },
        Position { x, y },
        RenderableBundle::new(")", palettes::css::ORANGE.into(), palettes::basic::BLACK.into(), RenderOrder::ITEM, font),
    ));
}

fn spawn_confusion_scroll(commands: &mut Commands, font: &TextFont, x: i32, y: i32) {
    commands.spawn((
        Item,
        Consumable,
        Ranged { range: 6 },
        CausesConfusion { turns: 4 },
        Targeting::SingleEntity,
        Name {
            name: "Confusion Scroll".to_string(),
        },
        Position { x, y },
        RenderableBundle::new(")", palettes::css::PINK.into(), palettes::basic::BLACK.into(), RenderOrder::ITEM, font),
    ));
}
