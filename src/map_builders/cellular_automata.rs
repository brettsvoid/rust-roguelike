//! Cellular automata map builder.
//!
//! Fills the map with random noise, then smooths it over several passes with
//! one simple rule: a tile becomes wall if most of its neighbors are walls.
//! The noise settles into organic blobs with irregular edges.
//!
//! Good for: natural caves and caverns. Not great when you want distinct
//! rooms or predictable corridors.

use rand::Rng;

use crate::map::{Map, TileType, MAP_HEIGHT, MAP_WIDTH};
use crate::pathfinding::dijkstra_map;
use crate::rng::GameRng;

use super::{BuilderMap, InitialMapBuilder};

#[derive(Default)]
pub struct CellularAutomataBuilder;

impl CellularAutomataBuilder {
    pub fn new() -> Self {
        Self
    }
}

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
