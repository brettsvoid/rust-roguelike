//! Money. Item values live in the raws (`base_value`, `tier`); this module
//! holds the gold itself.

use bevy::prelude::*;

pub mod shop;
pub mod stock;
pub mod town;
pub mod trade;

/// The party's shared gold. A resource, not a player component, so it
/// belongs to the whole party.
#[derive(Resource, Default, Debug)]
pub struct Purse {
    pub gold: i32,
}

/// Gold on an entity: a pile on the floor (with `Item`) that goes into the
/// purse when picked up, or coins a monster carries and drops on death.
#[derive(Component, Clone, Copy, Debug)]
pub struct Gold {
    pub amount: i32,
}
