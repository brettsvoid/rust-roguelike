mod bsp_dungeon;
mod bsp_interior;
mod common;
mod simple_map;

use bevy::prelude::*;
use rand::Rng;

use crate::map::Map;
use crate::rng::GameRng;
use crate::shapes::Rect;

pub use bsp_dungeon::BspDungeonBuilder;
pub use bsp_interior::BspInteriorBuilder;
pub use simple_map::SimpleMapBuilder;

pub trait MapBuilder {
    fn build_map(&mut self, rng: &mut GameRng);
    fn spawn_entities(&self, commands: &mut Commands, rng: &mut GameRng, font: &TextFont);
    fn get_map(&self) -> Map;
    fn get_starting_position(&self) -> (i32, i32);
    fn get_snapshot_history(&self) -> Vec<Map>;
    fn take_snapshot(&mut self);
    fn get_spawn_regions(&self) -> Vec<Rect>;
    fn get_name(&self) -> &'static str;
}

pub fn random_builder(depth: i32, rng: &mut GameRng) -> Box<dyn MapBuilder> {
    match rng.0.gen_range(0..3) {
        0 => Box::new(SimpleMapBuilder::new(depth)),
        1 => Box::new(BspDungeonBuilder::new(depth)),
        _ => Box::new(BspInteriorBuilder::new(depth)),
    }
}
