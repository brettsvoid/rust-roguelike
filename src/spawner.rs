use bevy::{color::palettes, prelude::*};
use rand::Rng;

use crate::{
    combat::CombatStats,
    components::{
        AreaOfEffect, BlocksTile, CausesConfusion, Consumable, DefenseBonus, EquipmentSlot,
        Equippable, HungerClock, HungerState, InflictsDamage, Item, MagicMapper, MeleePowerBonus,
        Name, ProvidesFood, ProvidesHealing, Ranged, RenderOrder, RenderableBundle, Targeting,
    },
    map::Position,
    monsters::Monster,
    player::Player,
    rng::{GameRng, RandomTable},
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

    // Build weighted spawn tables based on depth
    let monster_table = RandomTable::new()
        .add("Goblin", 10)
        .add("Orc", 1 + map_depth);

    let item_table = RandomTable::new()
        .add("Health Potion", 7)
        .add("Rations", 10)
        .add("Magic Missile Scroll", 2)
        .add("Fireball Scroll", map_depth - 1)
        .add("Confusion Scroll", map_depth - 1)
        .add("Magic Mapping Scroll", 2)
        .add("Dagger", 3)
        .add("Shield", 3)
        .add("Longsword", map_depth - 1)
        .add("Tower Shield", map_depth - 1);

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

    // Spawn monsters using weighted table
    for (x, y) in spawn_points.iter() {
        if let Some(monster_name) = monster_table.roll(rng) {
            match monster_name.as_str() {
                "Orc" => spawn_orc(commands, font, *x, *y, *monster_id),
                _ => spawn_goblin(commands, font, *x, *y, *monster_id),
            }
            *monster_id += 1;
        }
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

    // Spawn items using weighted table
    for (x, y) in item_spawn_points.iter() {
        if let Some(item_name) = item_table.roll(rng) {
            match item_name.as_str() {
                "Health Potion" => spawn_health_potion(commands, font, *x, *y),
                "Rations" => spawn_rations(commands, font, *x, *y),
                "Magic Missile Scroll" => spawn_magic_missile_scroll(commands, font, *x, *y),
                "Fireball Scroll" => spawn_fireball_scroll(commands, font, *x, *y),
                "Confusion Scroll" => spawn_confusion_scroll(commands, font, *x, *y),
                "Magic Mapping Scroll" => spawn_magic_mapping_scroll(commands, font, *x, *y),
                "Dagger" => spawn_dagger(commands, font, *x, *y),
                "Shield" => spawn_shield(commands, font, *x, *y),
                "Longsword" => spawn_longsword(commands, font, *x, *y),
                "Tower Shield" => spawn_tower_shield(commands, font, *x, *y),
                _ => {}
            }
        }
    }
}

fn spawn_orc(commands: &mut Commands, font: &TextFont, x: i32, y: i32, id: usize) {
    spawn_monster(commands, font, x, y, "o", &format!("Orc #{}", id));
}

fn spawn_goblin(commands: &mut Commands, font: &TextFont, x: i32, y: i32, id: usize) {
    spawn_monster(commands, font, x, y, "g", &format!("Goblin #{}", id));
}

fn spawn_monster(
    commands: &mut Commands,
    font: &TextFont,
    x: i32,
    y: i32,
    glyph: &str,
    name: &str,
) {
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
        RenderableBundle::new(
            glyph,
            palettes::basic::RED.into(),
            palettes::basic::BLACK.into(),
            RenderOrder::MONSTER,
            font,
        ),
    ));
}

pub fn spawn_health_potion(commands: &mut Commands, font: &TextFont, x: i32, y: i32) {
    commands.spawn((
        Item,
        Consumable,
        ProvidesHealing { heal_amount: 8 },
        Name {
            name: "Health Potion".to_string(),
        },
        Position { x, y },
        RenderableBundle::new(
            "¡",
            palettes::basic::FUCHSIA.into(),
            palettes::basic::BLACK.into(),
            RenderOrder::ITEM,
            font,
        ),
    ));
}

