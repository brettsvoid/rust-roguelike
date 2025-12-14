use bevy::color::palettes;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[cfg(not(target_arch = "wasm32"))]
use std::fs;
#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;

use crate::combat::CombatStats;
use crate::components::{
    AreaOfEffect, BlocksTile, CausesConfusion, Confusion, Consumable, InBackpack, InflictsDamage,
    Item, Name, ProvidesHealing, Ranged, RenderOrder, RenderableBundle, Targeting,
};
use crate::gamelog::GameLog;
use crate::map::{Map, Position, Revealed, RevealedState, Tile, TileType, MAP_WIDTH};
use crate::monsters::Monster;
use crate::player::Player;
use crate::resources::UiFont;
use crate::shapes::Rect;
use crate::viewshed::Viewshed;

#[cfg(not(target_arch = "wasm32"))]
const SAVE_FILE: &str = "savegame.json";

// ============================================================================
// Serializable Data Structures
// ============================================================================

#[derive(Serialize, Deserialize)]
pub struct SaveData {
    pub map: SerializedMap,
    pub player: SerializedPlayer,
    pub monsters: Vec<SerializedMonster>,
    pub items: Vec<SerializedItem>,
    pub game_log: Vec<String>,
}

#[derive(Serialize, Deserialize)]
pub struct SerializedMap {
    pub tiles: Vec<TileType>,
    pub rooms: Vec<Rect>,
    pub revealed_tiles: Vec<bool>,
    pub width: i32,
    pub height: i32,
    pub depth: i32,
}

#[derive(Serialize, Deserialize)]
pub struct SerializedPlayer {
    pub x: i32,
    pub y: i32,
    pub name: String,
    pub max_hp: i32,
    pub hp: i32,
    pub defense: i32,
    pub power: i32,
    pub viewshed_range: i32,
}

#[derive(Serialize, Deserialize)]
pub struct SerializedMonster {
    pub x: i32,
    pub y: i32,
    pub name: String,
    pub glyph: String,
    pub max_hp: i32,
    pub hp: i32,
    pub defense: i32,
    pub power: i32,
    pub viewshed_range: i32,
    pub confusion_turns: Option<i32>,
}

#[derive(Serialize, Deserialize)]
pub struct SerializedItem {
    pub name: String,
    pub glyph: String,
    pub color: SerializedColor,
    pub location: ItemLocation,
    pub properties: ItemProperties,
}

#[derive(Serialize, Deserialize)]
pub enum ItemLocation {
    OnGround { x: i32, y: i32 },
    InPlayerBackpack,
}

#[derive(Serialize, Deserialize, Default)]
pub struct ItemProperties {
    pub consumable: bool,
    pub provides_healing: Option<i32>,
    pub ranged_range: Option<i32>,
    pub inflicts_damage: Option<i32>,
    pub area_of_effect: Option<i32>,
    pub targeting: Option<String>,
    pub causes_confusion: Option<i32>,
}

