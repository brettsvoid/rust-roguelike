use bevy::prelude::*;
use rand::Rng;

use crate::map::{Map, TileType, MAP_HEIGHT, MAP_WIDTH};
use crate::pathfinding::dijkstra_map;
use crate::rng::GameRng;
use crate::shapes::Rect;
use crate::spawner;

use super::{BuilderMap, InitialMapBuilder, MapBuilder};

const TOP: usize = 0;
const RIGHT: usize = 1;
const BOTTOM: usize = 2;
const LEFT: usize = 3;

#[derive(Clone)]
struct Cell {
    row: i32,
    column: i32,
    walls: [bool; 4],
    visited: bool,
}

impl Cell {
    fn new(row: i32, column: i32) -> Self {
        Self {
            row,
            column,
            walls: [true, true, true, true],
            visited: false,
        }
    }
}

struct Grid {
    width: i32,
    height: i32,
    cells: Vec<Cell>,
    backtrace: Vec<usize>,
    current: usize,
}

impl Grid {
    fn new(width: i32, height: i32) -> Self {
        let mut cells = Vec::new();
        for row in 0..height {
            for col in 0..width {
                cells.push(Cell::new(row, col));
            }
        }
        Self {
            width,
            height,
            cells,
            backtrace: Vec::new(),
            current: 0,
        }
    }

    fn cell_index(&self, row: i32, column: i32) -> usize {
        (row * self.width + column) as usize
    }

    fn get_available_neighbors(&self) -> Vec<usize> {
        let mut neighbors = Vec::new();
        let current = &self.cells[self.current];
        let row = current.row;
        let col = current.column;

        // Top
        if row > 0 {
            let idx = self.cell_index(row - 1, col);
            if !self.cells[idx].visited {
                neighbors.push(idx);
            }
        }
        // Bottom
        if row < self.height - 1 {
            let idx = self.cell_index(row + 1, col);
            if !self.cells[idx].visited {
                neighbors.push(idx);
            }
        }
        // Left
        if col > 0 {
            let idx = self.cell_index(row, col - 1);
            if !self.cells[idx].visited {
                neighbors.push(idx);
            }
        }
        // Right
        if col < self.width - 1 {
            let idx = self.cell_index(row, col + 1);
            if !self.cells[idx].visited {
                neighbors.push(idx);
            }
        }

        neighbors
    }

    fn remove_walls(&mut self, current_idx: usize, next_idx: usize) {
        let current_row = self.cells[current_idx].row;
        let current_col = self.cells[current_idx].column;
        let next_row = self.cells[next_idx].row;
        let next_col = self.cells[next_idx].column;

        let dx = current_col - next_col;
        let dy = current_row - next_row;

        if dx == 1 {
            // Next is to the left
            self.cells[current_idx].walls[LEFT] = false;
            self.cells[next_idx].walls[RIGHT] = false;
        } else if dx == -1 {
            // Next is to the right
            self.cells[current_idx].walls[RIGHT] = false;
            self.cells[next_idx].walls[LEFT] = false;
        } else if dy == 1 {
            // Next is above
            self.cells[current_idx].walls[TOP] = false;
            self.cells[next_idx].walls[BOTTOM] = false;
        } else if dy == -1 {
            // Next is below
            self.cells[current_idx].walls[BOTTOM] = false;
            self.cells[next_idx].walls[TOP] = false;
        }
    }

    fn copy_to_map(&self, map: &mut Map) {
        for cell in &self.cells {
            let x = cell.column * 2 + 1;
            let y = cell.row * 2 + 1;

            // Cell center is always floor
            let idx = map.xy_idx(x, y);
            if idx < map.tiles.len() {
                map.tiles[idx] = TileType::Floor;
            }

            // Open passages where walls are removed
            if !cell.walls[BOTTOM] && y + 1 < MAP_HEIGHT as i32 {
                let idx = map.xy_idx(x, y + 1);
                map.tiles[idx] = TileType::Floor;
            }
            if !cell.walls[RIGHT] && x + 1 < MAP_WIDTH as i32 {
                let idx = map.xy_idx(x + 1, y);
                map.tiles[idx] = TileType::Floor;
            }
            if !cell.walls[TOP] && y > 0 {
                let idx = map.xy_idx(x, y - 1);
                map.tiles[idx] = TileType::Floor;
            }
            if !cell.walls[LEFT] && x > 0 {
                let idx = map.xy_idx(x - 1, y);
                map.tiles[idx] = TileType::Floor;
            }
        }
    }
}

pub struct MazeBuilder {
    map: Map,
    starting_position: (i32, i32),
    depth: i32,
    history: Vec<Map>,
    spawn_regions: Vec<Vec<usize>>,
}

impl MazeBuilder {
    pub fn new(depth: i32) -> Self {
        Self {
            map: Map::new(MAP_WIDTH, MAP_HEIGHT, depth),
            starting_position: (2, 2),
            depth,
            history: Vec::new(),
            spawn_regions: Vec::new(),
        }
    }
}

