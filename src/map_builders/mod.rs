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

/// All available map builder types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuilderType {
    SimpleMap,
    BspDungeon,
    BspInterior,
    CellularAutomata,
    DrunkardOpenArea,
    DrunkardOpenHalls,
    DrunkardWinding,
    DrunkardFatPassages,
    DrunkardSymmetry,
    Maze,
    DlaWalkInwards,
    DlaWalkOutwards,
    DlaCentralAttractor,
    DlaInsectoid,
    VoronoiEuclidean,
    VoronoiManhattan,
    VoronoiChebyshev,
}

impl BuilderType {
    /// All builder types in order
    pub const ALL: &'static [BuilderType] = &[
        BuilderType::SimpleMap,
        BuilderType::BspDungeon,
        BuilderType::BspInterior,
        BuilderType::CellularAutomata,
        BuilderType::DrunkardOpenArea,
        BuilderType::DrunkardOpenHalls,
        BuilderType::DrunkardWinding,
        BuilderType::DrunkardFatPassages,
        BuilderType::DrunkardSymmetry,
        BuilderType::Maze,
        BuilderType::DlaWalkInwards,
        BuilderType::DlaWalkOutwards,
        BuilderType::DlaCentralAttractor,
        BuilderType::DlaInsectoid,
        BuilderType::VoronoiEuclidean,
        BuilderType::VoronoiManhattan,
        BuilderType::VoronoiChebyshev,
    ];

    /// Get the display name for this builder type
    pub fn name(&self) -> &'static str {
        match self {
            BuilderType::SimpleMap => "Simple Map",
            BuilderType::BspDungeon => "BSP Dungeon",
            BuilderType::BspInterior => "BSP Interior",
            BuilderType::CellularAutomata => "Cellular Automata",
            BuilderType::DrunkardOpenArea => "Drunkard (Open Area)",
            BuilderType::DrunkardOpenHalls => "Drunkard (Open Halls)",
            BuilderType::DrunkardWinding => "Drunkard (Winding)",
            BuilderType::DrunkardFatPassages => "Drunkard (Fat Passages)",
            BuilderType::DrunkardSymmetry => "Drunkard (Symmetry)",
            BuilderType::Maze => "Maze",
            BuilderType::DlaWalkInwards => "DLA (Walk Inwards)",
            BuilderType::DlaWalkOutwards => "DLA (Walk Outwards)",
            BuilderType::DlaCentralAttractor => "DLA (Central Attractor)",
            BuilderType::DlaInsectoid => "DLA (Insectoid)",
            BuilderType::VoronoiEuclidean => "Voronoi (Euclidean)",
            BuilderType::VoronoiManhattan => "Voronoi (Manhattan)",
            BuilderType::VoronoiChebyshev => "Voronoi (Chebyshev)",
        }
    }

    /// Create a builder instance for this type
    pub fn create(&self, depth: i32) -> Box<dyn MapBuilder> {
        match self {
            BuilderType::SimpleMap => Box::new(SimpleMapBuilder::new(depth)),
            BuilderType::BspDungeon => Box::new(BspDungeonBuilder::new(depth)),
            BuilderType::BspInterior => Box::new(BspInteriorBuilder::new(depth)),
            BuilderType::CellularAutomata => Box::new(CellularAutomataBuilder::new(depth)),
            BuilderType::DrunkardOpenArea => Box::new(DrunkardsWalkBuilder::open_area(depth)),
            BuilderType::DrunkardOpenHalls => Box::new(DrunkardsWalkBuilder::open_halls(depth)),
            BuilderType::DrunkardWinding => Box::new(DrunkardsWalkBuilder::winding_passages(depth)),
            BuilderType::DrunkardFatPassages => Box::new(DrunkardsWalkBuilder::fat_passages(depth)),
            BuilderType::DrunkardSymmetry => {
                Box::new(DrunkardsWalkBuilder::fearful_symmetry(depth))
            }
            BuilderType::Maze => Box::new(MazeBuilder::new(depth)),
            BuilderType::DlaWalkInwards => Box::new(DLABuilder::walk_inwards(depth)),
            BuilderType::DlaWalkOutwards => Box::new(DLABuilder::walk_outwards(depth)),
            BuilderType::DlaCentralAttractor => Box::new(DLABuilder::central_attractor(depth)),
            BuilderType::DlaInsectoid => Box::new(DLABuilder::insectoid(depth)),
            BuilderType::VoronoiEuclidean => Box::new(VoronoiCellBuilder::euclidean(depth)),
            BuilderType::VoronoiManhattan => Box::new(VoronoiCellBuilder::manhattan(depth)),
            BuilderType::VoronoiChebyshev => Box::new(VoronoiCellBuilder::chebyshev(depth)),
        }
    }

    /// Get builder type from index
    pub fn from_index(index: usize) -> Option<BuilderType> {
        BuilderType::ALL.get(index).copied()
    }
}

/// Returns a list of all available map builder names
pub fn get_builder_names() -> Vec<&'static str> {
    BuilderType::ALL.iter().map(|b| b.name()).collect()
}

/// Returns a specific builder by index
pub fn builder_by_index(index: usize, depth: i32) -> Box<dyn MapBuilder> {
    BuilderType::from_index(index)
        .unwrap_or(BuilderType::SimpleMap)
        .create(depth)
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
    let index = rng.0.gen_range(0..BuilderType::ALL.len());
    BuilderType::ALL[index].create(depth)
}

/// The default builder used for new games and level transitions.
/// Change this one line to use a different map generator everywhere.
pub fn default_builder(depth: i32) -> Box<dyn MapBuilder> {
    BuilderType::BspDungeon.create(depth)
}
