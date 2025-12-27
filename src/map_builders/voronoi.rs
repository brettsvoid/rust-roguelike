use bevy::prelude::*;
use rand::Rng;

use crate::distance::DistanceAlg;
use crate::map::{Map, TileType, MAP_HEIGHT, MAP_WIDTH};
use crate::pathfinding::dijkstra_map;
use crate::rng::GameRng;
use crate::shapes::Rect;
use crate::spawner;

use super::MapBuilder;

pub struct VoronoiCellBuilder {
    map: Map,
    starting_position: (i32, i32),
    depth: i32,
    history: Vec<Map>,
    spawn_regions: Vec<Vec<usize>>,
    n_seeds: usize,
    distance_algorithm: DistanceAlg,
}

impl VoronoiCellBuilder {
    fn new(depth: i32, n_seeds: usize, distance_algorithm: DistanceAlg) -> Self {
        Self {
            map: Map::new(MAP_WIDTH, MAP_HEIGHT, depth),
            starting_position: (MAP_WIDTH as i32 / 2, MAP_HEIGHT as i32 / 2),
            depth,
            history: Vec::new(),
            spawn_regions: Vec::new(),
            n_seeds,
            distance_algorithm,
        }
    }

    pub fn euclidean(depth: i32) -> Self {
        Self::new(depth, 64, DistanceAlg::Euclidean)
    }

    pub fn manhattan(depth: i32) -> Self {
        Self::new(depth, 64, DistanceAlg::Manhattan)
    }

    pub fn chebyshev(depth: i32) -> Self {
        Self::new(depth, 64, DistanceAlg::Chebyshev)
    }
}

impl MapBuilder for VoronoiCellBuilder {
    fn build_map(&mut self, rng: &mut GameRng) {
        self.take_snapshot();

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
                let idx = self.map.xy_idx(x, y);
                memberships[idx] = closest;
            }
        }

        // 3. Place walls at region boundaries, floors in interior
        for y in 1..MAP_HEIGHT as i32 - 1 {
            for x in 1..MAP_WIDTH as i32 - 1 {
                let idx = self.map.xy_idx(x, y);
                let my_membership = memberships[idx];

                // Check all 8 neighbors
                let neighbors = [
                    self.map.xy_idx(x - 1, y - 1),
                    self.map.xy_idx(x, y - 1),
                    self.map.xy_idx(x + 1, y - 1),
                    self.map.xy_idx(x - 1, y),
                    self.map.xy_idx(x + 1, y),
                    self.map.xy_idx(x - 1, y + 1),
                    self.map.xy_idx(x, y + 1),
                    self.map.xy_idx(x + 1, y + 1),
                ];

                // Check if any neighbor is from a different group
                let mut is_boundary = false;
                for n_idx in neighbors {
                    if memberships[n_idx] != my_membership {
                        is_boundary = true;
                        break;
                    }
                }

                if is_boundary {
                    self.map.tiles[idx] = TileType::Wall;
                } else {
                    self.map.tiles[idx] = TileType::Floor;
                }
            }
            // Take snapshot every few rows
            if y % 5 == 0 {
                self.take_snapshot();
            }
        }

        self.take_snapshot();

        // 4. Connect cells by drawing corridors between nearby seeds
        for i in 0..seeds.len() {
            // Find the 2 nearest seeds and connect to them
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

            // Connect to nearest 2 neighbors
            for (j, _) in distances.iter().take(2) {
                let (x1, y1) = seeds[i];
                let (x2, y2) = seeds[*j];
                // Draw corridor
                let mut x = x1;
                let mut y = y1;
                while x != x2 || y != y2 {
                    if x < x2 {
                        x += 1;
                    } else if x > x2 {
                        x -= 1;
                    } else if y < y2 {
                        y += 1;
                    } else if y > y2 {
                        y -= 1;
                    }
                    let idx = self.map.xy_idx(x, y);
                    self.map.tiles[idx] = TileType::Floor;
                }
            }
        }

        self.take_snapshot();

        // Find a starting position (center of map, find nearest floor)
        let center_idx = self.map.xy_idx(MAP_WIDTH as i32 / 2, MAP_HEIGHT as i32 / 2);
        if self.map.tiles[center_idx] == TileType::Floor {
            self.starting_position = (MAP_WIDTH as i32 / 2, MAP_HEIGHT as i32 / 2);
        } else {
            // Find nearest floor tile to center
            for radius in 1..20 {
                let mut found = false;
                for dy in -radius..=radius {
                    for dx in -radius..=radius {
                        let x = MAP_WIDTH as i32 / 2 + dx;
                        let y = MAP_HEIGHT as i32 / 2 + dy;
                        if x > 0 && x < MAP_WIDTH as i32 - 1 && y > 0 && y < MAP_HEIGHT as i32 - 1 {
                            let idx = self.map.xy_idx(x, y);
                            if self.map.tiles[idx] == TileType::Floor {
                                self.starting_position = (x, y);
                                found = true;
                                break;
                            }
                        }
                    }
                    if found {
                        break;
                    }
                }
                if found {
                    break;
                }
            }
        }

        // Use Dijkstra to find reachable tiles and place stairs
        let start_idx = self.map.xy_idx(self.starting_position.0, self.starting_position.1);
        let dijkstra = dijkstra_map(&self.map, &[start_idx]);

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
        match self.distance_algorithm {
            DistanceAlg::Euclidean => "Voronoi (Euclidean)",
            DistanceAlg::Manhattan => "Voronoi (Manhattan)",
            DistanceAlg::Chebyshev => "Voronoi (Chebyshev)",
        }
    }
}
