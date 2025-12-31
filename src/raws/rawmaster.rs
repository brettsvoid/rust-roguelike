use std::collections::HashMap;

use bevy::prelude::*;

use crate::combat::CombatStats;
use crate::components::{
    AreaOfEffect, BlocksTile, BlocksVisibility, CausesConfusion, Consumable, DefenseBonus,
    Door, EntryTrigger, EquipmentSlot, Equippable, Hidden, InflictsDamage, Item, MagicMapper,
    MeleePowerBonus, Name, ProvidesFood, ProvidesHealing, Ranged, RenderOrder, RenderableBundle,
    SingleActivation, Targeting,
};
use crate::map::Position;
use crate::monsters::Monster;
use crate::viewshed::Viewshed;

use super::Raws;

pub struct RawMaster {
    pub raws: Raws,
    pub item_index: HashMap<String, usize>,
    pub mob_index: HashMap<String, usize>,
    pub prop_index: HashMap<String, usize>,
}

impl RawMaster {
    pub fn empty() -> Self {
        RawMaster {
            raws: Raws {
                items: Vec::new(),
                mobs: Vec::new(),
                props: Vec::new(),
            },
            item_index: HashMap::new(),
            mob_index: HashMap::new(),
            prop_index: HashMap::new(),
        }
    }

    pub fn load(&mut self, raws: Raws) {
        self.raws = raws;
        self.item_index.clear();
        self.mob_index.clear();
        self.prop_index.clear();

        for (i, item) in self.raws.items.iter().enumerate() {
            self.item_index.insert(item.name.clone(), i);
        }
        for (i, mob) in self.raws.mobs.iter().enumerate() {
            self.mob_index.insert(mob.name.clone(), i);
        }
        for (i, prop) in self.raws.props.iter().enumerate() {
            self.prop_index.insert(prop.name.clone(), i);
        }
    }
}

/// Parse a hex color string like "#FF00FF" into a Bevy Color
fn parse_color(hex: &str) -> Color {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return Color::WHITE;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(255) as f32 / 255.0;
    let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(255) as f32 / 255.0;
    let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(255) as f32 / 255.0;
    Color::srgb(r, g, b)
}

/// Convert raw renderable to RenderOrder
fn get_render_order(order: i32) -> RenderOrder {
    match order {
        0 => RenderOrder::ITEM,
        1 => RenderOrder::MONSTER,
        2 => RenderOrder::PLAYER,
        _ => RenderOrder::ITEM,
    }
}

/// Spawn a named entity (item, mob, or prop)
pub fn spawn_named_entity(
    raws: &RawMaster,
    commands: &mut Commands,
    font: &TextFont,
    key: &str,
    x: i32,
    y: i32,
    monster_id: Option<usize>,
) -> bool {
    if raws.item_index.contains_key(key) {
        return spawn_named_item(raws, commands, font, key, x, y);
    } else if raws.mob_index.contains_key(key) {
        return spawn_named_mob(raws, commands, font, key, x, y, monster_id);
    } else if raws.prop_index.contains_key(key) {
        return spawn_named_prop(raws, commands, font, key, x, y);
    }
    false
}

/// Spawn an item from raw data
pub fn spawn_named_item(
    raws: &RawMaster,
    commands: &mut Commands,
    font: &TextFont,
    key: &str,
    x: i32,
    y: i32,
) -> bool {
    let Some(&idx) = raws.item_index.get(key) else {
        return false;
    };
    let item_template = &raws.raws.items[idx];

    let mut entity = commands.spawn((
        Item,
        Name {
            name: item_template.name.clone(),
        },
        Position { x, y },
    ));

    // Add renderable
    if let Some(ref renderable) = item_template.renderable {
        entity.insert(RenderableBundle::new(
            &renderable.glyph,
            parse_color(&renderable.fg),
            parse_color(&renderable.bg),
            get_render_order(renderable.order),
            font,
        ));
    }

    // Handle consumable effects
    if let Some(ref consumable) = item_template.consumable {
        entity.insert(Consumable);

        for (effect, value) in consumable.effects.iter() {
            match effect.as_str() {
                "provides_healing" => {
                    let amount = value.parse::<i32>().unwrap_or(0);
                    entity.insert(ProvidesHealing { heal_amount: amount });
                }
                "provides_food" => {
                    entity.insert(ProvidesFood);
                }
                "ranged" => {
                    let range = value.parse::<i32>().unwrap_or(0);
                    entity.insert(Ranged { range });
                }
                "damage" => {
                    let damage = value.parse::<i32>().unwrap_or(0);
                    entity.insert(InflictsDamage { damage });
                }
                "area_of_effect" => {
                    let radius = value.parse::<i32>().unwrap_or(0);
                    entity.insert(AreaOfEffect { radius });
                }
                "confusion" => {
                    let turns = value.parse::<i32>().unwrap_or(0);
                    entity.insert(CausesConfusion { turns });
                }
                "targeting" => {
                    if value == "single_entity" {
                        entity.insert(Targeting::SingleEntity);
                    }
                }
                "magic_mapping" => {
                    entity.insert(MagicMapper);
                }
                _ => {}
            }
        }
    }

    // Handle weapon
    if let Some(ref weapon) = item_template.weapon {
        let slot = match weapon.slot.as_str() {
            "melee" => EquipmentSlot::Melee,
            "shield" => EquipmentSlot::Shield,
            _ => EquipmentSlot::Melee,
        };
        entity.insert(Equippable { slot });
        entity.insert(MeleePowerBonus {
            power: weapon.power_bonus,
        });
    }

    // Handle shield
    if let Some(ref shield) = item_template.shield {
        entity.insert(Equippable {
            slot: EquipmentSlot::Shield,
        });
        entity.insert(DefenseBonus {
            defense: shield.defense_bonus,
        });
    }

    true
}

