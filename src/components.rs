use bevy::prelude::*;

#[derive(Component, Clone, Copy)]
pub struct RenderOrder(pub i32);

impl RenderOrder {
    pub const ITEM: RenderOrder = RenderOrder(0);
    pub const MONSTER: RenderOrder = RenderOrder(1);
    pub const PLAYER: RenderOrder = RenderOrder(2);
}

#[derive(Bundle)]
pub struct RenderableBundle {
    pub text: Text2d,
    pub font: TextFont,
    pub color: TextColor,
    pub background: BackgroundColor,
    pub render_order: RenderOrder,
}

impl RenderableBundle {
    pub fn new(glyph: &str, fg: Color, bg: Color, render_order: RenderOrder, font: &TextFont) -> Self {
        Self {
            text: Text2d::new(glyph),
            font: font.clone(),
            color: TextColor(fg),
            background: BackgroundColor(bg),
            render_order,
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
pub struct Potion {
    pub heal_amount: i32,
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
pub struct WantsToDrinkPotion {
    pub potion: Entity,
}

#[derive(Component, Debug)]
pub struct WantsToDropItem {
    pub item: Entity,
}
