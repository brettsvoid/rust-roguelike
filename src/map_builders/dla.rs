use bevy::prelude::*;
use rand::Rng;

use crate::map::{Map, TileType, MAP_HEIGHT, MAP_WIDTH};
use crate::pathfinding::dijkstra_map;
use crate::rng::GameRng;
use crate::shapes::Rect;
use crate::spawner;
use crate::viewshed::bresenham_line;

use super::MapBuilder;

#[derive(Clone, Copy, PartialEq)]
pub enum DLAAlgorithm {
    WalkInwards,
    WalkOutwards,
    CentralAttractor,
}

#[derive(Clone, Copy, PartialEq)]
pub enum DLASymmetry {
    None,
    Horizontal,
    Vertical,
    Both,
}

pub struct DLABuilder {
    map: Map,
    starting_position: (i32, i32),
    depth: i32,
    history: Vec<Map>,
    spawn_regions: Vec<Vec<usize>>,
    algorithm: DLAAlgorithm,
    brush_size: i32,
    symmetry: DLASymmetry,
    floor_percent: f32,
}

impl DLABuilder {
    fn new(
        depth: i32,
        algorithm: DLAAlgorithm,
        brush_size: i32,
        symmetry: DLASymmetry,
        floor_percent: f32,
    ) -> Self {
        Self {
            map: Map::new(MAP_WIDTH, MAP_HEIGHT, depth),
            starting_position: (MAP_WIDTH as i32 / 2, MAP_HEIGHT as i32 / 2),
            depth,
            history: Vec::new(),
            spawn_regions: Vec::new(),
            algorithm,
            brush_size,
            symmetry,
            floor_percent,
        }
    }

    /// Diggers walk from edges toward center
    pub fn walk_inwards(depth: i32) -> Self {
        Self::new(depth, DLAAlgorithm::WalkInwards, 0, DLASymmetry::None, 0.25)
    }

    /// Diggers walk from center outward
    pub fn walk_outwards(depth: i32) -> Self {
        Self::new(depth, DLAAlgorithm::WalkOutwards, 0, DLASymmetry::None, 0.25)
    }

    /// Particles path toward center
    pub fn central_attractor(depth: i32) -> Self {
        Self::new(
            depth,
            DLAAlgorithm::CentralAttractor,
            0,
            DLASymmetry::None,
            0.25,
        )
    }

    /// Symmetric insectoid pattern
    pub fn insectoid(depth: i32) -> Self {
        Self::new(
            depth,
            DLAAlgorithm::CentralAttractor,
            0,
            DLASymmetry::Horizontal,
            0.25,
        )
    }

    fn count_floors(&self) -> usize {
        self.map
            .tiles
            .iter()
            .filter(|t| **t == TileType::Floor)
            .count()
    }

    fn paint(&mut self, x: i32, y: i32) {
        match self.symmetry {
            DLASymmetry::None => {
                self.apply_paint(x, y);
            }
            DLASymmetry::Horizontal => {
                let center_x = MAP_WIDTH as i32 / 2;
                if x == center_x {
                    self.apply_paint(x, y);
                } else {
                    let dist = (center_x - x).abs();
                    self.apply_paint(center_x + dist, y);
                    self.apply_paint(center_x - dist, y);
                }
            }
            DLASymmetry::Vertical => {
                let center_y = MAP_HEIGHT as i32 / 2;
                if y == center_y {
                    self.apply_paint(x, y);
                } else {
                    let dist = (center_y - y).abs();
                    self.apply_paint(x, center_y + dist);
                    self.apply_paint(x, center_y - dist);
                }
            }
            DLASymmetry::Both => {
                let center_x = MAP_WIDTH as i32 / 2;
                let center_y = MAP_HEIGHT as i32 / 2;
                let dist_x = (center_x - x).abs();
                let dist_y = (center_y - y).abs();
                self.apply_paint(center_x + dist_x, center_y + dist_y);
                self.apply_paint(center_x - dist_x, center_y + dist_y);
                self.apply_paint(center_x + dist_x, center_y - dist_y);
                self.apply_paint(center_x - dist_x, center_y - dist_y);
            }
        }
    }

    fn apply_paint(&mut self, x: i32, y: i32) {
        for dy in -self.brush_size..=self.brush_size {
            for dx in -self.brush_size..=self.brush_size {
                let px = x + dx;
                let py = y + dy;
                if px > 0 && px < MAP_WIDTH as i32 - 1 && py > 0 && py < MAP_HEIGHT as i32 - 1 {
                    let idx = self.map.xy_idx(px, py);
                    self.map.tiles[idx] = TileType::Floor;
                }
            }
        }
    }
}

