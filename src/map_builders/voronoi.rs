//! Voronoi cell map builder.
//!
//! Scatters random seed points across the map and assigns every tile to its
//! nearest seed, like soap bubbles growing until they touch. The borders
//! between bubbles become walls, the insides become floor, and corridors
//! link each region to its two nearest neighbors.
//!
//! The distance formula changes the bubble shape: Euclidean gives round
//! cells, Manhattan diamond-ish cells, Chebyshev square-ish cells.
//!
//! Good for: cellular, honeycomb-like cave networks of connected chambers.

use bevy::prelude::*;
use rand::Rng;

use crate::distance::DistanceAlg;
use crate::map::{TileType, MAP_HEIGHT, MAP_WIDTH};
use crate::pathfinding::dijkstra_map;
use crate::rng::GameRng;

use super::{BuilderMap, InitialMapBuilder};

pub struct VoronoiCellBuilder {
    n_seeds: usize,
    distance_algorithm: DistanceAlg,
}

impl VoronoiCellBuilder {
    fn new(n_seeds: usize, distance_algorithm: DistanceAlg) -> Self {
        Self {
            n_seeds,
            distance_algorithm,
        }
    }

    pub fn euclidean() -> Self {
        Self::new(64, DistanceAlg::Euclidean)
    }

    pub fn manhattan() -> Self {
        Self::new(64, DistanceAlg::Manhattan)
    }

    pub fn chebyshev() -> Self {
        Self::new(64, DistanceAlg::Chebyshev)
    }
}

impl InitialMapBuilder for VoronoiCellBuilder {
    fn build_map(&mut self, rng: &mut GameRng, build_data: &mut BuilderMap) {
        build_data.take_snapshot();

        // 1. Generate unique seed points
        let mut seeds: Vec<(i32, i32)> = Vec::new();
        while seeds.len() < self.n_seeds {
            let x = rng.0.gen_range(1..MAP_WIDTH as i32 - 1);
            let y = rng.0.gen_range(1..MAP_HEIGHT as i32 - 1);
            if !seeds.contains(&(x, y)) {
                seeds.push((x, y));
            }
        }

        // 2. Assign each tile to nearest seed
        let mut memberships: Vec<usize> = vec![0; MAP_WIDTH * MAP_HEIGHT];
        for y in 0..MAP_HEIGHT as i32 {
            for x in 0..MAP_WIDTH as i32 {
                let mut min_dist = f32::MAX;
                let mut closest = 0;
                for (i, seed) in seeds.iter().enumerate() {
                    let dist = self.distance_algorithm.distance2d(
                        Vec2::new(x as f32, y as f32),
                        Vec2::new(seed.0 as f32, seed.1 as f32),
                    );
                    if dist < min_dist {
                        min_dist = dist;
                        closest = i;
                    }
                }
                let idx = build_data.map.xy_idx(x, y);
                memberships[idx] = closest;
            }
        }

        // 3. Place walls at region boundaries, floors in interior
        for y in 1..MAP_HEIGHT as i32 - 1 {
            for x in 1..MAP_WIDTH as i32 - 1 {
                let idx = build_data.map.xy_idx(x, y);
                let my_membership = memberships[idx];

                let neighbors = [
                    build_data.map.xy_idx(x - 1, y - 1),
                    build_data.map.xy_idx(x, y - 1),
                    build_data.map.xy_idx(x + 1, y - 1),
                    build_data.map.xy_idx(x - 1, y),
                    build_data.map.xy_idx(x + 1, y),
                    build_data.map.xy_idx(x - 1, y + 1),
                    build_data.map.xy_idx(x, y + 1),
                    build_data.map.xy_idx(x + 1, y + 1),
                ];

                let mut is_boundary = false;
                for n_idx in neighbors {
                    if memberships[n_idx] != my_membership {
                        is_boundary = true;
                        break;
                    }
                }

                if is_boundary {
                    build_data.map.tiles[idx] = TileType::Wall;
                } else {
                    build_data.map.tiles[idx] = TileType::Floor;
                }
            }
            if y % 5 == 0 {
                build_data.take_snapshot();
            }
        }

        build_data.take_snapshot();

        // 4. Connect cells by drawing corridors between nearby seeds
        for i in 0..seeds.len() {
            let mut distances: Vec<(usize, f32)> = seeds
                .iter()
                .enumerate()
                .filter(|(j, _)| *j != i)
                .map(|(j, seed)| {
                    let dist = self.distance_algorithm.distance2d(
                        Vec2::new(seeds[i].0 as f32, seeds[i].1 as f32),
                        Vec2::new(seed.0 as f32, seed.1 as f32),
                    );
                    (j, dist)
                })
                .collect();
            distances.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

            for (j, _) in distances.iter().take(2) {
                let (x1, y1) = seeds[i];
                let (x2, y2) = seeds[*j];
                let mut x = x1;
                let mut y = y1;
                while x != x2 || y != y2 {
                    if x < x2 { x += 1; }
                    else if x > x2 { x -= 1; }
                    else if y < y2 { y += 1; }
                    else if y > y2 { y -= 1; }
                    let idx = build_data.map.xy_idx(x, y);
                    build_data.map.tiles[idx] = TileType::Floor;
                }
            }
        }

        build_data.take_snapshot();

        // Find a starting position
        let center_idx = build_data.map.xy_idx(MAP_WIDTH as i32 / 2, MAP_HEIGHT as i32 / 2);
        if build_data.map.tiles[center_idx] == TileType::Floor {
            build_data.starting_position = Some((MAP_WIDTH as i32 / 2, MAP_HEIGHT as i32 / 2));
        } else {
            for radius in 1..20 {
                let mut found = false;
                for dy in -radius..=radius {
                    for dx in -radius..=radius {
                        let x = MAP_WIDTH as i32 / 2 + dx;
                        let y = MAP_HEIGHT as i32 / 2 + dy;
                        if x > 0 && x < MAP_WIDTH as i32 - 1 && y > 0 && y < MAP_HEIGHT as i32 - 1 {
                            let idx = build_data.map.xy_idx(x, y);
                            if build_data.map.tiles[idx] == TileType::Floor {
                                build_data.starting_position = Some((x, y));
                                found = true;
                                break;
                            }
                        }
                    }
                    if found { break; }
                }
                if found { break; }
            }
        }

        // Use Dijkstra to find reachable tiles and place stairs
        let start_pos = build_data.starting_position.unwrap_or((MAP_WIDTH as i32 / 2, MAP_HEIGHT as i32 / 2));
        let start_idx = build_data.map.xy_idx(start_pos.0, start_pos.1);
        let dijkstra = dijkstra_map(&build_data.map, &[start_idx]);

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
        build_data.take_snapshot();
    }
}
