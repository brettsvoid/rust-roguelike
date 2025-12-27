use bevy::prelude::*;
use rand::Rng;

use crate::map::{Map, TileType, MAP_HEIGHT, MAP_WIDTH};
use crate::pathfinding::dijkstra_map;
use crate::rng::GameRng;
use crate::shapes::Rect;
use crate::spawner;

use super::{BuilderMap, InitialMapBuilder, MapBuilder};

pub struct CellularAutomataBuilder {
    map: Map,
    starting_position: (i32, i32),
    depth: i32,
    history: Vec<Map>,
    spawn_regions: Vec<Vec<usize>>,
}

impl CellularAutomataBuilder {
    pub fn new(depth: i32) -> Self {
        Self {
            map: Map::new(MAP_WIDTH, MAP_HEIGHT, depth),
            starting_position: (MAP_WIDTH as i32 / 2, MAP_HEIGHT as i32 / 2),
            depth,
            history: Vec::new(),
            spawn_regions: Vec::new(),
        }
    }

    fn count_wall_neighbors(&self, x: i32, y: i32) -> usize {
        let mut count = 0;
        for dy in -1..=1i32 {
            for dx in -1..=1i32 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let nx = x + dx;
                let ny = y + dy;
                if nx < 0 || nx >= MAP_WIDTH as i32 || ny < 0 || ny >= MAP_HEIGHT as i32 {
                    count += 1; // Treat out-of-bounds as walls
                } else {
                    let idx = self.map.xy_idx(nx, ny);
                    if self.map.tiles[idx] == TileType::Wall {
                        count += 1;
                    }
                }
            }
        }
        count
    }
}

impl MapBuilder for CellularAutomataBuilder {
    fn build_map(&mut self, rng: &mut GameRng) {
        self.take_snapshot();

        // Step 1: Random fill - 55% floor, 45% wall
        for y in 1..MAP_HEIGHT as i32 - 1 {
            for x in 1..MAP_WIDTH as i32 - 1 {
                let roll = rng.0.gen_range(0..100);
                let idx = self.map.xy_idx(x, y);
                if roll > 55 {
                    self.map.tiles[idx] = TileType::Wall;
                } else {
                    self.map.tiles[idx] = TileType::Floor;
                }
            }
        }
        self.take_snapshot();

        // Step 2: Cellular automata iterations
        for _ in 0..15 {
            let mut new_tiles = self.map.tiles.clone();

            for y in 1..MAP_HEIGHT as i32 - 1 {
                for x in 1..MAP_WIDTH as i32 - 1 {
                    let idx = self.map.xy_idx(x, y);
                    let neighbors = self.count_wall_neighbors(x, y);

                    if neighbors > 4 || neighbors == 0 {
                        new_tiles[idx] = TileType::Wall;
                    } else {
                        new_tiles[idx] = TileType::Floor;
                    }
                }
            }

            self.map.tiles = new_tiles;
            self.take_snapshot();
        }

        // Step 3: Find starting position - start at center, move left until floor
        let mut start_x = MAP_WIDTH as i32 / 2;
        let start_y = MAP_HEIGHT as i32 / 2;
        let mut start_idx = self.map.xy_idx(start_x, start_y);

        while start_x > 1 && self.map.tiles[start_idx] != TileType::Floor {
            start_x -= 1;
            start_idx = self.map.xy_idx(start_x, start_y);
        }

        self.starting_position = (start_x, start_y);

        // Step 4: Use Dijkstra to find reachable tiles and cull unreachable
        let dijkstra = dijkstra_map(&self.map, &[start_idx]);

        // Find the furthest reachable tile for stairs
        let mut exit_idx = 0;
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

        self.take_snapshot();

        // Step 5: Place stairs at furthest point
        self.map.tiles[exit_idx] = TileType::DownStairs;
        self.take_snapshot();

        // Step 6: Create spawn regions by dividing reachable floors
        // Simple approach: divide the map into grid sections and collect floor tiles
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
                            // Don't spawn at player start
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
        // For cellular automata, we don't have rectangular rooms
        // Return empty - spawning is handled via spawn_regions internally
        Vec::new()
    }

