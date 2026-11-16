use bevy::color::palettes;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[cfg(not(target_arch = "wasm32"))]
use std::fs;
#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;

use crate::abilities::HeroClass;
use crate::combat::{CombatStats, DEFAULT_AIM};
use crate::components::{
    AreaOfEffect, BlocksTile, CausesConfusion, Confusion, Consumable, EntryTrigger, Equipped, Hidden,
    HungerClock, HungerState, InBackpack, InflictsDamage, Item, MagicMapper, Name, ProvidesFood, TownPortal,
    ProvidesHealing, Ranged, RenderableBundle, SingleActivation, Targeting,
};
use crate::economy::{Gold, Purse};
use crate::levels::Delve;
use crate::entity_render::{EntityShape, Renderable};
use crate::game_flow::RunState;
use crate::gamelog::GameLog;
use crate::map::{tile_walkable, Map, Position, Tile, TileType, FONT_SIZE};
use crate::map_render::{spawn_map_tiles, TileReveal};
use crate::models::Model;
use crate::monsters::Monster;
use crate::experience::{mob_xp, Experience, XpValue};
use crate::party::{ActiveUnit, PartyMember};
use crate::raws::{spawn_class_kit, RAWS};
use crate::resources::UiFont;
use crate::tactics::DEFAULT_MOVEMENT;
use crate::viewshed::Viewshed;

#[cfg(all(not(target_arch = "wasm32"), not(test)))]
const SAVE_FILE: &str = "savegame.json";
// Tests drive the real death path; keep it away from a developer's save.
#[cfg(all(not(target_arch = "wasm32"), test))]
const SAVE_FILE: &str = "savegame.test-never-written.json";

// ============================================================================
// Save Queries
// ============================================================================
// Everything the save file needs to capture, written down once. If you add a
// component that should survive a save/load, extend the matching query here
// and the (de)serialization code below.

pub type PartySaveQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static PartyMember,
        &'static Position,
        &'static Name,
        &'static CombatStats,
        &'static Viewshed,
        &'static HungerClock,
        &'static Renderable,
        Option<&'static HeroClass>,
        Option<&'static Experience>,
    ),
>;

pub type MonsterSaveQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Position,
        &'static Name,
        &'static CombatStats,
        &'static Viewshed,
        &'static Renderable,
        Option<&'static Confusion>,
        Option<&'static Gold>,
    ),
    With<Monster>,
>;

pub type ItemSaveQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Name,
        &'static Renderable,
        Option<&'static Position>,
        Option<&'static InBackpack>,
        Option<&'static Consumable>,
        Option<&'static ProvidesHealing>,
        Option<&'static ProvidesFood>,
        Option<&'static Ranged>,
        Option<&'static InflictsDamage>,
        Option<&'static AreaOfEffect>,
        Option<&'static Targeting>,
        Option<&'static CausesConfusion>,
        // Nested: Bevy query tuples stop at 15 elements.
        (Option<&'static MagicMapper>, Option<&'static Gold>, Option<&'static TownPortal>),
    ),
    // Equipment is not saved yet (P01): a loaded hero gets their class's gear afresh.
    (With<Item>, Without<Equipped>),
>;

pub type TrapSaveQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Position,
        &'static Name,
        &'static Renderable,
        &'static InflictsDamage,
        Option<&'static Hidden>,
        Option<&'static SingleActivation>,
    ),
    With<EntryTrigger>,
>;

/// Q key: save the game (if a run is in progress and a hero is alive)
/// and exit. Registered as a system in main.rs.
pub fn save_on_quit(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut exit: MessageWriter<AppExit>,
    state: Res<State<RunState>>,
    map: Res<Map>,
    game_log: Res<GameLog>,
    purse: Res<Purse>,
    delve: Res<Delve>,
    active: Res<ActiveUnit>,
    party_query: PartySaveQuery,
    monster_query: MonsterSaveQuery,
    item_query: ItemSaveQuery,
    trap_query: TrapSaveQuery,
) {
    if keyboard.just_released(KeyCode::KeyQ) {
        // Only save if we're in-game (not in MainMenu) and a hero is alive
        if *state.get() != RunState::MainMenu {
            let party_alive = party_query.iter().any(|(_, _, _, _, stats, ..)| stats.hp > 0);

            if party_alive {
                save_game(map, game_log, purse, delve, active, party_query, monster_query, item_query, trap_query);
            }
        }
        exit.write(AppExit::Success);
    }
}

