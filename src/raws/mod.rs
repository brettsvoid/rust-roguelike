mod item_structs;
mod mob_structs;
mod prop_structs;
mod rawmaster;
mod spawn_table_structs;

pub use item_structs::*;
pub use mob_structs::*;
pub use prop_structs::*;
pub use rawmaster::*;
pub use spawn_table_structs::*;

use lazy_static::lazy_static;
use serde::Deserialize;
use std::sync::Mutex;

#[derive(Deserialize, Debug, Clone)]
pub struct Raws {
    pub items: Vec<Item>,
    pub mobs: Vec<Mob>,
    pub props: Vec<Prop>,
    pub spawn_table: Vec<SpawnTableEntry>,
}

lazy_static! {
    pub static ref RAWS: Mutex<RawMaster> = Mutex::new(RawMaster::empty());
}

pub fn load_raws() {
    let raw_string = include_str!("../../raws/spawns.json");
    let raws: Raws = serde_json::from_str(raw_string).expect("Failed to parse spawns.json");
    RAWS.lock().unwrap().load(raws);
}