/// Spawn a mob from raw data
pub fn spawn_named_mob(
    raws: &RawMaster,
    commands: &mut Commands,
    font: &TextFont,
    key: &str,
    x: i32,
    y: i32,
    monster_id: Option<usize>,
) -> bool {
    let Some(&idx) = raws.mob_index.get(key) else {
        return false;
    };
    let mob_template = &raws.raws.mobs[idx];

    // Build name with optional ID
    let name = if let Some(id) = monster_id {
        format!("{} #{}", mob_template.name, id)
    } else {
        mob_template.name.clone()
    };

    let mut entity = commands.spawn((
        Monster,
        Name { name },
        Position { x, y },
        CombatStats {
            max_hp: mob_template.stats.max_hp,
            hp: mob_template.stats.hp,
            defense: mob_template.stats.defense,
            power: mob_template.stats.power,
        },
        Viewshed {
            range: mob_template.vision_range,
            ..default()
        },
    ));

    if mob_template.blocks_tile {
        entity.insert(BlocksTile);
    }

    // Add renderable
    if let Some(ref renderable) = mob_template.renderable {
        entity.insert(RenderableBundle::new(
            &renderable.glyph,
            parse_color(&renderable.fg),
            parse_color(&renderable.bg),
            get_render_order(renderable.order),
            font,
        ));
    }

    true
}

/// Spawn a prop from raw data
pub fn spawn_named_prop(
    raws: &RawMaster,
    commands: &mut Commands,
    font: &TextFont,
    key: &str,
    x: i32,
    y: i32,
) -> bool {
    let Some(&idx) = raws.prop_index.get(key) else {
        return false;
    };
    let prop_template = &raws.raws.props[idx];

    let mut entity = commands.spawn((
        Name {
            name: prop_template.name.clone(),
        },
        Position { x, y },
    ));

    // Add renderable
    if let Some(ref renderable) = prop_template.renderable {
        entity.insert(RenderableBundle::new(
            &renderable.glyph,
            parse_color(&renderable.fg),
            parse_color(&renderable.bg),
            get_render_order(renderable.order),
            font,
        ));
    }

    // Door handling
    if prop_template.door_open.is_some() {
        entity.insert(Door {
            open: prop_template.door_open.unwrap_or(false),
        });
    }

    // Blocking
    if prop_template.blocks_tile.unwrap_or(false) {
        entity.insert(BlocksTile);
    }

    if prop_template.blocks_visibility.unwrap_or(false) {
        entity.insert(BlocksVisibility);
    }

    // Hidden
    if prop_template.hidden.unwrap_or(false) {
        entity.insert(Hidden);
    }

    // Entry trigger
    if let Some(ref trigger) = prop_template.entry_trigger {
        entity.insert(EntryTrigger);

        for (effect, value) in trigger.effects.iter() {
            match effect.as_str() {
                "damage" => {
                    let damage = value.parse::<i32>().unwrap_or(0);
                    entity.insert(InflictsDamage { damage });
                }
                _ => {}
            }
        }
    }

    // Single activation
    if prop_template.single_activation.unwrap_or(false) {
        entity.insert(SingleActivation);
    }

    true
}
