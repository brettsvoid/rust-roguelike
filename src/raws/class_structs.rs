use std::collections::HashMap;

use serde::Deserialize;

/// A hero class: starting stats, starting gear and abilities by name.
#[derive(Deserialize, Debug, Clone)]
pub struct Class {
    pub name: String,
    pub stats: ClassStats,
    /// Item names from `items`. The first item for each equipment slot is
    /// worn or wielded; anything else goes in the backpack.
    pub gear: Vec<String>,
    /// Ability names from `abilities`, in hotkey order (1, 2).
    pub abilities: Vec<String>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ClassStats {
    pub max_hp: i32,
    pub power: i32,
    /// Taken off each hit's damage (`CombatStats::armour`).
    pub armour: i32,
    pub aim: i32,
    /// Taken off attackers' chance to hit.
    pub defence: i32,
    /// Tiles per action point.
    pub movement: i32,
}

/// An ability: what it does and how many rounds it rests after use.
#[derive(Deserialize, Debug, Clone)]
pub struct Ability {
    pub name: String,
    /// Rounds before it can be used again (`abilities::Ability::left`).
    pub cooldown: u8,
    pub effect: AbilityEffect,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AbilityEffect {
    /// An attack with the wielded weapon on every hostile beside the hero,
    /// each rolled separately.
    Cleave,
    /// An attack with the wielded weapon on one hostile, with extra aim.
    AimedShot { aim_bonus: i32 },
    /// A scroll's effects (the keys of an item's `consumable.effects`), cast
    /// through the item code without using anything up. `"targeting": "ally"`
    /// aims it at a hero.
    Spell(HashMap<String, String>),
}
