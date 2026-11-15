use std::collections::HashMap;

use serde::Deserialize;

use super::item_structs::Renderable;

#[derive(Deserialize, Debug, Clone)]
pub struct Prop {
    pub name: String,
    pub renderable: Option<Renderable>,
    pub hidden: Option<bool>,
    pub blocks_tile: Option<bool>,
    pub blocks_visibility: Option<bool>,
    pub door_open: Option<bool>,
    pub entry_trigger: Option<EntryTrigger>,
    pub single_activation: Option<bool>,
    pub light: Option<Light>,
}

/// Becomes a `LightSource` (see `lighting.rs`).
#[derive(Deserialize, Debug, Clone)]
pub struct Light {
    pub colour: String,
    pub radius: f32,
    pub flicker: f32,
    pub height: f32,
}

#[derive(Deserialize, Debug, Clone)]
pub struct EntryTrigger {
    pub effects: HashMap<String, String>,
}