    fn get_name(&self) -> &'static str {
        "Cellular Automata"
    }
}

// ============================================================================
// New InitialMapBuilder trait implementation
// ============================================================================

fn count_wall_neighbors(map: &Map, x: i32, y: i32) -> usize {
    let mut count = 0;
    for dy in -1..=1i32 {
        for dx in -1..=1i32 {
            if dx == 0 && dy == 0 {
                continue;
            }
            let nx = x + dx;
            let ny = y + dy;
            if nx < 0 || nx >= MAP_WIDTH as i32 || ny < 0 || ny >= MAP_HEIGHT as i32 {
                count += 1; // Treat out-of-bounds as walls
            } else {
                let idx = map.xy_idx(nx, ny);
                if map.tiles[idx] == TileType::Wall {
                    count += 1;
                }
            }
        }
    }
    count
}

impl InitialMapBuilder for CellularAutomataBuilder {
    fn build_map(&mut self, rng: &mut GameRng, build_data: &mut BuilderMap) {
        build_data.take_snapshot();

        // Step 1: Random fill - 55% floor, 45% wall
        for y in 1..MAP_HEIGHT as i32 - 1 {
            for x in 1..MAP_WIDTH as i32 - 1 {
                let roll = rng.0.gen_range(0..100);
                let idx = build_data.map.xy_idx(x, y);
                if roll > 55 {
                    build_data.map.tiles[idx] = TileType::Wall;
                } else {
                    build_data.map.tiles[idx] = TileType::Floor;
                }
            }
        }
        build_data.take_snapshot();

        // Step 2: Cellular automata iterations
        for _ in 0..15 {
            let mut new_tiles = build_data.map.tiles.clone();

            for y in 1..MAP_HEIGHT as i32 - 1 {
                for x in 1..MAP_WIDTH as i32 - 1 {
                    let idx = build_data.map.xy_idx(x, y);
                    let neighbors = count_wall_neighbors(&build_data.map, x, y);

                    if neighbors > 4 || neighbors == 0 {
                        new_tiles[idx] = TileType::Wall;
                    } else {
                        new_tiles[idx] = TileType::Floor;
                    }
                }
            }

            build_data.map.tiles = new_tiles;
            build_data.take_snapshot();
        }

        // Step 3: Find starting position - start at center, move left until floor
        let mut start_x = MAP_WIDTH as i32 / 2;
        let start_y = MAP_HEIGHT as i32 / 2;
        let mut start_idx = build_data.map.xy_idx(start_x, start_y);

        while start_x > 1 && build_data.map.tiles[start_idx] != TileType::Floor {
            start_x -= 1;
            start_idx = build_data.map.xy_idx(start_x, start_y);
        }

        build_data.starting_position = Some((start_x, start_y));

        // Step 4: Use Dijkstra to find reachable tiles and cull unreachable
        let dijkstra = dijkstra_map(&build_data.map, &[start_idx]);

        // Find the furthest reachable tile for stairs
        let mut exit_idx = 0;
        let mut max_distance = 0.0f32;

        for (idx, &dist) in dijkstra.iter().enumerate() {
            if dist < f32::MAX {
                if dist > max_distance {
                    max_distance = dist;
                    exit_idx = idx;
                }
            } else if build_data.map.tiles[idx] == TileType::Floor {
                // Unreachable floor - convert to wall
                build_data.map.tiles[idx] = TileType::Wall;
            }
        }

        build_data.take_snapshot();

        // Step 5: Place stairs at furthest point
        build_data.map.tiles[exit_idx] = TileType::DownStairs;
        build_data.take_snapshot();
    }
}
