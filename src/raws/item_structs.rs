use std::collections::HashMap;

use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
pub struct Item {
    pub name: String,
    /// Price in gold before vendor markup. Required, so every item has one.
    #[allow(dead_code)] // first read by vendors (E02)
    pub base_value: i32,
    /// Quality band, starting at 1. Gates vendor stock and depth of loot.
    #[allow(dead_code)] // first read by tiered stock (E05)
    pub tier: i32,
    /// A gold pile: rolls an amount in this inclusive range when spawned.
    #[serde(default)]
    pub gold: Option<(i32, i32)>,
    pub renderable: Option<Renderable>,
    pub consumable: Option<Consumable>,
    pub weapon: Option<Weapon>,
    pub shield: Option<Shield>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Renderable {
    pub glyph: String,
    pub fg: String,
    pub bg: String,
    pub order: i32,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Consumable {
    pub effects: HashMap<String, String>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Weapon {
    pub slot: String,
    pub power_bonus: i32,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Shield {
    pub defense_bonus: i32,
}
