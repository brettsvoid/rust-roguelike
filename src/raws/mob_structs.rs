use serde::Deserialize;

use super::item_structs::Renderable;
use crate::enemy_ai::Archetype;

#[derive(Deserialize, Debug, Clone)]
pub struct Mob {
    pub name: String,
    pub renderable: Option<Renderable>,
    pub blocks_tile: bool,
    pub stats: MobStats,
    pub vision_range: i32,
    #[serde(default)]
    pub ai: Option<String>, // "melee" (default) or "bystander"
    /// Gold carried, rolled in this inclusive range at spawn, dropped on death.
    #[serde(default)]
    pub gold: Option<(i32, i32)>,
    /// Melee hits poison: `[damage per turn, turns]`.
    #[serde(default)]
    pub poison: Option<(i32, i32)>,
    /// How it fights (`enemy_ai::Archetype`): "brute" (the default),
    /// "skirmisher", "ranged" or "coward".
    #[serde(default)]
    pub archetype: Option<Archetype>,
    /// The items entry whose `weapon` it attacks with (e.g. "Shortbow");
    /// without one, `Weapon::NATURAL`.
    #[serde(default)]
    pub weapon: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct MobStats {
    pub max_hp: i32,
    pub hp: i32,
    pub power: i32,
    /// Armour: taken off each hit's damage (`CombatStats::armour`). Not `defence`.
    pub defense: i32,
    /// Tiles per action point; `tactics::DEFAULT_MOVEMENT` when missing.
    #[serde(default)]
    pub movement: Option<i32>,
    /// Base chance to hit in percent; `combat::DEFAULT_AIM` when missing.
    #[serde(default)]
    pub aim: Option<i32>,
    /// Taken off attackers' chance to hit.
    #[serde(default)]
    pub defence: i32,
}
