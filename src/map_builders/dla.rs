//! Diffusion-limited aggregation (DLA) map builder.
//!
//! Grows the map like a crystal: starting from a small seed of open floor,
//! particles wander (or beeline) across the map and open up the first wall
//! tile they touch next to the existing floor. The open area slowly
//! accretes outward in branching, organic tendrils.
//!
//! Good for: weird, root-system caves with lots of branches. The insectoid
//! preset mirrors the growth for a strangely biological look.

use rand::Rng;

use crate::map::{Map, TileType, MAP_HEIGHT, MAP_WIDTH};
use crate::pathfinding::dijkstra_map;
use crate::rng::GameRng;
use crate::viewshed::bresenham_line;

use super::common::{paint, Symmetry};
use super::{BuilderMap, InitialMapBuilder};

#[derive(Clone, Copy, PartialEq)]
pub enum DLAAlgorithm {
    /// Particles spawn anywhere and wander until they bump into the floor.
    WalkInwards,
    /// Particles start on the floor and wander until they hit a wall.
    WalkOutwards,
    /// Particles spawn anywhere and walk a straight line toward the center.
    CentralAttractor,
}

pub struct DLABuilder {
    algorithm: DLAAlgorithm,
    brush_size: i32,
    symmetry: Symmetry,
    floor_percent: f32,
}

impl DLABuilder {
    fn new(
        algorithm: DLAAlgorithm,
        brush_size: i32,
        symmetry: Symmetry,
        floor_percent: f32,
    ) -> Self {
        Self {
            algorithm,
            brush_size,
            symmetry,
            floor_percent,
        }
    }

    /// Diggers walk from edges toward center
    pub fn walk_inwards() -> Self {
        Self::new(DLAAlgorithm::WalkInwards, 0, Symmetry::None, 0.25)
    }

    /// Diggers walk from center outward
    pub fn walk_outwards() -> Self {
        Self::new(DLAAlgorithm::WalkOutwards, 0, Symmetry::None, 0.25)
    }

    /// Particles path toward center
    pub fn central_attractor() -> Self {
        Self::new(DLAAlgorithm::CentralAttractor, 0, Symmetry::None, 0.25)
    }

    /// Symmetric insectoid pattern
    pub fn insectoid() -> Self {
        Self::new(DLAAlgorithm::CentralAttractor, 0, Symmetry::Horizontal, 0.25)
    }
}

fn dla_count_floors(map: &Map) -> usize {
    map.tiles.iter().filter(|t| **t == TileType::Floor).count()
}

impl InitialMapBuilder for DLABuilder {
    fn build_map(&mut self, rng: &mut GameRng, build_data: &mut BuilderMap) {
        build_data.take_snapshot();

        // Seed the center with a plus/cross pattern (5 tiles)
        let center_x = MAP_WIDTH as i32 / 2;
        let center_y = MAP_HEIGHT as i32 / 2;
        let center_idx = build_data.map.xy_idx(center_x, center_y);
        build_data.map.tiles[center_idx] = TileType::Floor;
        build_data.map.tiles[center_idx - 1] = TileType::Floor;
        build_data.map.tiles[center_idx + 1] = TileType::Floor;
        build_data.map.tiles[center_idx - MAP_WIDTH] = TileType::Floor;
        build_data.map.tiles[center_idx + MAP_WIDTH] = TileType::Floor;

        let total_tiles = (MAP_WIDTH * MAP_HEIGHT) as f32;
        let target_floor = (total_tiles * self.floor_percent) as usize;

        let mut iterations = 0;
        while dla_count_floors(&build_data.map) < target_floor && iterations < 50000 {
            match self.algorithm {
                DLAAlgorithm::WalkInwards => {
                    let mut x = rng.0.gen_range(2..MAP_WIDTH as i32 - 2);
                    let mut y = rng.0.gen_range(2..MAP_HEIGHT as i32 - 2);
                    let mut prev_x = x;
                    let mut prev_y = y;
                    let mut idx = build_data.map.xy_idx(x, y);

                    while build_data.map.tiles[idx] == TileType::Wall {
                        prev_x = x;
                        prev_y = y;
                        match rng.0.gen_range(0..4) {
                            0 => { if x > 2 { x -= 1; } }
                            1 => { if x < MAP_WIDTH as i32 - 2 { x += 1; } }
                            2 => { if y > 2 { y -= 1; } }
                            _ => { if y < MAP_HEIGHT as i32 - 2 { y += 1; } }
                        }
                        idx = build_data.map.xy_idx(x, y);
                    }
                    paint(&mut build_data.map, self.symmetry, self.brush_size, prev_x, prev_y);
                }
                DLAAlgorithm::WalkOutwards => {
                    let mut x = center_x;
                    let mut y = center_y;
                    let mut idx = build_data.map.xy_idx(x, y);

                    while build_data.map.tiles[idx] == TileType::Floor {
                        match rng.0.gen_range(0..4) {
                            0 => { if x > 2 { x -= 1; } }
                            1 => { if x < MAP_WIDTH as i32 - 2 { x += 1; } }
                            2 => { if y > 2 { y -= 1; } }
                            _ => { if y < MAP_HEIGHT as i32 - 2 { y += 1; } }
                        }
                        idx = build_data.map.xy_idx(x, y);
                    }
                    paint(&mut build_data.map, self.symmetry, self.brush_size, x, y);
                }
                DLAAlgorithm::CentralAttractor => {
                    let start_x = rng.0.gen_range(2..MAP_WIDTH as i32 - 2);
                    let start_y = rng.0.gen_range(2..MAP_HEIGHT as i32 - 2);
                    let path = bresenham_line(start_x, start_y, center_x, center_y);

                    let mut prev_x = start_x;
                    let mut prev_y = start_y;

                    for (x, y) in path {
                        let idx = build_data.map.xy_idx(x, y);
                        if build_data.map.tiles[idx] != TileType::Wall {
                            break;
                        }
                        prev_x = x;
                        prev_y = y;
                    }
                    paint(&mut build_data.map, self.symmetry, self.brush_size, prev_x, prev_y);
                }
            }

            iterations += 1;
            if iterations % 10 == 0 {
                build_data.take_snapshot();
            }
        }

        build_data.take_snapshot();

        // Use Dijkstra to find reachable tiles and place stairs
        let dijkstra = dijkstra_map(&build_data.map, &[center_idx]);

        let mut exit_idx = 0;
        let mut max_distance = 0.0f32;

        for (idx, &dist) in dijkstra.iter().enumerate() {
            if dist < f32::MAX {
                if dist > max_distance {
                    max_distance = dist;
                    exit_idx = idx;
                }
            } else if build_data.map.tiles[idx] == TileType::Floor {
                build_data.map.tiles[idx] = TileType::Wall;
            }
        }

        build_data.map.tiles[exit_idx] = TileType::DownStairs;
        build_data.starting_position = Some((center_x, center_y));
        build_data.take_snapshot();
    }
}
