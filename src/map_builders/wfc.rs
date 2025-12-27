use bevy::prelude::*;
use rand::Rng;
use std::collections::{HashMap, HashSet};

use crate::map::{Map, TileType, MAP_HEIGHT, MAP_WIDTH};
use crate::pathfinding::dijkstra_map;
use crate::rng::GameRng;
use crate::shapes::Rect;
use crate::spawner;

use super::MapBuilder;

/// Default chunk size for pattern extraction (in tiles)
/// Smaller = faster but less variety, larger = slower but more detail
pub const DEFAULT_CHUNK_SIZE: i32 = 4;

/// Maximum number of unique patterns to keep (for performance)
const MAX_PATTERNS: usize = 64;

/// Maximum retry attempts before falling back to source map
const MAX_RETRIES: u32 = 10;

// ============================================================================
// Direction
// ============================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Direction {
    North,
    South,
    East,
    West,
}

impl Direction {
    fn opposite(&self) -> Direction {
        match self {
            Direction::North => Direction::South,
            Direction::South => Direction::North,
            Direction::East => Direction::West,
            Direction::West => Direction::East,
        }
    }

    fn all() -> [Direction; 4] {
        [
            Direction::North,
            Direction::South,
            Direction::East,
            Direction::West,
        ]
    }
}

// ============================================================================
// MapChunk - A pattern extracted from source map
// ============================================================================

#[derive(Clone, Debug)]
struct MapChunk {
    /// Flattened tile data (size * size)
    tiles: Vec<TileType>,
    /// Size of chunk (width = height)
    size: i32,
    /// Frequency weight - how often this pattern appears
    weight: u32,
}

impl MapChunk {
    /// Create a chunk from a region of the source map
    fn from_map(map: &Map, start_x: i32, start_y: i32, size: i32) -> Self {
        let mut tiles = Vec::with_capacity((size * size) as usize);
        for y in start_y..start_y + size {
            for x in start_x..start_x + size {
                let idx = map.xy_idx(x, y);
                tiles.push(map.tiles[idx]);
            }
        }
        MapChunk {
            tiles,
            size,
            weight: 1,
        }
    }

    /// Get tile at local chunk coordinates
    fn get(&self, x: i32, y: i32) -> TileType {
        self.tiles[(y * self.size + x) as usize]
    }

    /// Extract edge tiles for a given direction
    fn get_edge(&self, direction: Direction) -> Vec<TileType> {
        match direction {
            Direction::North => (0..self.size).map(|x| self.get(x, 0)).collect(),
            Direction::South => (0..self.size).map(|x| self.get(x, self.size - 1)).collect(),
            Direction::West => (0..self.size).map(|y| self.get(0, y)).collect(),
            Direction::East => (0..self.size).map(|y| self.get(self.size - 1, y)).collect(),
        }
    }
}

// ============================================================================
// Constraints - Which chunks can be adjacent
// ============================================================================

struct Constraints {
    /// For each chunk index, maps direction to set of compatible chunk indices
    adjacency: Vec<HashMap<Direction, HashSet<usize>>>,
}

impl Constraints {
    fn new(num_chunks: usize) -> Self {
        Constraints {
            adjacency: vec![HashMap::new(); num_chunks],
        }
    }

    fn add_adjacency(&mut self, chunk_a: usize, direction: Direction, chunk_b: usize) {
        self.adjacency[chunk_a]
            .entry(direction)
            .or_default()
            .insert(chunk_b);
    }

    fn get_compatible(&self, chunk_idx: usize, direction: Direction) -> Option<&HashSet<usize>> {
        self.adjacency[chunk_idx].get(&direction)
    }
}

// ============================================================================
// Cell - State of a single position during solving
// ============================================================================

#[derive(Clone)]
struct Cell {
    /// Set of chunk indices that could still be placed here
    possible: HashSet<usize>,
    /// If collapsed, the chosen chunk index
    collapsed: Option<usize>,
}

impl Cell {
    fn new(num_patterns: usize) -> Self {
        Cell {
            possible: (0..num_patterns).collect(),
            collapsed: None,
        }
    }

    fn entropy(&self) -> usize {
        if self.collapsed.is_some() {
            0
        } else {
            self.possible.len()
        }
    }

