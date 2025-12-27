mod bsp_dungeon;
mod bsp_interior;
mod cellular_automata;
mod common;
mod dla;
mod drunkard;
mod maze;
mod simple_map;

use bevy::prelude::*;
use rand::Rng;

use crate::map::Map;
use crate::rng::GameRng;
use crate::shapes::Rect;

pub use bsp_dungeon::BspDungeonBuilder;
pub use bsp_interior::BspInteriorBuilder;
pub use cellular_automata::CellularAutomataBuilder;
pub use dla::DLABuilder;
pub use drunkard::DrunkardsWalkBuilder;
pub use maze::MazeBuilder;
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
    match rng.0.gen_range(0..12) {
        0 => Box::new(SimpleMapBuilder::new(depth)),
        1 => Box::new(BspDungeonBuilder::new(depth)),
        2 => Box::new(BspInteriorBuilder::new(depth)),
        3 => Box::new(CellularAutomataBuilder::new(depth)),
        4 => Box::new(DrunkardsWalkBuilder::open_area(depth)),
        5 => Box::new(DrunkardsWalkBuilder::open_halls(depth)),
        6 => Box::new(DrunkardsWalkBuilder::winding_passages(depth)),
        7 => Box::new(MazeBuilder::new(depth)),
        8 => Box::new(DLABuilder::walk_inwards(depth)),
        9 => Box::new(DLABuilder::walk_outwards(depth)),
        10 => Box::new(DLABuilder::central_attractor(depth)),
        _ => Box::new(DLABuilder::insectoid(depth)),
    }
}
