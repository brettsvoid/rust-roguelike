mod area_based;
mod bsp_dungeon;
mod bsp_interior;
mod cellular_automata;
mod common;
mod dla;
mod drunkard;
mod maze;
mod prefab;
mod room_based;
mod simple_map;
mod voronoi;
mod wfc;

use bevy::prelude::*;
use rand::Rng;

use crate::map::Map;
use crate::rng::GameRng;
use crate::shapes::Rect;

pub use area_based::{AreaStartingPosition, CullUnreachable, DistantExit, VoronoiSpawning, XStart, YStart};
pub use bsp_dungeon::BspDungeonBuilder;
pub use bsp_interior::BspInteriorBuilder;
pub use cellular_automata::CellularAutomataBuilder;
pub use dla::DLABuilder;
pub use drunkard::DrunkardsWalkBuilder;
pub use maze::MazeBuilder;
pub use prefab::{PrefabBuilder, PrefabMetaBuilder, CORNER_FORT};
pub use room_based::{RoomBasedSpawner, RoomBasedStairs, RoomBasedStartingPosition};
pub use simple_map::SimpleMapBuilder;
pub use voronoi::VoronoiCellBuilder;
pub use wfc::WfcBuilder;

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
    WfcCellularAutomata,
    WfcBspDungeon,
    WfcBspInterior,
    WfcDla,
    PrefabVaults,
    PrefabSectional,
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
        BuilderType::WfcCellularAutomata,
        BuilderType::WfcBspDungeon,
        BuilderType::WfcBspInterior,
        BuilderType::WfcDla,
        BuilderType::PrefabVaults,
        BuilderType::PrefabSectional,
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
            BuilderType::WfcCellularAutomata => "WFC (Cellular Automata)",
            BuilderType::WfcBspDungeon => "WFC (BSP Dungeon)",
            BuilderType::WfcBspInterior => "WFC (BSP Interior)",
            BuilderType::WfcDla => "WFC (DLA)",
            BuilderType::PrefabVaults => "Prefab (Vaults)",
            BuilderType::PrefabSectional => "Prefab (Sectional)",
        }
    }

    /// Create a builder instance for this type using the new BuilderChain architecture
    pub fn create(&self, depth: i32) -> Box<dyn MapBuilder> {
        match self {
            // Room-based builders - use room-based meta builders for starting position/stairs
            BuilderType::SimpleMap => Box::new(
                BuilderChain::new(depth, "Simple Map")
                    .start_with(Box::new(SimpleMapBuilder::new(depth)))
            ),
            BuilderType::BspDungeon => Box::new(
                BuilderChain::new(depth, "BSP Dungeon")
                    .start_with(Box::new(BspDungeonBuilder::new(depth)))
            ),
            BuilderType::BspInterior => Box::new(
                BuilderChain::new(depth, "BSP Interior")
                    .start_with(Box::new(BspInteriorBuilder::new(depth)))
            ),

            // Area-based builders - use area-based meta builders
            BuilderType::CellularAutomata => Box::new(
                BuilderChain::new(depth, "Cellular Automata")
                    .start_with(Box::new(CellularAutomataBuilder::new(depth)))
            ),
            BuilderType::DrunkardOpenArea => Box::new(
                BuilderChain::new(depth, "Drunkard (Open Area)")
                    .start_with(Box::new(DrunkardsWalkBuilder::open_area(depth)))
            ),
            BuilderType::DrunkardOpenHalls => Box::new(
                BuilderChain::new(depth, "Drunkard (Open Halls)")
                    .start_with(Box::new(DrunkardsWalkBuilder::open_halls(depth)))
            ),
            BuilderType::DrunkardWinding => Box::new(
                BuilderChain::new(depth, "Drunkard (Winding)")
                    .start_with(Box::new(DrunkardsWalkBuilder::winding_passages(depth)))
            ),
            BuilderType::DrunkardFatPassages => Box::new(
                BuilderChain::new(depth, "Drunkard (Fat Passages)")
                    .start_with(Box::new(DrunkardsWalkBuilder::fat_passages(depth)))
            ),
            BuilderType::DrunkardSymmetry => Box::new(
                BuilderChain::new(depth, "Drunkard (Symmetry)")
                    .start_with(Box::new(DrunkardsWalkBuilder::fearful_symmetry(depth)))
            ),
            BuilderType::Maze => Box::new(
                BuilderChain::new(depth, "Maze")
                    .start_with(Box::new(MazeBuilder::new(depth)))
            ),
            BuilderType::DlaWalkInwards => Box::new(
                BuilderChain::new(depth, "DLA (Walk Inwards)")
                    .start_with(Box::new(DLABuilder::walk_inwards(depth)))
            ),
            BuilderType::DlaWalkOutwards => Box::new(
                BuilderChain::new(depth, "DLA (Walk Outwards)")
                    .start_with(Box::new(DLABuilder::walk_outwards(depth)))
            ),
            BuilderType::DlaCentralAttractor => Box::new(
                BuilderChain::new(depth, "DLA (Central Attractor)")
                    .start_with(Box::new(DLABuilder::central_attractor(depth)))
            ),
            BuilderType::DlaInsectoid => Box::new(
                BuilderChain::new(depth, "DLA (Insectoid)")
                    .start_with(Box::new(DLABuilder::insectoid(depth)))
            ),
            BuilderType::VoronoiEuclidean => Box::new(
                BuilderChain::new(depth, "Voronoi (Euclidean)")
                    .start_with(Box::new(VoronoiCellBuilder::euclidean(depth)))
            ),
            BuilderType::VoronoiManhattan => Box::new(
                BuilderChain::new(depth, "Voronoi (Manhattan)")
                    .start_with(Box::new(VoronoiCellBuilder::manhattan(depth)))
            ),
            BuilderType::VoronoiChebyshev => Box::new(
                BuilderChain::new(depth, "Voronoi (Chebyshev)")
                    .start_with(Box::new(VoronoiCellBuilder::chebyshev(depth)))
            ),

            // WFC builders - use source builder + WFC as meta builder
            BuilderType::WfcCellularAutomata => Box::new(
                BuilderChain::new(depth, "WFC (Cellular Automata)")
                    .start_with(Box::new(CellularAutomataBuilder::new(depth)))
                    .with(Box::new(WfcBuilder::new(depth)))
                    .with(CullUnreachable::new())
                    .with(DistantExit::new())
                    .with(AreaStartingPosition::new(XStart::Center, YStart::Center))
                    .with(VoronoiSpawning::new())
            ),
            BuilderType::WfcBspDungeon => Box::new(
                BuilderChain::new(depth, "WFC (BSP Dungeon)")
                    .start_with(Box::new(BspDungeonBuilder::new(depth)))
                    .with(Box::new(WfcBuilder::new(depth)))
                    .with(CullUnreachable::new())
                    .with(DistantExit::new())
                    .with(AreaStartingPosition::new(XStart::Center, YStart::Center))
                    .with(VoronoiSpawning::new())
            ),
            BuilderType::WfcBspInterior => Box::new(
                BuilderChain::new(depth, "WFC (BSP Interior)")
                    .start_with(Box::new(BspInteriorBuilder::new(depth)))
                    .with(Box::new(WfcBuilder::new(depth)))
                    .with(CullUnreachable::new())
                    .with(DistantExit::new())
                    .with(AreaStartingPosition::new(XStart::Center, YStart::Center))
                    .with(VoronoiSpawning::new())
            ),
            BuilderType::WfcDla => Box::new(
                BuilderChain::new(depth, "WFC (DLA)")
                    .start_with(Box::new(DLABuilder::walk_inwards(depth)))
                    .with(Box::new(WfcBuilder::new(depth)))
                    .with(CullUnreachable::new())
                    .with(DistantExit::new())
                    .with(AreaStartingPosition::new(XStart::Center, YStart::Center))
                    .with(VoronoiSpawning::new())
            ),

            // Prefab builders - use base builder + prefab meta builder
            BuilderType::PrefabVaults => Box::new(
                BuilderChain::new(depth, "Prefab (Vaults)")
                    .start_with(Box::new(CellularAutomataBuilder::new(depth)))
                    .with(PrefabMetaBuilder::vaults())
            ),
            BuilderType::PrefabSectional => Box::new(
                BuilderChain::new(depth, "Prefab (Sectional)")
                    .start_with(Box::new(CellularAutomataBuilder::new(depth)))
                    .with(PrefabMetaBuilder::sectional(CORNER_FORT.clone()))
            ),
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

// ============================================================================
// Builder Chaining Architecture
// ============================================================================

use crate::map::{MAP_HEIGHT, MAP_WIDTH};

/// Shared state container for the builder chain
pub struct BuilderMap {
    pub map: Map,
    pub starting_position: Option<(i32, i32)>,
    pub rooms: Option<Vec<Rect>>,
    pub spawn_list: Vec<(usize, String)>,
    pub history: Vec<Map>,
    pub depth: i32,
}

impl BuilderMap {
    pub fn new(depth: i32) -> Self {
        Self {
            map: Map::new(MAP_WIDTH, MAP_HEIGHT, depth),
            starting_position: None,
            rooms: None,
            spawn_list: Vec::new(),
            history: Vec::new(),
            depth,
        }
    }

    pub fn take_snapshot(&mut self) {
        self.history.push(self.map.clone());
    }
}

/// Trait for builders that create a map from scratch
pub trait InitialMapBuilder {
    fn build_map(&mut self, rng: &mut GameRng, build_data: &mut BuilderMap);
}

/// Trait for builders that modify an existing map
pub trait MetaMapBuilder {
    fn build_map(&mut self, rng: &mut GameRng, build_data: &mut BuilderMap);
}

/// Orchestrates the builder pipeline
pub struct BuilderChain {
    starter: Option<Box<dyn InitialMapBuilder>>,
    builders: Vec<Box<dyn MetaMapBuilder>>,
    pub build_data: BuilderMap,
    name: &'static str,
}

impl BuilderChain {
    pub fn new(depth: i32, name: &'static str) -> Self {
        Self {
            starter: None,
            builders: Vec::new(),
            build_data: BuilderMap::new(depth),
            name,
        }
    }

    pub fn start_with(mut self, starter: Box<dyn InitialMapBuilder>) -> Self {
        self.starter = Some(starter);
        self
    }

    pub fn with(mut self, metabuilder: Box<dyn MetaMapBuilder>) -> Self {
        self.builders.push(metabuilder);
        self
    }

    fn run_build(&mut self, rng: &mut GameRng) {
        // Run the initial builder
        if let Some(ref mut starter) = self.starter {
            starter.build_map(rng, &mut self.build_data);
        }

        // Run each meta builder in sequence
        for builder in &mut self.builders {
            builder.build_map(rng, &mut self.build_data);
        }
    }
}

// Implement MapBuilder for BuilderChain so it can be used with existing game code
impl MapBuilder for BuilderChain {
    fn build_map(&mut self, rng: &mut GameRng) {
        self.run_build(rng);
    }

    fn spawn_entities(&self, commands: &mut Commands, rng: &mut GameRng, font: &TextFont) {
        // Spawn from rooms if available
        if let Some(ref rooms) = self.build_data.rooms {
            let mut monster_id: usize = 0;
            for room in rooms.iter().skip(1) {
                crate::spawner::spawn_room(commands, rng, font, room, &mut monster_id, self.build_data.depth);
            }
        }

        // Also spawn from spawn_list (for prefabs, etc.)
        for (idx, name) in &self.build_data.spawn_list {
            let x = (*idx % MAP_WIDTH) as i32;
            let y = (*idx / MAP_WIDTH) as i32;
            prefab::spawn_by_name(commands, font, x, y, name, &mut 0);
        }
    }

    fn get_map(&self) -> Map {
        self.build_data.map.clone()
    }

    fn get_starting_position(&self) -> (i32, i32) {
        self.build_data.starting_position.unwrap_or((MAP_WIDTH as i32 / 2, MAP_HEIGHT as i32 / 2))
    }

    fn get_snapshot_history(&self) -> Vec<Map> {
        self.build_data.history.clone()
    }

    fn take_snapshot(&mut self) {
        self.build_data.take_snapshot();
    }

    fn get_spawn_regions(&self) -> Vec<Rect> {
        self.build_data.rooms.clone().unwrap_or_default()
    }

    fn get_name(&self) -> &'static str {
        self.name
    }
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