    fn is_collapsed(&self) -> bool {
        self.collapsed.is_some()
    }

    fn is_contradiction(&self) -> bool {
        self.collapsed.is_none() && self.possible.is_empty()
    }
}

// ============================================================================
// WfcSolver - The core WFC algorithm
// ============================================================================

struct WfcSolver {
    /// Grid of cells (in chunk coordinates)
    cells: Vec<Cell>,
    /// Width of grid in chunks
    grid_width: i32,
    /// Height of grid in chunks
    grid_height: i32,
    /// All extracted patterns
    patterns: Vec<MapChunk>,
    /// Adjacency constraints
    constraints: Constraints,
    /// Chunk size in tiles
    chunk_size: i32,
}

impl WfcSolver {
    fn new(source: &Map, chunk_size: i32) -> Self {
        let patterns = Self::extract_patterns(source, chunk_size);
        let constraints = Self::build_constraints(&patterns);

        let grid_width = MAP_WIDTH as i32 / chunk_size;
        let grid_height = MAP_HEIGHT as i32 / chunk_size;

        let cells = vec![Cell::new(patterns.len()); (grid_width * grid_height) as usize];

        WfcSolver {
            cells,
            grid_width,
            grid_height,
            patterns,
            constraints,
            chunk_size,
        }
    }

    /// Extract unique chunks from source map, limited to MAX_PATTERNS
    fn extract_patterns(source: &Map, chunk_size: i32) -> Vec<MapChunk> {
        let mut pattern_counts: HashMap<Vec<TileType>, usize> = HashMap::new();
        let mut patterns: Vec<MapChunk> = Vec::new();

        // Slide window across source map
        for y in 0..=(source.height - chunk_size) {
            for x in 0..=(source.width - chunk_size) {
                let chunk = MapChunk::from_map(source, x, y, chunk_size);

                // Deduplicate by tile content, track frequency
                if let Some(&idx) = pattern_counts.get(&chunk.tiles) {
                    patterns[idx].weight += 1;
                } else {
                    pattern_counts.insert(chunk.tiles.clone(), patterns.len());
                    patterns.push(chunk);
                }
            }
        }

        // Limit patterns to MAX_PATTERNS, keeping highest frequency ones
        if patterns.len() > MAX_PATTERNS {
            patterns.sort_by(|a, b| b.weight.cmp(&a.weight));
            patterns.truncate(MAX_PATTERNS);
        }

        patterns
    }

    /// Build adjacency constraints by comparing chunk edges
    fn build_constraints(patterns: &[MapChunk]) -> Constraints {
        let mut constraints = Constraints::new(patterns.len());

        for (i, chunk_a) in patterns.iter().enumerate() {
            for (j, chunk_b) in patterns.iter().enumerate() {
                for direction in Direction::all() {
                    let edge_a = chunk_a.get_edge(direction);
                    let edge_b = chunk_b.get_edge(direction.opposite());

                    // Chunks are compatible if their facing edges match
                    if edge_a == edge_b {
                        constraints.add_adjacency(i, direction, j);
                    }
                }
            }
        }

        constraints
    }

    fn cell_idx(&self, x: i32, y: i32) -> usize {
        (y * self.grid_width + x) as usize
    }

    /// Find the cell with lowest non-zero entropy
    fn find_lowest_entropy(&self, rng: &mut GameRng) -> Option<usize> {
        let mut min_entropy = usize::MAX;
        let mut candidates = Vec::new();

        for (idx, cell) in self.cells.iter().enumerate() {
            if cell.is_collapsed() || cell.is_contradiction() {
                continue;
            }
            let entropy = cell.entropy();
            if entropy < min_entropy {
                min_entropy = entropy;
                candidates.clear();
                candidates.push(idx);
            } else if entropy == min_entropy {
                candidates.push(idx);
            }
        }

        if candidates.is_empty() {
            None
        } else {
            Some(candidates[rng.0.gen_range(0..candidates.len())])
        }
    }

