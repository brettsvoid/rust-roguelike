use std::collections::HashMap;

use serde::Deserialize;

use crate::economy::town::Sector;

#[derive(Deserialize, Debug, Clone)]
pub struct Item {
    pub name: String,
    /// Price in gold before vendor markup. Required, so every item has one.
    pub base_value: i32,
    /// Quality band, 1 to 5. A vendor stocks it once its sector reaches this
    /// level, and the spawn table keeps it to matching depths.
    pub tier: i32,
    /// The town sector whose stock this item joins when sold. Only gold has
    /// none.
    #[serde(default)]
    pub sector: Option<Sector>,
    /// A gold pile: rolls an amount in this inclusive range when spawned.
    #[serde(default)]
    pub gold: Option<(i32, i32)>,
    pub renderable: Option<Renderable>,
    pub consumable: Option<Consumable>,
    pub weapon: Option<Weapon>,
    pub shield: Option<Shield>,
    #[serde(default)]
    pub armour: Option<Armour>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Armour {
    /// "chest" or "cloak".
    pub slot: String,
    pub defense_bonus: i32,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Renderable {
    pub glyph: String,
    pub fg: String,
    /// Loads `assets/models/<model>.glb`; without the file the grey-box primitive stays.
    #[serde(default)]
    pub model: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Consumable {
    pub effects: HashMap<String, String>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Weapon {
    /// "melee" or "ranged"; both take the weapon hand.
    pub slot: String,
    /// Damage range rolled on a hit, added to the wielder's power.
    pub damage: (i32, i32),
    /// The farthest target in tiles; 1 (melee) when missing.
    #[serde(default = "melee_range")]
    pub range: i32,
    /// No range penalty up to this far; `range` when missing.
    #[serde(default)]
    pub optimal_range: Option<i32>,
    /// Percent chance that a hit is a critical.
    pub crit_chance: i32,
}

fn melee_range() -> i32 {
    1
}

#[derive(Deserialize, Debug, Clone)]
pub struct Shield {
    pub defense_bonus: i32,
}
