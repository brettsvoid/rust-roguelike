mod common;
mod simple_map;

use bevy::prelude::*;

use crate::map::Map;
use crate::rng::GameRng;

pub use simple_map::SimpleMapBuilder;

pub trait MapBuilder {
    fn build_map(&mut self, rng: &mut GameRng);
    fn spawn_entities(&self, commands: &mut Commands, rng: &mut GameRng, font: &TextFont);
    fn get_map(&self) -> Map;
    fn get_starting_position(&self) -> (i32, i32);
}

pub fn random_builder(depth: i32) -> Box<dyn MapBuilder> {
    // For now, always return simple map builder
    // Later chapters add more builders here
    Box::new(SimpleMapBuilder::new(depth))
}