    /// Collapse a cell to a single pattern (weighted by frequency)
    fn collapse(&mut self, cell_idx: usize, rng: &mut GameRng) {
        let cell = &self.cells[cell_idx];
        if cell.possible.is_empty() {
            return;
        }

        let possible: Vec<usize> = cell.possible.iter().copied().collect();
        let total_weight: u32 = possible.iter().map(|&i| self.patterns[i].weight).sum();

        let mut roll = rng.0.gen_range(0..total_weight);
        let mut chosen = possible[0];

        for &pattern_idx in &possible {
            let weight = self.patterns[pattern_idx].weight;
            if roll < weight {
                chosen = pattern_idx;
                break;
            }
            roll -= weight;
        }

        self.cells[cell_idx].collapsed = Some(chosen);
        self.cells[cell_idx].possible.clear();
    }

    /// Propagate constraints from a collapsed cell to neighbors
    fn propagate(&mut self, start_idx: usize) -> bool {
        let mut stack = vec![start_idx];
        let max_iterations = self.cells.len() * 4;
        let mut iterations = 0;

        while let Some(cell_idx) = stack.pop() {
            iterations += 1;
            if iterations > max_iterations {
                return false; // Propagation taking too long
            }
            let x = (cell_idx as i32) % self.grid_width;
            let y = (cell_idx as i32) / self.grid_width;

            let current_possible: HashSet<usize> =
                if let Some(collapsed) = self.cells[cell_idx].collapsed {
                    [collapsed].into_iter().collect()
                } else {
                    self.cells[cell_idx].possible.clone()
                };

            for direction in Direction::all() {
                let (nx, ny) = match direction {
                    Direction::North => (x, y - 1),
                    Direction::South => (x, y + 1),
                    Direction::West => (x - 1, y),
                    Direction::East => (x + 1, y),
                };

                if nx < 0 || nx >= self.grid_width || ny < 0 || ny >= self.grid_height {
                    continue;
                }

                let neighbor_idx = self.cell_idx(nx, ny);
                if self.cells[neighbor_idx].is_collapsed() {
                    continue;
                }

                let mut valid_for_neighbor: HashSet<usize> = HashSet::new();
                for &pattern_idx in &current_possible {
                    if let Some(compatible) = self.constraints.get_compatible(pattern_idx, direction)
                    {
                        valid_for_neighbor.extend(compatible);
                    }
                }

                let old_count = self.cells[neighbor_idx].possible.len();
                self.cells[neighbor_idx]
                    .possible
                    .retain(|p| valid_for_neighbor.contains(p));
                let new_count = self.cells[neighbor_idx].possible.len();

                if new_count < old_count {
                    if new_count == 0 {
                        return false; // Contradiction
                    }
                    stack.push(neighbor_idx);
                }
            }
        }

        true
    }

    /// Run WFC algorithm to completion, taking snapshots for visualization
    fn solve(&mut self, rng: &mut GameRng, map: &mut Map, history: &mut Vec<Map>) -> bool {
        // Guard against empty patterns
        if self.patterns.is_empty() {
            return false;
        }

        // Limit iterations to prevent hangs
        let max_iterations = self.cells.len() * 2;
        let mut iterations = 0;

        // Snapshot every N collapses for visualization
        let snapshot_interval = (self.cells.len() / 10).max(1);

        while let Some(cell_idx) = self.find_lowest_entropy(rng) {
            iterations += 1;
            if iterations > max_iterations {
                return false; // Taking too long, fail and retry
            }

            self.collapse(cell_idx, rng);

            if !self.propagate(cell_idx) {
                return false;
            }

            // Take periodic snapshots
            if iterations % snapshot_interval == 0 {
                self.render_to_map(map);
                history.push(map.clone());
            }
        }

        self.cells.iter().all(|c| c.is_collapsed())
    }

    /// Render solved grid to map
    fn render_to_map(&self, map: &mut Map) {
        for cy in 0..self.grid_height {
            for cx in 0..self.grid_width {
                let cell_idx = self.cell_idx(cx, cy);
                if let Some(pattern_idx) = self.cells[cell_idx].collapsed {
                    let chunk = &self.patterns[pattern_idx];

                    for local_y in 0..self.chunk_size {
                        for local_x in 0..self.chunk_size {
                            let map_x = cx * self.chunk_size + local_x;
                            let map_y = cy * self.chunk_size + local_y;

                            if map_x < MAP_WIDTH as i32 && map_y < MAP_HEIGHT as i32 {
                                let map_idx = map.xy_idx(map_x, map_y);
                                map.tiles[map_idx] = chunk.get(local_x, local_y);
                            }
                        }
                    }
                }
            }
        }
    }
}