impl MapBuilder for DLABuilder {
    fn build_map(&mut self, rng: &mut GameRng) {
        self.take_snapshot();

        // Seed the center with a plus/cross pattern (5 tiles)
        let center_x = MAP_WIDTH as i32 / 2;
        let center_y = MAP_HEIGHT as i32 / 2;
        let center_idx = self.map.xy_idx(center_x, center_y);
        self.map.tiles[center_idx] = TileType::Floor;
        self.map.tiles[center_idx - 1] = TileType::Floor;
        self.map.tiles[center_idx + 1] = TileType::Floor;
        self.map.tiles[center_idx - MAP_WIDTH] = TileType::Floor;
        self.map.tiles[center_idx + MAP_WIDTH] = TileType::Floor;
        self.starting_position = (center_x, center_y);

        let total_tiles = (MAP_WIDTH * MAP_HEIGHT) as f32;
        let target_floor = (total_tiles * self.floor_percent) as usize;

        let mut iterations = 0;
        while self.count_floors() < target_floor && iterations < 50000 {
            match self.algorithm {
                DLAAlgorithm::WalkInwards => {
                    // Start at random point anywhere on the map
                    let mut x = rng.0.gen_range(2..MAP_WIDTH as i32 - 2);
                    let mut y = rng.0.gen_range(2..MAP_HEIGHT as i32 - 2);
                    let mut prev_x = x;
                    let mut prev_y = y;
                    let mut idx = self.map.xy_idx(x, y);

                    // Walk while on walls, stop when hitting floor
                    while self.map.tiles[idx] == TileType::Wall {
                        prev_x = x;
                        prev_y = y;

                        // Drunkard walk
                        match rng.0.gen_range(0..4) {
                            0 => {
                                if x > 2 {
                                    x -= 1;
                                }
                            }
                            1 => {
                                if x < MAP_WIDTH as i32 - 2 {
                                    x += 1;
                                }
                            }
                            2 => {
                                if y > 2 {
                                    y -= 1;
                                }
                            }
                            _ => {
                                if y < MAP_HEIGHT as i32 - 2 {
                                    y += 1;
                                }
                            }
                        }
                        idx = self.map.xy_idx(x, y);
                    }
                    self.paint(prev_x, prev_y);
                }
                DLAAlgorithm::WalkOutwards => {
                    // Start at center
                    let mut x = center_x;
                    let mut y = center_y;
                    let mut idx = self.map.xy_idx(x, y);

                    // Walk while on floor, stop when hitting wall
                    while self.map.tiles[idx] == TileType::Floor {
                        // Drunkard walk
                        match rng.0.gen_range(0..4) {
                            0 => {
                                if x > 2 {
                                    x -= 1;
                                }
                            }
                            1 => {
                                if x < MAP_WIDTH as i32 - 2 {
                                    x += 1;
                                }
                            }
                            2 => {
                                if y > 2 {
                                    y -= 1;
                                }
                            }
                            _ => {
                                if y < MAP_HEIGHT as i32 - 2 {
                                    y += 1;
                                }
                            }
                        }
                        idx = self.map.xy_idx(x, y);
                    }
                    self.paint(x, y);
                }
                DLAAlgorithm::CentralAttractor => {
                    // Start at random point
                    let start_x = rng.0.gen_range(2..MAP_WIDTH as i32 - 2);
                    let start_y = rng.0.gen_range(2..MAP_HEIGHT as i32 - 2);

                    // Use Bresenham line to path toward center
                    let path = bresenham_line(start_x, start_y, center_x, center_y);

                    let mut prev_x = start_x;
                    let mut prev_y = start_y;

                    // Walk along line while on walls
                    for (x, y) in path {
                        let idx = self.map.xy_idx(x, y);
                        if self.map.tiles[idx] != TileType::Wall {
                            break;
                        }
                        prev_x = x;
                        prev_y = y;
                    }
                    // Always paint at the last wall position
                    self.paint(prev_x, prev_y);
                }
            }

            iterations += 1;
            if iterations % 10 == 0 {
                self.take_snapshot();
            }
        }

        self.take_snapshot();

        // Use Dijkstra to find reachable tiles and place stairs
        let start_idx = self.map.xy_idx(center_x, center_y);
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
        match self.algorithm {
            DLAAlgorithm::WalkInwards => "DLA (Walk Inwards)",
            DLAAlgorithm::WalkOutwards => "DLA (Walk Outwards)",
            DLAAlgorithm::CentralAttractor => {
                if self.symmetry == DLASymmetry::Horizontal {
                    "DLA (Insectoid)"
                } else {
                    "DLA (Central Attractor)"
                }
            }
        }
    }
}
