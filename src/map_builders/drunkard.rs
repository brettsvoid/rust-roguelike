use bevy::prelude::*;
use rand::Rng;

use crate::map::{Map, TileType, MAP_HEIGHT, MAP_WIDTH};
use crate::pathfinding::dijkstra_map;
use crate::rng::GameRng;
use crate::shapes::Rect;
use crate::spawner;

use super::MapBuilder;

#[derive(Clone, Copy)]
pub enum DrunkSpawnMode {
    StartingPoint,
    Random,
}

#[derive(Clone, Copy)]
pub struct DrunkardSettings {
    pub spawn_mode: DrunkSpawnMode,
    pub lifetime: i32,
    pub floor_percent: f32,
}

pub struct DrunkardsWalkBuilder {
    map: Map,
    starting_position: (i32, i32),
    depth: i32,
    history: Vec<Map>,
    spawn_regions: Vec<Vec<usize>>,
    settings: DrunkardSettings,
}

impl DrunkardsWalkBuilder {
    fn new(depth: i32, settings: DrunkardSettings) -> Self {
        Self {
            map: Map::new(MAP_WIDTH, MAP_HEIGHT, depth),
            starting_position: (MAP_WIDTH as i32 / 2, MAP_HEIGHT as i32 / 2),
            depth,
            history: Vec::new(),
            spawn_regions: Vec::new(),
            settings,
        }
    }

    /// Large open caves - long-lived drunkards from center
    pub fn open_area(depth: i32) -> Self {
        Self::new(
            depth,
            DrunkardSettings {
                spawn_mode: DrunkSpawnMode::StartingPoint,
                lifetime: 400,
                floor_percent: 0.5,
            },
        )
    }

    /// Sprawling caverns - long-lived drunkards from random locations
    pub fn open_halls(depth: i32) -> Self {
        Self::new(
            depth,
            DrunkardSettings {
                spawn_mode: DrunkSpawnMode::Random,
                lifetime: 400,
                floor_percent: 0.5,
            },
        )
    }

    /// Cramped winding tunnels - short-lived drunkards from random locations
    pub fn winding_passages(depth: i32) -> Self {
        Self::new(
            depth,
            DrunkardSettings {
                spawn_mode: DrunkSpawnMode::Random,
                lifetime: 100,
                floor_percent: 0.4,
            },
        )
    }

    fn count_floors(&self) -> usize {
        self.map
            .tiles
            .iter()
            .filter(|t| **t == TileType::Floor)
            .count()
    }

    fn random_floor_tile(&self, rng: &mut GameRng) -> (i32, i32) {
        loop {
            let x = rng.0.gen_range(1..MAP_WIDTH as i32 - 1);
            let y = rng.0.gen_range(1..MAP_HEIGHT as i32 - 1);
            let idx = self.map.xy_idx(x, y);
            if self.map.tiles[idx] == TileType::Floor {
                return (x, y);
            }
        }
    }
}

impl MapBuilder for DrunkardsWalkBuilder {
    fn build_map(&mut self, rng: &mut GameRng) {
        self.take_snapshot();

        // Start with floor at center
        let start_idx = self.map.xy_idx(self.starting_position.0, self.starting_position.1);
        self.map.tiles[start_idx] = TileType::Floor;

        let total_tiles = (MAP_WIDTH * MAP_HEIGHT) as f32;
        let target_floor = (total_tiles * self.settings.floor_percent) as usize;

        let mut iterations = 0;
        while self.count_floors() < target_floor && iterations < 10000 {
            // Determine spawn position
            let (mut x, mut y) = match self.settings.spawn_mode {
                DrunkSpawnMode::StartingPoint => self.starting_position,
                DrunkSpawnMode::Random => {
                    if self.count_floors() == 1 {
                        self.starting_position
                    } else {
                        self.random_floor_tile(rng)
                    }
                }
            };

            // Drunkard walks for its lifetime
            for _ in 0..self.settings.lifetime {
                let direction = rng.0.gen_range(0..4);
                match direction {
                    0 => {
                        if y > 1 {
                            y -= 1;
                        }
                    }
                    1 => {
                        if y < MAP_HEIGHT as i32 - 2 {
                            y += 1;
                        }
                    }
                    2 => {
                        if x > 1 {
                            x -= 1;
                        }
                    }
                    _ => {
                        if x < MAP_WIDTH as i32 - 2 {
                            x += 1;
                        }
                    }
                }
                let idx = self.map.xy_idx(x, y);
                self.map.tiles[idx] = TileType::Floor;
            }

            iterations += 1;
            if iterations % 10 == 0 {
                self.take_snapshot();
            }
        }

        self.take_snapshot();

        // Use Dijkstra to find reachable tiles and cull unreachable
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

        // Place stairs at furthest point
        self.map.tiles[exit_idx] = TileType::DownStairs;
        self.take_snapshot();

        // Create spawn regions by dividing reachable floors into grid sections
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
        // For drunkard's walk, we don't have rectangular rooms
        Vec::new()
    }

    fn get_name(&self) -> &'static str {
        match self.settings.spawn_mode {
            DrunkSpawnMode::StartingPoint => "Drunkard (Open Area)",
            DrunkSpawnMode::Random => {
                if self.settings.lifetime > 200 {
                    "Drunkard (Open Halls)"
                } else {
                    "Drunkard (Winding)"
                }
            }
        }
    }
}
