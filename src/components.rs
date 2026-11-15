use bevy::prelude::*;

use crate::entity_render::{EntityShape, Renderable};

#[derive(Bundle)]
pub struct RenderableBundle {
    pub renderable: Renderable,
    pub shape: EntityShape,
    pub visibility: Visibility,
}

impl RenderableBundle {
    pub fn new(glyph: &str, fg: Color, shape: EntityShape) -> Self {
        Self {
            renderable: Renderable {
                glyph: glyph.to_string(),
                fg,
            },
            shape,
            visibility: Visibility::Inherited,
        }
    }
}

#[derive(Component, Debug)]
pub struct Name {
    pub name: String,
}

#[derive(Component, Debug)]
pub struct BlocksTile;

#[derive(Component, Debug)]
pub struct Item;

#[derive(Component, Debug)]
pub struct Consumable;

#[derive(Component, Debug)]
pub struct ProvidesHealing {
    pub heal_amount: i32,
}

#[derive(Component, Debug)]
pub struct Ranged {
    pub range: i32,
}

#[derive(Component, Debug)]
pub struct InflictsDamage {
    pub damage: i32,
}

#[derive(Component, Debug)]
pub struct AreaOfEffect {
    pub radius: i32,
}

#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Targeting {
    #[default]
    Tile,           // Can target any tile
    SingleEntity,   // Must target an entity
    // Future: MultiEntity { count: i32 }, Line { length: i32 }, etc.
}

#[derive(Component, Debug)]
pub struct Confusion {
    pub turns: i32,
}

#[derive(Component, Debug)]
pub struct CausesConfusion {
    pub turns: i32,
}

#[derive(Component, Debug)]
pub struct InBackpack {
    pub owner: Entity,
}

#[derive(Component, Debug)]
pub struct WantsToPickupItem {
    pub collected_by: Entity,
    pub item: Entity,
}

#[derive(Component, Debug)]
pub struct WantsToUseItem {
    pub item: Entity,
    pub target: Option<(i32, i32)>,
}

#[derive(Component, Debug)]
pub struct WantsToDropItem {
    pub item: Entity,
}

// Equipment system
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum EquipmentSlot {
    Melee,
    Shield,
    Chest,
    Cloak,
}

#[derive(Component, Debug)]
pub struct Equippable {
    pub slot: EquipmentSlot,
}

#[derive(Component, Debug)]
pub struct Equipped {
    pub owner: Entity,
    pub slot: EquipmentSlot,
}

#[derive(Component, Debug)]
pub struct MeleePowerBonus {
    pub power: i32,
}

#[derive(Component, Debug)]
pub struct DefenseBonus {
    pub defense: i32,
}

#[derive(Component, Debug)]
pub struct WantsToRemoveItem {
    pub item: Entity,
}

// Hunger system
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum HungerState {
    #[default]
    WellFed,
    Normal,
    Hungry,
    Starving,
}

#[derive(Component, Debug)]
pub struct HungerClock {
    pub state: HungerState,
    pub duration: i32,
}

#[derive(Component, Debug)]
pub struct ProvidesFood;

#[derive(Component, Debug)]
pub struct MagicMapper;

// Trap system
#[derive(Component, Debug)]
pub struct Hidden;

#[derive(Component, Debug)]
pub struct EntryTrigger;

#[derive(Component, Debug)]
pub struct SingleActivation;

// Door system
#[derive(Component, Debug)]
pub struct Door {
    pub open: bool,
}

#[derive(Component, Debug)]
pub struct BlocksVisibility;