#[derive(Serialize, Deserialize)]
pub struct SerializedColor {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

// ============================================================================
// Save System
// ============================================================================

#[cfg(not(target_arch = "wasm32"))]
pub fn save_game(
    map: Res<Map>,
    game_log: Res<GameLog>,
    player_query: Query<(Entity, &Position, &Name, &CombatStats, &Viewshed), With<Player>>,
    monster_query: Query<
        (&Position, &Name, &CombatStats, &Viewshed, &Text2d, Option<&Confusion>),
        With<Monster>,
    >,
    item_query: Query<
        (
            Entity,
            &Name,
            &Text2d,
            &TextColor,
            Option<&Position>,
            Option<&InBackpack>,
            Option<&Consumable>,
            Option<&ProvidesHealing>,
            Option<&Ranged>,
            Option<&InflictsDamage>,
            Option<&AreaOfEffect>,
            Option<&Targeting>,
            Option<&CausesConfusion>,
        ),
        With<Item>,
    >,
) {
    let Ok((player_entity, player_pos, player_name, player_stats, player_viewshed)) =
        player_query.get_single()
    else {
        warn!("Cannot save: no player found");
        return;
    };

    // Serialize player
    let player = SerializedPlayer {
        x: player_pos.x,
        y: player_pos.y,
        name: player_name.name.clone(),
        max_hp: player_stats.max_hp,
        hp: player_stats.hp,
        defense: player_stats.defense,
        power: player_stats.power,
        viewshed_range: player_viewshed.range,
    };

    // Serialize monsters
    let monsters: Vec<SerializedMonster> = monster_query
        .iter()
        .map(|(pos, name, stats, viewshed, text, confusion)| SerializedMonster {
            x: pos.x,
            y: pos.y,
            name: name.name.clone(),
            glyph: text.0.clone(),
            max_hp: stats.max_hp,
            hp: stats.hp,
            defense: stats.defense,
            power: stats.power,
            viewshed_range: viewshed.range,
            confusion_turns: confusion.map(|c| c.turns),
        })
        .collect();

    // Serialize items
    let items: Vec<SerializedItem> = item_query
        .iter()
        .map(
            |(
                _entity,
                name,
                text,
                color,
                pos,
                in_backpack,
                consumable,
                healing,
                ranged,
                damage,
                aoe,
                targeting,
                causes_confusion,
            )| {
                let location = if let Some(backpack) = in_backpack {
                    if backpack.owner == player_entity {
                        ItemLocation::InPlayerBackpack
                    } else {
                        // Shouldn't happen in this game, but handle it
                        ItemLocation::OnGround {
                            x: pos.map(|p| p.x).unwrap_or(0),
                            y: pos.map(|p| p.y).unwrap_or(0),
                        }
                    }
                } else if let Some(p) = pos {
                    ItemLocation::OnGround { x: p.x, y: p.y }
                } else {
                    warn!("Item {} has no position or backpack", name.name);
                    ItemLocation::OnGround { x: 0, y: 0 }
                };

                let srgba = color.0.to_srgba();
                SerializedItem {
                    name: name.name.clone(),
                    glyph: text.0.clone(),
                    color: SerializedColor {
                        r: srgba.red,
                        g: srgba.green,
                        b: srgba.blue,
                    },
                    location,
                    properties: ItemProperties {
                        consumable: consumable.is_some(),
                        provides_healing: healing.map(|h| h.heal_amount),
                        ranged_range: ranged.map(|r| r.range),
                        inflicts_damage: damage.map(|d| d.damage),
                        area_of_effect: aoe.map(|a| a.radius),
                        targeting: targeting.map(|t| match t {
                            Targeting::Tile => "Tile".to_string(),
                            Targeting::SingleEntity => "SingleEntity".to_string(),
                        }),
                        causes_confusion: causes_confusion.map(|c| c.turns),
                    },
                }
            },
        )
        .collect();

    // Serialize map
    let serialized_map = SerializedMap {
        tiles: map.tiles.clone(),
        rooms: map.rooms.clone(),
        revealed_tiles: map.revealed_tiles.clone(),
        width: map.width,
        height: map.height,
        depth: map.depth,
    };

    let save_data = SaveData {
        map: serialized_map,
        player,
        monsters,
        items,
        game_log: game_log.entries.clone(),
    };

    match serde_json::to_string_pretty(&save_data) {
        Ok(json) => {
            if let Err(e) = fs::write(SAVE_FILE, json) {
                error!("Failed to write save file: {}", e);
            } else {
                info!("Game saved to {}", SAVE_FILE);
            }
        }
        Err(e) => {
            error!("Failed to serialize save data: {}", e);
        }
    }
}

// ============================================================================
// Load System
// ============================================================================

#[cfg(not(target_arch = "wasm32"))]
pub fn has_save_file() -> bool {
    Path::new(SAVE_FILE).exists()
}

#[cfg(not(target_arch = "wasm32"))]
pub fn delete_save_file() {
    let _ = fs::remove_file(SAVE_FILE);
}

#[cfg(not(target_arch = "wasm32"))]
pub fn load_game(
    commands: &mut Commands,
    entities_to_despawn: &Query<Entity, Or<(With<Player>, With<Monster>, With<Item>, With<Tile>)>>,
    map: &mut Map,
    game_log: &mut GameLog,
    font: &UiFont,
) -> bool {
    let Ok(json) = fs::read_to_string(SAVE_FILE) else {
        warn!("No save file found");
        return false;
    };

    let Ok(save_data) = serde_json::from_str::<SaveData>(&json) else {
        error!("Failed to parse save file");
        return false;
    };

    // Despawn all existing game entities
    for entity in entities_to_despawn.iter() {
        commands.entity(entity).despawn_recursive();
    }

    // Restore map
    map.tiles = save_data.map.tiles;
    map.rooms = save_data.map.rooms;
    map.revealed_tiles = save_data.map.revealed_tiles;
    map.width = save_data.map.width;
    map.height = save_data.map.height;
    map.depth = save_data.map.depth;
    // Recalculate blocked tiles
    map.blocked_tiles = map
        .tiles
        .iter()
        .map(|t| *t == TileType::Wall)
        .collect();
    map.visible_tiles = vec![false; map.tiles.len()];
    map.tile_content = vec![Vec::new(); map.tiles.len()];

    // Restore game log
    game_log.entries = save_data.game_log;

    let text_font = TextFont {
        font: font.0.clone(),
        font_size: 16.0,
        ..default()
    };

    // Spawn map tiles
    let mut y = 0;
    let mut x = 0;
    for (idx, tile) in map.tiles.iter().enumerate() {
        let revealed_state = if map.revealed_tiles[idx] {
            RevealedState::Explored
        } else {
            RevealedState::Hidden
        };

        match tile {
            TileType::Floor => {
                commands.spawn((
                    Tile,
                    Position { x, y },
                    Text2d::new("."),
                    text_font.clone(),
                    TextColor(Color::srgb(0.5, 0.5, 0.5)),
                    Revealed(revealed_state),
                ));
            }
            TileType::Wall => {
                commands.spawn((
                    Tile,
                    Position { x, y },
                    Text2d::new("#"),
                    text_font.clone(),
                    TextColor(Color::srgb(0.0, 1.0, 0.0)),
                    Revealed(revealed_state),
                ));
            }
            TileType::DownStairs => {
                commands.spawn((
                    Tile,
                    Position { x, y },
                    Text2d::new(">"),
                    text_font.clone(),
                    TextColor(Color::srgb(0.0, 1.0, 1.0)),
                    Revealed(revealed_state),
                ));
            }
        }

        x += 1;
        if x > MAP_WIDTH as i32 - 1 {
            x = 0;
            y += 1;
        }
    }

    // Spawn player
    let player_entity = commands
        .spawn((
            Player,
            Name {
                name: save_data.player.name,
            },
            Position {
                x: save_data.player.x,
                y: save_data.player.y,
            },
            CombatStats {
                max_hp: save_data.player.max_hp,
                hp: save_data.player.hp,
                defense: save_data.player.defense,
                power: save_data.player.power,
            },
            Viewshed {
                range: save_data.player.viewshed_range,
                visible_tiles: Vec::new(),
                dirty: true,
            },
            RenderableBundle::new(
                "☺",
                palettes::basic::YELLOW.into(),
                palettes::basic::BLACK.into(),
                RenderOrder::PLAYER,
                &text_font,
            ),
        ))
        .id();

    // Spawn monsters
    for monster in save_data.monsters {
        let mut entity_commands = commands.spawn((
            Monster,
            BlocksTile,
            Name { name: monster.name },
            Position {
                x: monster.x,
                y: monster.y,
            },
            CombatStats {
                max_hp: monster.max_hp,
                hp: monster.hp,
                defense: monster.defense,
                power: monster.power,
            },
            Viewshed {
                range: monster.viewshed_range,
                visible_tiles: Vec::new(),
                dirty: true,
            },
            RenderableBundle::new(
                &monster.glyph,
                palettes::basic::RED.into(),
                palettes::basic::BLACK.into(),
                RenderOrder::MONSTER,
                &text_font,
            ),
        ));

        if let Some(turns) = monster.confusion_turns {
            entity_commands.insert(Confusion { turns });
        }
    }

    // Spawn items
    for item in save_data.items {
        let color = Color::srgb(item.color.r, item.color.g, item.color.b);
        let mut entity_commands = commands.spawn((
            Item,
            Name { name: item.name },
            RenderableBundle::new(
                &item.glyph,
                color,
                palettes::basic::BLACK.into(),
                RenderOrder::ITEM,
                &text_font,
            ),
        ));

        // Add location
        match item.location {
            ItemLocation::OnGround { x, y } => {
                entity_commands.insert(Position { x, y });
            }
            ItemLocation::InPlayerBackpack => {
                entity_commands.insert(InBackpack {
                    owner: player_entity,
                });
                entity_commands.insert(Visibility::Hidden);
            }
        }

        // Add properties
        if item.properties.consumable {
            entity_commands.insert(Consumable);
        }
        if let Some(heal) = item.properties.provides_healing {
            entity_commands.insert(ProvidesHealing { heal_amount: heal });
        }
        if let Some(range) = item.properties.ranged_range {
            entity_commands.insert(Ranged { range });
        }
        if let Some(damage) = item.properties.inflicts_damage {
            entity_commands.insert(InflictsDamage { damage });
        }
        if let Some(radius) = item.properties.area_of_effect {
            entity_commands.insert(AreaOfEffect { radius });
        }
        if let Some(ref targeting_str) = item.properties.targeting {
            let targeting = match targeting_str.as_str() {
                "SingleEntity" => Targeting::SingleEntity,
                _ => Targeting::Tile,
            };
            entity_commands.insert(targeting);
        }
        if let Some(turns) = item.properties.causes_confusion {
            entity_commands.insert(CausesConfusion { turns });
        }
    }

    // Delete save file (permadeath)
    delete_save_file();

    info!("Game loaded successfully");
    true
}

// ============================================================================
// WASM Stubs (no filesystem access)
// ============================================================================

#[cfg(target_arch = "wasm32")]
pub fn save_game(
    _map: Res<Map>,
    _game_log: Res<GameLog>,
    _player_query: Query<(Entity, &Position, &Name, &CombatStats, &Viewshed), With<Player>>,
    _monster_query: Query<
        (&Position, &Name, &CombatStats, &Viewshed, &Text2d, Option<&Confusion>),
        With<Monster>,
    >,
    _item_query: Query<
        (
            Entity,
            &Name,
            &Text2d,
            &TextColor,
            Option<&Position>,
            Option<&InBackpack>,
            Option<&Consumable>,
            Option<&ProvidesHealing>,
            Option<&Ranged>,
            Option<&InflictsDamage>,
            Option<&AreaOfEffect>,
            Option<&Targeting>,
            Option<&CausesConfusion>,
        ),
        With<Item>,
    >,
) {
    // No-op on WASM
}

#[cfg(target_arch = "wasm32")]
pub fn has_save_file() -> bool {
    false
}

#[cfg(target_arch = "wasm32")]
pub fn delete_save_file() {
    // No-op on WASM
}

#[cfg(target_arch = "wasm32")]
pub fn load_game(
    _commands: &mut Commands,
    _entities_to_despawn: &Query<Entity, Or<(With<Player>, With<Monster>, With<Item>, With<Tile>)>>,
    _map: &mut Map,
    _game_log: &mut GameLog,
    _font: &UiFont,
) -> bool {
    false
}