// ============================================================================
// Serializable Data Structures
// ============================================================================

#[derive(Serialize, Deserialize)]
pub struct SaveData {
    pub map: SerializedMap,
    /// The heroes still alive, in slot order.
    pub party: Vec<SerializedMember>,
    /// The slot of the active unit.
    pub active: usize,
    pub monsters: Vec<SerializedMonster>,
    pub items: Vec<SerializedItem>,
    #[serde(default)]
    pub traps: Vec<SerializedTrap>,
    pub game_log: Vec<String>,
    #[serde(default)]
    pub gold: i32,
    /// The deepest depth reached and the levels entered since leaving town.
    pub delve: Delve,
}

#[derive(Serialize, Deserialize)]
pub struct SerializedMap {
    pub tiles: Vec<TileType>,
    pub revealed_tiles: Vec<bool>,
    pub width: i32,
    pub height: i32,
    pub depth: i32,
    #[serde(default)]
    pub bloodstains: Vec<usize>,
}

#[derive(Serialize, Deserialize)]
pub struct SerializedMember {
    pub slot: usize,
    pub x: i32,
    pub y: i32,
    pub name: String,
    pub color: SerializedColor,
    pub max_hp: i32,
    pub hp: i32,
    pub defense: i32,
    pub power: i32,
    #[serde(default = "default_movement")]
    pub movement: i32,
    #[serde(default = "default_aim")]
    pub aim: i32,
    #[serde(default)]
    pub defence: i32,
    pub viewshed_range: i32,
    #[serde(default)]
    pub hunger_state: HungerState,
    #[serde(default = "default_hunger_duration")]
    pub hunger_duration: i32,
    /// The raws class; missing in saves from before classes.
    #[serde(default)]
    pub class: Option<String>,
    /// Missing in saves from before levels: level 1, no experience.
    #[serde(default = "first_level")]
    pub level: i32,
    #[serde(default)]
    pub xp: i32,
}

fn first_level() -> i32 {
    1
}

fn default_hunger_duration() -> i32 {
    20
}

fn default_movement() -> i32 {
    DEFAULT_MOVEMENT
}

fn default_aim() -> i32 {
    DEFAULT_AIM
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
    #[serde(default = "default_movement")]
    pub movement: i32,
    #[serde(default = "default_aim")]
    pub aim: i32,
    #[serde(default)]
    pub defence: i32,
    pub viewshed_range: i32,
    pub confusion_turns: Option<i32>,
    #[serde(default)]
    pub gold: Option<i32>,
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
    /// In the backpack of the hero in this slot.
    InBackpack { slot: usize },
}

#[derive(Serialize, Deserialize, Default)]
pub struct ItemProperties {
    pub consumable: bool,
    pub provides_healing: Option<i32>,
    pub provides_food: bool,
    pub ranged_range: Option<i32>,
    pub inflicts_damage: Option<i32>,
    pub area_of_effect: Option<i32>,
    pub targeting: Option<String>,
    pub causes_confusion: Option<i32>,
    #[serde(default)]
    pub magic_mapper: bool,
    #[serde(default)]
    pub town_portal: bool,
    #[serde(default)]
    pub gold: Option<i32>,
}