impl MapBuilder for MazeBuilder {
    fn build_map(&mut self, rng: &mut GameRng) {
        self.take_snapshot();

        // Grid is half the map size (each cell becomes 2x2 in the map)
        let grid_width = (MAP_WIDTH / 2) as i32 - 1;
        let grid_height = (MAP_HEIGHT / 2) as i32 - 1;

        let mut grid = Grid::new(grid_width, grid_height);

        // Generate the maze
        let mut iteration = 0;
        grid.cells[0].visited = true;
        grid.current = 0;

        loop {
            let neighbors = grid.get_available_neighbors();
            if !neighbors.is_empty() {
                let next = neighbors[rng.0.gen_range(0..neighbors.len())];
                grid.backtrace.push(grid.current);
                grid.remove_walls(grid.current, next);
                grid.current = next;
                grid.cells[next].visited = true;
            } else if !grid.backtrace.is_empty() {
                grid.current = grid.backtrace.pop().unwrap();
            } else {
                break;
            }

            iteration += 1;
            if iteration % 50 == 0 {
                grid.copy_to_map(&mut self.map);
                self.take_snapshot();
            }
        }

        // Final copy
        grid.copy_to_map(&mut self.map);
        self.take_snapshot();

        // Starting position
        self.starting_position = (2, 2);
        let start_idx = self.map.xy_idx(2, 2);

        // Use Dijkstra to find furthest point for stairs
        let dijkstra = dijkstra_map(&self.map, &[start_idx]);

        let mut exit_idx = 0;
        let mut max_distance = 0.0f32;

        for (idx, &dist) in dijkstra.iter().enumerate() {
            if dist < f32::MAX && dist > max_distance {
                max_distance = dist;
                exit_idx = idx;
            }
        }

        self.map.tiles[exit_idx] = TileType::DownStairs;
        self.take_snapshot();

        // Create spawn regions
        let section_width = MAP_WIDTH / 4;
        let section_height = MAP_HEIGHT / 4;

        for sy in 0..4 {
            for sx in 0..4 {
                let mut region_tiles = Vec::new();
                let min_x = sx * section_width;
                let max_x = (sx + 1) * section_width;
                let min_y = sy * section_height;
                let max_y = (sy + 1) * section_height;

                for y in min_y..max_y {
                    for x in min_x..max_x {
                        let idx = self.map.xy_idx(x as i32, y as i32);
                        if self.map.tiles[idx] == TileType::Floor && dijkstra[idx] < f32::MAX {
                            if idx != start_idx {
                                region_tiles.push(idx);
                            }
                        }
                    }
                }

                if !region_tiles.is_empty() {
                    self.spawn_regions.push(region_tiles);
                }
            }
        }
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
        "Maze"
    }
}

// ============================================================================
// New InitialMapBuilder trait implementation
// ============================================================================

impl InitialMapBuilder for MazeBuilder {
    fn build_map(&mut self, rng: &mut GameRng, build_data: &mut BuilderMap) {
        build_data.take_snapshot();

        // Grid is half the map size (each cell becomes 2x2 in the map)
        let grid_width = (MAP_WIDTH / 2) as i32 - 1;
        let grid_height = (MAP_HEIGHT / 2) as i32 - 1;

        let mut grid = Grid::new(grid_width, grid_height);

        // Generate the maze
        let mut iteration = 0;
        grid.cells[0].visited = true;
        grid.current = 0;

        loop {
            let neighbors = grid.get_available_neighbors();
            if !neighbors.is_empty() {
                let next = neighbors[rng.0.gen_range(0..neighbors.len())];
                grid.backtrace.push(grid.current);
                grid.remove_walls(grid.current, next);
                grid.current = next;
                grid.cells[next].visited = true;
            } else if !grid.backtrace.is_empty() {
                grid.current = grid.backtrace.pop().unwrap();
            } else {
                break;
            }

            iteration += 1;
            if iteration % 50 == 0 {
                grid.copy_to_map(&mut build_data.map);
                build_data.take_snapshot();
            }
        }

        // Final copy
        grid.copy_to_map(&mut build_data.map);
        build_data.take_snapshot();

        // Starting position
        build_data.starting_position = Some((2, 2));
        let start_idx = build_data.map.xy_idx(2, 2);

        // Use Dijkstra to find furthest point for stairs
        let dijkstra = dijkstra_map(&build_data.map, &[start_idx]);

        let mut exit_idx = 0;
        let mut max_distance = 0.0f32;

        for (idx, &dist) in dijkstra.iter().enumerate() {
            if dist < f32::MAX && dist > max_distance {
                max_distance = dist;
                exit_idx = idx;
            }
        }

        build_data.map.tiles[exit_idx] = TileType::DownStairs;
        build_data.take_snapshot();
    }
}