pub fn spawn_magic_missile_scroll(commands: &mut Commands, font: &TextFont, x: i32, y: i32) {
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
        RenderableBundle::new(
            ")",
            palettes::basic::AQUA.into(),
            palettes::basic::BLACK.into(),
            RenderOrder::ITEM,
            font,
        ),
    ));
}

pub fn spawn_fireball_scroll(commands: &mut Commands, font: &TextFont, x: i32, y: i32) {
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
        RenderableBundle::new(
            ")",
            palettes::css::ORANGE.into(),
            palettes::basic::BLACK.into(),
            RenderOrder::ITEM,
            font,
        ),
    ));
}

pub fn spawn_confusion_scroll(commands: &mut Commands, font: &TextFont, x: i32, y: i32) {
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
        RenderableBundle::new(
            ")",
            palettes::css::PINK.into(),
            palettes::basic::BLACK.into(),
            RenderOrder::ITEM,
            font,
        ),
    ));
}

// Equipment spawners
pub fn spawn_dagger(commands: &mut Commands, font: &TextFont, x: i32, y: i32) {
    commands.spawn((
        Item,
        Equippable {
            slot: EquipmentSlot::Melee,
        },
        MeleePowerBonus { power: 2 },
        Name {
            name: "Dagger".to_string(),
        },
        Position { x, y },
        RenderableBundle::new(
            "/",
            palettes::basic::AQUA.into(),
            palettes::basic::BLACK.into(),
            RenderOrder::ITEM,
            font,
        ),
    ));
}

pub fn spawn_shield(commands: &mut Commands, font: &TextFont, x: i32, y: i32) {
    commands.spawn((
        Item,
        Equippable {
            slot: EquipmentSlot::Shield,
        },
        DefenseBonus { defense: 1 },
        Name {
            name: "Shield".to_string(),
        },
        Position { x, y },
        RenderableBundle::new(
            "(",
            palettes::basic::AQUA.into(),
            palettes::basic::BLACK.into(),
            RenderOrder::ITEM,
            font,
        ),
    ));
}

pub fn spawn_longsword(commands: &mut Commands, font: &TextFont, x: i32, y: i32) {
    commands.spawn((
        Item,
        Equippable {
            slot: EquipmentSlot::Melee,
        },
        MeleePowerBonus { power: 4 },
        Name {
            name: "Longsword".to_string(),
        },
        Position { x, y },
        RenderableBundle::new(
            "/",
            palettes::basic::YELLOW.into(),
            palettes::basic::BLACK.into(),
            RenderOrder::ITEM,
            font,
        ),
    ));
}

pub fn spawn_tower_shield(commands: &mut Commands, font: &TextFont, x: i32, y: i32) {
    commands.spawn((
        Item,
        Equippable {
            slot: EquipmentSlot::Shield,
        },
        DefenseBonus { defense: 3 },
        Name {
            name: "Tower Shield".to_string(),
        },
        Position { x, y },
        RenderableBundle::new(
            "(",
            palettes::basic::YELLOW.into(),
            palettes::basic::BLACK.into(),
            RenderOrder::ITEM,
            font,
        ),
    ));
}

pub fn spawn_rations(commands: &mut Commands, font: &TextFont, x: i32, y: i32) {
    commands.spawn((
        Item,
        Consumable,
        ProvidesFood,
        Name {
            name: "Rations".to_string(),
        },
        Position { x, y },
        RenderableBundle::new(
            "%",
            palettes::basic::GREEN.into(),
            palettes::basic::BLACK.into(),
            RenderOrder::ITEM,
            font,
        ),
    ));
}

pub fn spawn_magic_mapping_scroll(commands: &mut Commands, font: &TextFont, x: i32, y: i32) {
    commands.spawn((
        Item,
        Consumable,
        MagicMapper,
        Name {
            name: "Scroll of Magic Mapping".to_string(),
        },
        Position { x, y },
        RenderableBundle::new(
            ")",
            palettes::css::CORNFLOWER_BLUE.into(),
            palettes::basic::BLACK.into(),
            RenderOrder::ITEM,
            font,
        ),
    ));
}
