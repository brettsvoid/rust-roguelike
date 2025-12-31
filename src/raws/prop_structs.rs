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
}

#[derive(Deserialize, Debug, Clone)]
pub struct EntryTrigger {
    pub effects: HashMap<String, String>,
}