// ============================================================================
// WfcSourceType - Which generator to use for source patterns
// ============================================================================

#[derive(Clone, Copy, Debug)]
pub enum WfcSourceType {
    CellularAutomata,
    BspDungeon,
    BspInterior,
    Dla,
}

// ============================================================================
// WfcBuilder - The main map builder
// ============================================================================

pub struct WfcBuilder {
    map: Map,
    starting_position: (i32, i32),
    depth: i32,
    history: Vec<Map>,
    spawn_regions: Vec<Vec<usize>>,
    chunk_size: i32,
    source_type: WfcSourceType,
}

impl WfcBuilder {
    pub fn new(depth: i32) -> Self {
        Self::with_options(depth, DEFAULT_CHUNK_SIZE, WfcSourceType::CellularAutomata)
    }

    pub fn with_options(depth: i32, chunk_size: i32, source_type: WfcSourceType) -> Self {
        Self {
            map: Map::new(MAP_WIDTH, MAP_HEIGHT, depth),
            starting_position: (MAP_WIDTH as i32 / 2, MAP_HEIGHT as i32 / 2),
            depth,
            history: Vec::new(),
            spawn_regions: Vec::new(),
            chunk_size,
            source_type,
        }
    }

    pub fn cellular_automata(depth: i32) -> Self {
        Self::with_options(depth, DEFAULT_CHUNK_SIZE, WfcSourceType::CellularAutomata)
    }

    pub fn bsp_dungeon(depth: i32) -> Self {
        Self::with_options(depth, DEFAULT_CHUNK_SIZE, WfcSourceType::BspDungeon)
    }

    pub fn bsp_interior(depth: i32) -> Self {
        Self::with_options(depth, DEFAULT_CHUNK_SIZE, WfcSourceType::BspInterior)
    }

    pub fn dla(depth: i32) -> Self {
        Self::with_options(depth, DEFAULT_CHUNK_SIZE, WfcSourceType::Dla)
    }

    /// Generate source map using another builder
    fn generate_source_map(&self, rng: &mut GameRng) -> Map {
        let mut source_builder: Box<dyn MapBuilder> = match self.source_type {
            WfcSourceType::CellularAutomata => {
                Box::new(super::CellularAutomataBuilder::new(self.depth))
            }
            WfcSourceType::BspDungeon => Box::new(super::BspDungeonBuilder::new(self.depth)),
            WfcSourceType::BspInterior => Box::new(super::BspInteriorBuilder::new(self.depth)),
            WfcSourceType::Dla => Box::new(super::DLABuilder::walk_inwards(self.depth)),
        };

        source_builder.build_map(rng);
        source_builder.get_map()
    }

    /// Attempt WFC generation with retries
    fn attempt_wfc(&mut self, rng: &mut GameRng) -> bool {
        let source = self.generate_source_map(rng);

        // Snapshot the source map
        self.map = source.clone();
        self.take_snapshot();

        for _attempt in 0..MAX_RETRIES {
            // Reset map to walls
            self.map = Map::new(MAP_WIDTH, MAP_HEIGHT, self.depth);

            let mut solver = WfcSolver::new(&source, self.chunk_size);

            if solver.solve(rng, &mut self.map, &mut self.history) {
                solver.render_to_map(&mut self.map);
                self.take_snapshot();
                return true;
            }
        }

        false
    }

    /// Ensure map has solid border walls
    fn apply_border_walls(&mut self) {
        for x in 0..MAP_WIDTH as i32 {
            let idx_top = self.map.xy_idx(x, 0);
            let idx_bottom = self.map.xy_idx(x, MAP_HEIGHT as i32 - 1);
            self.map.tiles[idx_top] = TileType::Wall;
            self.map.tiles[idx_bottom] = TileType::Wall;
        }
        for y in 0..MAP_HEIGHT as i32 {
            let idx_left = self.map.xy_idx(0, y);
            let idx_right = self.map.xy_idx(MAP_WIDTH as i32 - 1, y);
            self.map.tiles[idx_left] = TileType::Wall;
            self.map.tiles[idx_right] = TileType::Wall;
        }
    }