#[derive(Serialize, Deserialize)]
pub struct SerializedColor {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

#[derive(Serialize, Deserialize)]
pub struct SerializedTrap {
    pub x: i32,
    pub y: i32,
    pub name: String,
    pub glyph: String,
    pub color: SerializedColor,
    pub damage: i32,
    pub hidden: bool,
    pub single_activation: bool,
}

// ============================================================================
// Save System
// ============================================================================

#[cfg(not(target_arch = "wasm32"))]
pub fn save_game(
    map: Res<Map>,
    game_log: Res<GameLog>,
    purse: Res<Purse>,
    delve: Res<Delve>,
    active: Res<ActiveUnit>,
    party_query: PartySaveQuery,
    monster_query: MonsterSaveQuery,
    item_query: ItemSaveQuery,
    trap_query: TrapSaveQuery,
) {
    let mut party: Vec<SerializedMember> = party_query
        .iter()
        .map(|(_, member, pos, name, stats, viewshed, hunger, renderable, class, experience)| SerializedMember {
            slot: member.slot,
            x: pos.x,
            y: pos.y,
            name: name.name.clone(),
            color: serialize_color(renderable.fg),
            max_hp: stats.max_hp,
            hp: stats.hp,
            defense: stats.armour,
            power: stats.power,
            movement: stats.movement,
            aim: stats.aim,
            defence: stats.defence,
            viewshed_range: viewshed.range,
            hunger_state: hunger.state,
            hunger_duration: hunger.duration,
            class: class.map(|c| c.0.clone()),
            level: experience.map_or(1, |e| e.level),
            xp: experience.map_or(0, |e| e.xp),
        })
        .collect();
    if party.is_empty() {
        warn!("Cannot save: no party found");
        return;
    }
    party.sort_by_key(|member| member.slot);
    let slot_of = |entity: Entity| party_query.get(entity).ok().map(|(_, member, ..)| member.slot);
    let active = active.0.and_then(slot_of).unwrap_or(party[0].slot);

    // Serialize monsters
    let monsters: Vec<SerializedMonster> = monster_query
        .iter()
        .map(
            |(pos, name, stats, viewshed, renderable, confusion, gold)| SerializedMonster {
                x: pos.x,
                y: pos.y,
                name: name.name.clone(),
                glyph: renderable.glyph.clone(),
                max_hp: stats.max_hp,
                hp: stats.hp,
                defense: stats.armour,
                power: stats.power,
                movement: stats.movement,
                aim: stats.aim,
                defence: stats.defence,
                viewshed_range: viewshed.range,
                confusion_turns: confusion.map(|c| c.turns),
                gold: gold.map(|g| g.amount),
            },
        )
        .collect();

    // Serialize items
    let items: Vec<SerializedItem> = item_query
        .iter()
        .map(
            |(
                _entity,
                name,
                renderable,
                pos,
                in_backpack,
                consumable,
                healing,
                food,
                ranged,
                damage,
                aoe,
                targeting,
                causes_confusion,
                (magic_mapper, gold, town_portal),
            )| {
                let location = if let Some(backpack) = in_backpack {
                    if let Some(slot) = slot_of(backpack.owner) {
                        ItemLocation::InBackpack { slot }
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

                let srgba = renderable.fg.to_srgba();
                SerializedItem {
                    name: name.name.clone(),
                    glyph: renderable.glyph.clone(),
                    color: SerializedColor {
                        r: srgba.red,
                        g: srgba.green,
                        b: srgba.blue,
                    },
                    location,
                    properties: ItemProperties {
                        consumable: consumable.is_some(),
                        provides_healing: healing.map(|h| h.heal_amount),
                        provides_food: food.is_some(),
                        ranged_range: ranged.map(|r| r.range),
                        inflicts_damage: damage.map(|d| d.damage),
                        area_of_effect: aoe.map(|a| a.radius),
                        targeting: targeting.map(|t| match t {
                            Targeting::Tile => "Tile".to_string(),
                            Targeting::SingleEntity => "SingleEntity".to_string(),
                            Targeting::Ally => "Ally".to_string(),
                        }),
                        causes_confusion: causes_confusion.map(|c| c.turns),
                        magic_mapper: magic_mapper.is_some(),
                        town_portal: town_portal.is_some(),
                        gold: gold.map(|g| g.amount),
                    },
                }
            },
        )
        .collect();

    // Serialize traps
    let traps: Vec<SerializedTrap> = trap_query
        .iter()
        .map(|(pos, name, renderable, damage, hidden, single)| {
            let srgba = renderable.fg.to_srgba();
            SerializedTrap {
                x: pos.x,
                y: pos.y,
                name: name.name.clone(),
                glyph: renderable.glyph.clone(),
                color: SerializedColor {
                    r: srgba.red,
                    g: srgba.green,
                    b: srgba.blue,
                },
                damage: damage.damage,
                hidden: hidden.is_some(),
                single_activation: single.is_some(),
            }
        })
        .collect();

    // Serialize map
    let serialized_map = SerializedMap {
        tiles: map.tiles.clone(),
        revealed_tiles: map.revealed_tiles.clone(),
        width: map.width,
        height: map.height,
        depth: map.depth,
        bloodstains: map.bloodstains.iter().copied().collect(),
    };

    let save_data = SaveData {
        map: serialized_map,
        party,
        active,
        monsters,
        items,
        traps,
        game_log: game_log.entries.clone(),
        gold: purse.gold,
        delve: delve.clone(),
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

fn serialize_color(color: Color) -> SerializedColor {
    let srgba = color.to_srgba();
    SerializedColor {
        r: srgba.red,
        g: srgba.green,
        b: srgba.blue,
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
    entities_to_despawn: &Query<Entity, Or<(With<PartyMember>, With<Monster>, With<Item>, With<Tile>, With<EntryTrigger>)>>,
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

    restore_game(commands, entities_to_despawn, map, game_log, font, save_data);

    // Delete save file (permadeath)
    delete_save_file();

    info!("Game loaded successfully");
    true
}

/// Rebuild the world from parsed save data.
#[cfg(not(target_arch = "wasm32"))]
fn restore_game(
    commands: &mut Commands,
    entities_to_despawn: &Query<Entity, Or<(With<PartyMember>, With<Monster>, With<Item>, With<Tile>, With<EntryTrigger>)>>,
    map: &mut Map,
    game_log: &mut GameLog,
    font: &UiFont,
    save_data: SaveData,
) {
    // Despawn all existing game entities
    for entity in entities_to_despawn.iter() {
        commands.entity(entity).despawn();
    }

    // Restore map
    map.tiles = save_data.map.tiles;
    map.revealed_tiles = save_data.map.revealed_tiles;
    map.width = save_data.map.width;
    map.height = save_data.map.height;
    map.depth = save_data.map.depth;
    // Recalculate blocked tiles
    map.blocked_tiles = map.tiles.iter().map(|t| !tile_walkable(*t)).collect();
    map.visible_tiles = vec![false; map.tiles.len()];
    map.tile_content = vec![Vec::new(); map.tiles.len()];
    map.bloodstains = save_data.map.bloodstains.into_iter().collect();

    // Restore game log
    game_log.entries = save_data.game_log;
    commands.insert_resource(Purse {
        gold: save_data.gold,
    });
    commands.insert_resource(save_data.delve);

    let text_font = TextFont {
        font: font.0.clone().into(),
        font_size: FontSize::Px(FONT_SIZE),
        ..default()
    };

    // Spawn map tiles, keeping explored areas dimly visible
    spawn_map_tiles(commands, map, &text_font, TileReveal::FromSave);

    // Spawn the party
    let mut party = Vec::new();
    for member in save_data.party {
        let entity = commands
            .spawn((
                PartyMember { slot: member.slot },
                Experience { level: member.level, xp: member.xp },
                Name { name: member.name },
                Position {
                    x: member.x,
                    y: member.y,
                },
                CombatStats {
                    max_hp: member.max_hp,
                    hp: member.hp,
                    armour: member.defense,
                    power: member.power,
                    movement: member.movement,
                    aim: member.aim,
                    defence: member.defence,
                },
                Viewshed {
                    range: member.viewshed_range,
                    visible_tiles: Vec::new(),
                    dirty: true,
                },
                HungerClock {
                    state: member.hunger_state,
                    duration: member.hunger_duration,
                },
                RenderableBundle::new(
                    "☺",
                    Color::srgb(member.color.r, member.color.g, member.color.b),
                    EntityShape::Creature,
                ),
            ))
            .id();
        // Not saved yet (P01): equipment and cooldowns. The class's starting
        // gear comes back equipped and its abilities ready.
        let raws = RAWS.lock().unwrap();
        if let Some(class) = member.class.as_deref().and_then(|name| raws.class(name)) {
            commands.entity(entity).insert(HeroClass(class.name.clone()));
            spawn_class_kit(&raws, commands, entity, class);
        }
        party.push((member.slot, entity));
    }
    let member_in = |slot: usize| party.iter().find(|m| m.0 == slot).map(|m| m.1);
    commands.insert_resource(ActiveUnit(member_in(save_data.active).or(party.first().map(|m| m.1))));

    // Spawn monsters
    for monster in save_data.monsters {
        let mut entity_commands = commands.spawn((
            Monster,
            // Not saved yet (P01): a raws `xp` override.
            XpValue(mob_xp(monster.max_hp, monster.power)),
            BlocksTile,
            Name { name: monster.name },
            Position {
                x: monster.x,
                y: monster.y,
            },
            CombatStats {
                max_hp: monster.max_hp,
                hp: monster.hp,
                armour: monster.defense,
                power: monster.power,
                movement: monster.movement,
                aim: monster.aim,
                defence: monster.defence,
            },
            Viewshed {
                range: monster.viewshed_range,
                visible_tiles: Vec::new(),
                dirty: true,
            },
            RenderableBundle::new(
                &monster.glyph,
                palettes::basic::RED.into(),
                EntityShape::Creature,
            ),
        ));

        if let Some(turns) = monster.confusion_turns {
            entity_commands.insert(Confusion { turns });
        }
        if let Some(amount) = monster.gold {
            entity_commands.insert(Gold { amount });
        }
    }

    // Spawn items
    for item in save_data.items {
        let color = Color::srgb(item.color.r, item.color.g, item.color.b);
        let mut entity_commands = commands.spawn((Item, Name { name: item.name.clone() }));
        // The model is not saved: it comes from the raws, and goes in before the shape that reads it.
        if let Some(model) = RAWS.lock().unwrap().item_model(&item.name) {
            entity_commands.insert(Model(model.to_string()));
        }
        entity_commands.insert(RenderableBundle::new(&item.glyph, color, EntityShape::Item));

        // Add location
        match item.location {
            ItemLocation::OnGround { x, y } => {
                entity_commands.insert(Position { x, y });
            }
            ItemLocation::InBackpack { slot } => match member_in(slot) {
                Some(owner) => {
                    entity_commands.insert((InBackpack { owner }, Visibility::Hidden));
                }
                None => warn!("Item {} belongs to no hero in slot {slot}", item.name),
            },
        }

        // Add properties
        if item.properties.consumable {
            entity_commands.insert(Consumable);
        }
        if let Some(heal) = item.properties.provides_healing {
            entity_commands.insert(ProvidesHealing { heal_amount: heal });
        }
        if item.properties.provides_food {
            entity_commands.insert(ProvidesFood);
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
                "Ally" => Targeting::Ally,
                _ => Targeting::Tile,
            };
            entity_commands.insert(targeting);
        }
        if let Some(turns) = item.properties.causes_confusion {
            entity_commands.insert(CausesConfusion { turns });
        }
        if item.properties.magic_mapper {
            entity_commands.insert(MagicMapper);
        }
        if item.properties.town_portal {
            entity_commands.insert(TownPortal);
        }
        if let Some(amount) = item.properties.gold {
            entity_commands.insert(Gold { amount });
        }
    }

    // Spawn traps
    for trap in save_data.traps {
        let color = Color::srgb(trap.color.r, trap.color.g, trap.color.b);
        let mut entity_commands = commands.spawn((
            EntryTrigger,
            Name { name: trap.name },
            Position { x: trap.x, y: trap.y },
            InflictsDamage { damage: trap.damage },
            RenderableBundle::new(
                &trap.glyph,
                color,
                EntityShape::Prop,
            ),
        ));

        if trap.hidden {
            entity_commands.insert(Hidden);
        }
        if trap.single_activation {
            entity_commands.insert(SingleActivation);
        }
    }
}

// ============================================================================
// WASM Stubs (no filesystem access)
// ============================================================================

#[cfg(target_arch = "wasm32")]
pub fn save_game(
    _map: Res<Map>,
    _game_log: Res<GameLog>,
    _purse: Res<Purse>,
    _delve: Res<Delve>,
    _active: Res<ActiveUnit>,
    _party_query: PartySaveQuery,
    _monster_query: MonsterSaveQuery,
    _item_query: ItemSaveQuery,
    _trap_query: TrapSaveQuery,
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
    _entities_to_despawn: &Query<Entity, Or<(With<PartyMember>, With<Monster>, With<Item>, With<Tile>, With<EntryTrigger>)>>,
    _map: &mut Map,
    _game_log: &mut GameLog,
    _font: &UiFont,
) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn member(slot: usize, name: &str, x: i32) -> SerializedMember {
        SerializedMember {
            slot,
            x,
            y: 0,
            name: name.into(),
            color: SerializedColor { r: 0.0, g: 1.0, b: 0.0 },
            max_hp: 30,
            hp: 30 - slot as i32,
            defense: 2,
            power: 5,
            movement: DEFAULT_MOVEMENT,
            aim: DEFAULT_AIM,
            defence: 0,
            viewshed_range: 8,
            hunger_state: HungerState::Normal,
            hunger_duration: 20,
            class: None,
            level: 2,
            xp: 60,
        }
    }

    fn save_with_gold() -> SaveData {
        SaveData {
            map: SerializedMap {
                tiles: Vec::new(),
                revealed_tiles: Vec::new(),
                width: 0,
                height: 0,
                depth: 1,
                bloodstains: Vec::new(),
            },
            party: vec![member(0, "Aldric", 0), member(2, "Corin", 3)],
            active: 2,
            monsters: vec![SerializedMonster {
                x: 1,
                y: 1,
                name: "Goblin".into(),
                glyph: "g".into(),
                max_hp: 16,
                hp: 16,
                defense: 1,
                power: 4,
                movement: DEFAULT_MOVEMENT,
                aim: DEFAULT_AIM,
                defence: 0,
                viewshed_range: 8,
                confusion_turns: None,
                gold: Some(6),
            }],
            items: vec![
                SerializedItem {
                    name: "Gold".into(),
                    glyph: "$".into(),
                    color: SerializedColor { r: 1.0, g: 0.84, b: 0.0 },
                    location: ItemLocation::OnGround { x: 2, y: 2 },
                    properties: ItemProperties {
                        gold: Some(17),
                        ..default()
                    },
                },
                SerializedItem {
                    name: "Dagger".into(),
                    glyph: "/".into(),
                    color: SerializedColor { r: 0.0, g: 1.0, b: 1.0 },
                    location: ItemLocation::InBackpack { slot: 2 },
                    properties: ItemProperties::default(),
                },
            ],
            traps: Vec::new(),
            game_log: Vec::new(),
            gold: 42,
            delve: Delve { deepest: 3, levels_visited: 2 },
        }
    }

    #[test]
    fn gold_survives_a_save_round_trip() {
        let json = serde_json::to_string(&save_with_gold()).unwrap();
        let loaded: SaveData = serde_json::from_str(&json).unwrap();

        assert_eq!(loaded.gold, 42);
        assert_eq!(loaded.monsters[0].gold, Some(6));
        assert_eq!(loaded.items[0].properties.gold, Some(17));
    }

    #[test]
    fn the_whole_party_survives_a_save_round_trip() {
        let json = serde_json::to_string(&save_with_gold()).unwrap();
        let loaded: SaveData = serde_json::from_str(&json).unwrap();

        let party: Vec<(usize, &str, i32, i32)> =
            loaded.party.iter().map(|m| (m.slot, m.name.as_str(), m.x, m.hp)).collect();
        assert_eq!(party, vec![(0, "Aldric", 0, 30), (2, "Corin", 3, 28)]);
        assert_eq!(loaded.active, 2);
        assert!(matches!(loaded.items[1].location, ItemLocation::InBackpack { slot: 2 }));
        assert_eq!(loaded.delve, Delve { deepest: 3, levels_visited: 2 });
        assert_eq!((loaded.party[1].level, loaded.party[1].xp), (2, 60));
    }

    #[test]
    fn a_hero_saved_before_levels_loads_at_level_one() {
        let mut json = serde_json::to_value(save_with_gold()).unwrap();
        let hero = json["party"][0].as_object_mut().unwrap();
        hero.remove("level");
        hero.remove("xp");
        let loaded: SaveData = serde_json::from_value(json).unwrap();
        assert_eq!((loaded.party[0].level, loaded.party[0].xp), (1, 0));
    }

    #[test]
    fn a_loaded_game_puts_3d_bodies_on_their_tiles() {
        use crate::entity_render::{attach_entity_mesh, EntityAssets};
        use bevy::ecs::system::RunSystemOnce;

        crate::raws::load_raws();
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<EntityAssets>()
            .insert_resource(crate::models::AvailableModels(default()))
            .add_observer(attach_entity_mesh)
            .add_systems(Update, crate::map::translate_positions);
        app.world_mut()
            .run_system_once(
                |mut commands: Commands,
                 existing: Query<
                    Entity,
                    Or<(With<PartyMember>, With<Monster>, With<Item>, With<Tile>, With<EntryTrigger>)>,
                >| {
                    let font = UiFont(Handle::default());
                    let (mut map, mut log) = (Map::default(), GameLog::default());
                    restore_game(&mut commands, &existing, &mut map, &mut log, &font, save_with_gold());
                },
            )
            .unwrap();
        app.update();

        let world = app.world_mut();
        let mut bodies: Vec<(String, Vec3, bool)> = world
            .query_filtered::<(&Name, &Transform, Has<Mesh3d>), (With<EntityShape>, With<Position>)>()
            .iter(world)
            .map(|(name, transform, mesh)| (name.name.clone(), transform.translation, mesh))
            .collect();
        bodies.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(
            bodies,
            vec![
                ("Aldric".into(), Vec3::new(0.0, 0.7, 0.0), true),
                ("Corin".into(), Vec3::new(3.0, 0.7, 0.0), true),
                ("Goblin".into(), Vec3::new(1.0, 0.7, 1.0), true),
                ("Gold".into(), Vec3::new(2.0, 0.15, 2.0), true),
            ]
        );
        let mut models: Vec<(String, String)> = world
            .query_filtered::<(&Name, &Model), With<Item>>()
            .iter(world)
            .map(|(name, Model(model))| (name.name.clone(), model.clone()))
            .collect();
        models.sort();
        assert_eq!(models, vec![("Dagger".into(), "dagger".into()), ("Gold".into(), "gold".into())]);

        // The active unit and the dagger's owner are the restored Corin.
        let corin = world
            .query::<(Entity, &PartyMember)>()
            .iter(world)
            .find(|(_, member)| member.slot == 2)
            .map(|(entity, _)| entity);
        assert_eq!(world.resource::<ActiveUnit>().0, corin);
        assert_eq!(*world.resource::<Delve>(), Delve { deepest: 3, levels_visited: 2 });
        let owners: Vec<Entity> = world.query::<&InBackpack>().iter(world).map(|b| b.owner).collect();
        assert_eq!(owners, corin.into_iter().collect::<Vec<_>>());
    }

    #[test]
    fn a_loaded_hero_gets_their_class_gear_and_abilities_back() {
        use crate::abilities::HeroAbility;
        use crate::components::Equipped;
        use bevy::ecs::system::RunSystemOnce;

        crate::raws::load_raws();
        let mut save = save_with_gold();
        save.party[1].class = Some("Mage".into());
        let mut save = Some(save);
        let mut world = World::new();
        world
            .run_system_once(
                move |mut commands: Commands,
                      existing: Query<
                    Entity,
                    Or<(With<PartyMember>, With<Monster>, With<Item>, With<Tile>, With<EntryTrigger>)>,
                >| {
                    let font = UiFont(Handle::default());
                    let (mut map, mut log) = (Map::default(), GameLog::default());
                    restore_game(&mut commands, &existing, &mut map, &mut log, &font, save.take().unwrap());
                },
            )
            .unwrap();
        let classes: Vec<(String, Option<String>)> = world
            .query::<(&Name, Option<&HeroClass>)>()
            .iter(&world)
            .filter(|(n, _)| ["Aldric", "Corin"].contains(&n.name.as_str()))
            .map(|(n, c)| (n.name.clone(), c.map(|c| c.0.clone())))
            .collect();
        assert!(classes.contains(&("Corin".into(), Some("Mage".into()))), "{classes:?}");
        assert!(classes.contains(&("Aldric".into(), None)), "{classes:?}");
        let mut equipped: Vec<String> =
            world.query_filtered::<&Name, With<Equipped>>().iter(&world).map(|n| n.name.clone()).collect();
        equipped.sort();
        assert_eq!(equipped, ["Dagger"]);
        let mut abilities: Vec<(usize, u8)> =
            world.query::<&HeroAbility>().iter(&world).map(|a| (a.slot, a.left)).collect();
        abilities.sort();
        assert_eq!(abilities, [(0, 0), (1, 0)]);
    }

    #[test]
    fn a_save_from_before_gold_loads_with_an_empty_purse() {
        let mut json = serde_json::to_value(save_with_gold()).unwrap();
        json.as_object_mut().unwrap().remove("gold");
        let loaded: SaveData = serde_json::from_value(json).unwrap();

        assert_eq!(loaded.gold, 0);
    }
}
