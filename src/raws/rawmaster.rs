use std::collections::{HashMap, HashSet};

use bevy::prelude::*;

use crate::abilities::HeroAbility;
use crate::bystander::Bystander;
use crate::combat::{CombatStats, DEFAULT_AIM};
use crate::cover::Cover;
use crate::economy::Gold;
use crate::entity_render::EntityShape;
use crate::models::Model;
use crate::components::{
    AreaOfEffect, BlocksTile, BlocksVisibility, CausesConfusion, Consumable, DefenseBonus,
    Door, EntryTrigger, EquipmentSlot, Equippable, Equipped, Hidden, InBackpack, InflictsDamage, Item,
    MagicMapper, Name, Poison, PoisonsOnHit, ProvidesFood, ProvidesHealing, Ranged,
    RenderableBundle, SingleActivation, Targeting, TownPortal, Weapon,
};
use crate::lighting::LightSource;
use crate::map::Position;
use crate::monsters::Monster;
use crate::rng::RandomTable;
use crate::tactics::DEFAULT_MOVEMENT;
use crate::viewshed::Viewshed;

use super::{AbilityEffect, Raws};

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
                spawn_table: Vec::new(),
                classes: Vec::new(),
                abilities: Vec::new(),
            },
            item_index: HashMap::new(),
            mob_index: HashMap::new(),
            prop_index: HashMap::new(),
        }
    }

    pub fn class(&self, name: &str) -> Option<&super::Class> {
        self.raws.classes.iter().find(|c| c.name == name)
    }

    pub fn load(&mut self, raws: Raws) {
        self.raws = raws;
        self.item_index.clear();
        self.mob_index.clear();
        self.prop_index.clear();

        let mut used_names: HashSet<String> = HashSet::new();

        for (i, item) in self.raws.items.iter().enumerate() {
            if used_names.contains(&item.name) {
                warn!("Duplicate item definition: {}", item.name);
            }
            used_names.insert(item.name.clone());
            self.item_index.insert(item.name.clone(), i);
        }
        for (i, mob) in self.raws.mobs.iter().enumerate() {
            if used_names.contains(&mob.name) {
                warn!("Duplicate mob definition: {}", mob.name);
            }
            used_names.insert(mob.name.clone());
            self.mob_index.insert(mob.name.clone(), i);
        }
        for (i, prop) in self.raws.props.iter().enumerate() {
            if used_names.contains(&prop.name) {
                warn!("Duplicate prop definition: {}", prop.name);
            }
            used_names.insert(prop.name.clone());
            self.prop_index.insert(prop.name.clone(), i);
        }

        // Validate spawn table entries reference defined entities
        for entry in &self.raws.spawn_table {
            if !used_names.contains(&entry.name) {
                warn!(
                    "Spawn table references undefined entity: {}",
                    entry.name
                );
            }
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


/// Spawn a named entity (item, mob, or prop)
pub fn spawn_named_entity(
    raws: &RawMaster,
    commands: &mut Commands,
    key: &str,
    x: i32,
    y: i32,
    monster_id: Option<usize>,
) -> bool {
    if raws.item_index.contains_key(key) {
        return spawn_named_item(raws, commands, key, x, y).is_some();
    } else if raws.mob_index.contains_key(key) {
        return spawn_named_mob(raws, commands, key, x, y, monster_id);
    } else if raws.prop_index.contains_key(key) {
        return spawn_named_prop(raws, commands, key, x, y);
    }
    false
}

fn insert_renderable(entity: &mut EntityCommands, renderable: &super::Renderable, shape: EntityShape) {
    // The model goes in first: inserting the shape dresses the entity, and that reads it.
    if let Some(model) = &renderable.model {
        entity.insert(Model(model.clone()));
    }
    entity.insert(RenderableBundle::new(&renderable.glyph, parse_color(&renderable.fg), shape));
}

/// The components for a scroll's `effects` (also cast by spell abilities).
fn insert_effects(entity: &mut EntityCommands, effects: &HashMap<String, String>) {
    for (effect, value) in effects.iter() {
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
            "targeting" => match value.as_str() {
                "single_entity" => {
                    entity.insert(Targeting::SingleEntity);
                }
                "ally" => {
                    entity.insert(Targeting::Ally);
                }
                _ => {}
            },
            "magic_mapping" => {
                entity.insert(MagicMapper);
            }
            "town_portal" => {
                entity.insert(TownPortal);
            }
            _ => {}
        }
    }
}

/// Spawn an item from raw data
pub fn spawn_named_item(
    raws: &RawMaster,
    commands: &mut Commands,
    key: &str,
    x: i32,
    y: i32,
) -> Option<Entity> {
    let &idx = raws.item_index.get(key)?;
    let item_template = &raws.raws.items[idx];

    let mut entity = commands.spawn((
        Item,
        Name {
            name: item_template.name.clone(),
        },
        Position { x, y },
    ));

    if let Some(ref renderable) = item_template.renderable {
        insert_renderable(&mut entity, renderable, EntityShape::Item);
    }

    // Handle consumable effects
    if let Some(ref consumable) = item_template.consumable {
        entity.insert(Consumable);
        insert_effects(&mut entity, &consumable.effects);
    }

    // Handle weapon
    if let Some(ref weapon) = item_template.weapon {
        let slot = equipment_slot(item_template).unwrap_or(EquipmentSlot::Melee);
        entity.insert(Equippable { slot });
        entity.insert(Weapon {
            damage: weapon.damage,
            range: weapon.range,
            optimal_range: weapon.optimal_range.unwrap_or(weapon.range),
            crit_chance: weapon.crit_chance,
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

    if let Some(ref armour) = item_template.armour {
        let slot = equipment_slot(item_template).unwrap_or(EquipmentSlot::Chest);
        entity.insert(Equippable { slot });
        entity.insert(DefenseBonus {
            defense: armour.defense_bonus,
        });
    }

    if let Some(range) = item_template.gold {
        entity.insert(Gold { amount: roll_gold(range) });
    }

    Some(entity.id())
}

/// The slot an item is worn or wielded in, `None` if it cannot be equipped.
fn equipment_slot(item: &super::Item) -> Option<EquipmentSlot> {
    if let Some(weapon) = &item.weapon {
        Some(if weapon.slot == "shield" { EquipmentSlot::Shield } else { EquipmentSlot::Melee })
    } else if item.shield.is_some() {
        Some(EquipmentSlot::Shield)
    } else {
        let armour = item.armour.as_ref()?;
        Some(if armour.slot == "cloak" { EquipmentSlot::Cloak } else { EquipmentSlot::Chest })
    }
}

/// Gives a new hero their class: its gear (the first item per slot equipped,
/// the rest in the backpack) and an entity per ability, a child of the hero
/// (`abilities::HeroAbility`). Spells carry the scroll components of their
/// effects, so the item code casts them. Warns and skips names the raws lack.
pub fn spawn_class_kit(raws: &RawMaster, commands: &mut Commands, hero: Entity, class: &super::Class) {
    let mut worn = Vec::new();
    for name in &class.gear {
        let (Some(item), Some(template)) = (
            spawn_named_item(raws, commands, name, 0, 0),
            raws.item_index.get(name).map(|&i| &raws.raws.items[i]),
        ) else {
            warn!("{}: no item {name}", class.name);
            continue;
        };
        let mut item = commands.entity(item);
        item.remove::<Position>().insert(Visibility::Hidden);
        match equipment_slot(template).filter(|slot| !worn.contains(slot)) {
            Some(slot) => {
                worn.push(slot);
                item.insert(Equipped { owner: hero, slot });
            }
            None => {
                item.insert(InBackpack { owner: hero });
            }
        }
    }
    for (slot, name) in class.abilities.iter().enumerate() {
        let Some(ability) = raws.raws.abilities.iter().find(|a| &a.name == name) else {
            warn!("{}: no ability {name}", class.name);
            continue;
        };
        let mut entity = commands.spawn((
            Name { name: ability.name.clone() },
            HeroAbility::new(slot, ability),
            ChildOf(hero),
        ));
        match &ability.effect {
            AbilityEffect::Spell(effects) => insert_effects(&mut entity, effects),
            AbilityEffect::AimedShot { .. } => {
                entity.insert(Targeting::SingleEntity);
            }
            AbilityEffect::Cleave => {}
        }
    }
}

/// shortcut: rolls with the thread rng, not `GameRng`; thread `GameRng`
/// through the spawn functions if seeded runs need to reproduce gold.
fn roll_gold((min, max): (i32, i32)) -> i32 {
    rand::random_range(min..=max)
}

/// Spawn a mob from raw data
pub fn spawn_named_mob(
    raws: &RawMaster,
    commands: &mut Commands,
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
        Name { name },
        Position { x, y },
        CombatStats {
            max_hp: mob_template.stats.max_hp,
            hp: mob_template.stats.hp,
            armour: mob_template.stats.defense,
            power: mob_template.stats.power,
            movement: mob_template.stats.movement.unwrap_or(DEFAULT_MOVEMENT),
            aim: mob_template.stats.aim.unwrap_or(DEFAULT_AIM),
            defence: mob_template.stats.defence,
        },
        Viewshed {
            range: mob_template.vision_range,
            ..default()
        },
    ));

    // Add AI component based on type
    match mob_template.ai.as_deref() {
        Some("bystander") => {
            entity.insert(Bystander);
        }
        _ => {
            // Default to monster (aggressive) AI
            entity.insert(Monster);
        }
    }

    if mob_template.blocks_tile {
        entity.insert(BlocksTile);
    }

    if let Some((damage, turns)) = mob_template.poison {
        entity.insert(PoisonsOnHit(Poison { damage, turns }));
    }

    if let Some(range) = mob_template.gold {
        entity.insert(Gold { amount: roll_gold(range) });
    }

    if let Some(ref renderable) = mob_template.renderable {
        insert_renderable(&mut entity, renderable, EntityShape::Creature);
    }

    true
}

/// Spawn a prop from raw data
pub fn spawn_named_prop(
    raws: &RawMaster,
    commands: &mut Commands,
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

    if let Some(ref renderable) = prop_template.renderable {
        let shape = match prop_template.door_open {
            Some(false) => EntityShape::Door,
            Some(true) => EntityShape::OpenDoor,
            None => EntityShape::Prop,
        };
        insert_renderable(&mut entity, renderable, shape);
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
            if effect.as_str() == "damage" {
                let damage = value.parse::<i32>().unwrap_or(0);
                entity.insert(InflictsDamage { damage });
            }
        }
    }

    // Single activation
    if prop_template.single_activation.unwrap_or(false) {
        entity.insert(SingleActivation);
    }

    if let Some(cover) = prop_template.cover.filter(|&c| c != Cover::None) {
        entity.insert(cover);
    }

    if let Some(ref light) = prop_template.light {
        entity.insert(LightSource {
            colour: parse_color(&light.colour),
            radius: light.radius,
            flicker: light.flicker,
            height: light.height,
        });
    }

    true
}

/// Build a RandomTable for a specific depth from JSON spawn data
pub fn get_spawn_table_for_depth(raws: &RawMaster, depth: i32) -> RandomTable {
    let mut table = RandomTable::new();

    for entry in &raws.raws.spawn_table {
        if depth >= entry.min_depth && depth <= entry.max_depth {
            let mut weight = entry.weight;
            if entry.add_map_depth_to_weight {
                weight += depth;
            }
            if weight > 0 {
                table = table.add(&entry.name, weight);
            }
        }
    }

    table
}
