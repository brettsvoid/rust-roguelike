mod bsp_dungeon;
mod bsp_interior;
mod cellular_automata;
mod common;
mod dla;
mod drunkard;
mod maze;
mod simple_map;
mod voronoi;

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
pub use voronoi::VoronoiCellBuilder;

/// Returns a list of all available map builder names
pub fn get_builder_names() -> Vec<&'static str> {
    vec![
        "Simple Map",
        "BSP Dungeon",
        "BSP Interior",
        "Cellular Automata",
        "Drunkard (Open Area)",
        "Drunkard (Open Halls)",
        "Drunkard (Winding)",
        "Drunkard (Fat Passages)",
        "Drunkard (Symmetry)",
        "Maze",
        "DLA (Walk Inwards)",
        "DLA (Walk Outwards)",
        "DLA (Central Attractor)",
        "DLA (Insectoid)",
        "Voronoi (Euclidean)",
        "Voronoi (Manhattan)",
        "Voronoi (Chebyshev)",
    ]
}

/// Returns a specific builder by index
pub fn builder_by_index(index: usize, depth: i32) -> Box<dyn MapBuilder> {
    match index {
        0 => Box::new(SimpleMapBuilder::new(depth)),
        1 => Box::new(BspDungeonBuilder::new(depth)),
        2 => Box::new(BspInteriorBuilder::new(depth)),
        3 => Box::new(CellularAutomataBuilder::new(depth)),
        4 => Box::new(DrunkardsWalkBuilder::open_area(depth)),
        5 => Box::new(DrunkardsWalkBuilder::open_halls(depth)),
        6 => Box::new(DrunkardsWalkBuilder::winding_passages(depth)),
        7 => Box::new(DrunkardsWalkBuilder::fat_passages(depth)),
        8 => Box::new(DrunkardsWalkBuilder::fearful_symmetry(depth)),
        9 => Box::new(MazeBuilder::new(depth)),
        10 => Box::new(DLABuilder::walk_inwards(depth)),
        11 => Box::new(DLABuilder::walk_outwards(depth)),
        12 => Box::new(DLABuilder::central_attractor(depth)),
        13 => Box::new(DLABuilder::insectoid(depth)),
        14 => Box::new(VoronoiCellBuilder::euclidean(depth)),
        15 => Box::new(VoronoiCellBuilder::manhattan(depth)),
        _ => Box::new(VoronoiCellBuilder::chebyshev(depth)),
    }
}

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
    match rng.0.gen_range(0..17) {
        0 => Box::new(SimpleMapBuilder::new(depth)),
        1 => Box::new(BspDungeonBuilder::new(depth)),
        2 => Box::new(BspInteriorBuilder::new(depth)),
        3 => Box::new(CellularAutomataBuilder::new(depth)),
        4 => Box::new(DrunkardsWalkBuilder::open_area(depth)),
        5 => Box::new(DrunkardsWalkBuilder::open_halls(depth)),
        6 => Box::new(DrunkardsWalkBuilder::winding_passages(depth)),
        7 => Box::new(DrunkardsWalkBuilder::fat_passages(depth)),
        8 => Box::new(DrunkardsWalkBuilder::fearful_symmetry(depth)),
        9 => Box::new(MazeBuilder::new(depth)),
        10 => Box::new(DLABuilder::walk_inwards(depth)),
        11 => Box::new(DLABuilder::walk_outwards(depth)),
        12 => Box::new(DLABuilder::central_attractor(depth)),
        13 => Box::new(DLABuilder::insectoid(depth)),
        14 => Box::new(VoronoiCellBuilder::euclidean(depth)),
        15 => Box::new(VoronoiCellBuilder::manhattan(depth)),
        _ => Box::new(VoronoiCellBuilder::chebyshev(depth)),
    }
}