    /// Find a valid starting position
    fn find_starting_position(&mut self) {
        let mut start_x = MAP_WIDTH as i32 / 2;
        let start_y = MAP_HEIGHT as i32 / 2;

        // Search left from center until we find a floor
        while start_x > 1 {
            let idx = self.map.xy_idx(start_x, start_y);
            if self.map.tiles[idx] == TileType::Floor {
                break;
            }
            start_x -= 1;
        }

        // If still no floor, search the whole map
        if self.map.tiles[self.map.xy_idx(start_x, start_y)] != TileType::Floor {
            for y in 1..MAP_HEIGHT as i32 - 1 {
                for x in 1..MAP_WIDTH as i32 - 1 {
                    if self.map.tiles[self.map.xy_idx(x, y)] == TileType::Floor {
                        self.starting_position = (x, y);
                        return;
                    }
                }
            }
        }

        self.starting_position = (start_x, start_y);
    }

    /// Cull unreachable areas and place stairs
    fn finalize_map(&mut self) {
        let start_idx = self.map.xy_idx(self.starting_position.0, self.starting_position.1);
        let dijkstra = dijkstra_map(&self.map, &[start_idx]);

        let mut exit_idx = start_idx;
        let mut max_distance = 0.0f32;

        for (idx, &dist) in dijkstra.iter().enumerate() {
            if dist < f32::MAX {
                if dist > max_distance {
                    max_distance = dist;
                    exit_idx = idx;
                }
            } else if self.map.tiles[idx] == TileType::Floor {
                // Unreachable floor - convert to wall
                self.map.tiles[idx] = TileType::Wall;
            }
        }

        self.map.tiles[exit_idx] = TileType::DownStairs;

        // Create spawn regions (4x4 grid sections)
        let section_width = MAP_WIDTH / 4;
        let section_height = MAP_HEIGHT / 4;

        for sy in 0..4 {
            for sx in 0..4 {
                let mut region_tiles = Vec::new();
                for y in (sy * section_height)..((sy + 1) * section_height) {
                    for x in (sx * section_width)..((sx + 1) * section_width) {
                        let idx = self.map.xy_idx(x as i32, y as i32);
                        if self.map.tiles[idx] == TileType::Floor
                            && dijkstra[idx] < f32::MAX
                            && idx != start_idx
                        {
                            region_tiles.push(idx);
                        }
                    }
                }
                if !region_tiles.is_empty() {
                    self.spawn_regions.push(region_tiles);
                }
            }
        }
    }
}

impl MapBuilder for WfcBuilder {
    fn build_map(&mut self, rng: &mut GameRng) {
        self.take_snapshot();

        if !self.attempt_wfc(rng) {
            // Fallback: use source map directly
            self.map = self.generate_source_map(rng);
        }

        self.apply_border_walls();
        self.take_snapshot();

        self.find_starting_position();
        self.finalize_map();
        self.take_snapshot();
    }

    fn spawn_entities(&self, commands: &mut Commands, rng: &mut GameRng, font: &TextFont) {
        let mut monster_id: usize = 0;
        for region in &self.spawn_regions {
            spawner::spawn_region(commands, rng, font, region, &mut monster_id, self.depth);
        }
    }

    fn get_map(&self) -> Map {
        self.map.clone()
    }

    fn get_starting_position(&self) -> (i32, i32) {
        self.starting_position
    }

    fn get_snapshot_history(&self) -> Vec<Map> {
        self.history.clone()
    }

    fn take_snapshot(&mut self) {
        self.history.push(self.map.clone());
    }

    fn get_spawn_regions(&self) -> Vec<Rect> {
        Vec::new()
    }

    fn get_name(&self) -> &'static str {
        match self.source_type {
            WfcSourceType::CellularAutomata => "WFC (Cellular Automata)",
            WfcSourceType::BspDungeon => "WFC (BSP Dungeon)",
            WfcSourceType::BspInterior => "WFC (BSP Interior)",
            WfcSourceType::Dla => "WFC (DLA)",
        }
    }
}
