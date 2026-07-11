//! Drunkard's walk map builder.
//!
//! Drops a digger onto the map that staggers around at random, carving
//! floor wherever it steps. When one drunkard's lifetime runs out, another
//! is released, until enough of the map is open. The presets below tweak
//! lifetime, spawn point, brush size, and symmetry for different feels.
//!
//! Good for: messy natural caves full of dead ends and odd nooks. Guaranteed
//! fully connected, since every tile was walked to.

use rand::Rng;

use crate::map::{Map, TileType, MAP_HEIGHT, MAP_WIDTH};
use crate::pathfinding::dijkstra_map;
use crate::rng::GameRng;

use super::common::{paint, Symmetry};
use super::{BuilderMap, InitialMapBuilder};

#[derive(Clone, Copy)]
pub enum DrunkSpawnMode {
    /// Every drunkard starts at the map center - digs one big blob.
    StartingPoint,
    /// Drunkards start on random floor tiles - spreads out more.
    Random,
}

#[derive(Clone, Copy)]
pub struct DrunkardSettings {
    pub spawn_mode: DrunkSpawnMode,
    /// How many steps each drunkard takes before passing out.
    pub lifetime: i32,
    /// Stop once this fraction of the map is floor.
    pub floor_percent: f32,
    /// 0 = single tile, bigger = wider tunnels.
    pub brush_size: i32,
    pub symmetry: Symmetry,
}

pub struct DrunkardsWalkBuilder {
    settings: DrunkardSettings,
}

impl DrunkardsWalkBuilder {
    fn new(settings: DrunkardSettings) -> Self {
        Self { settings }
    }

    /// Large open caves - long-lived drunkards from center
    pub fn open_area() -> Self {
        Self::new(DrunkardSettings {
            spawn_mode: DrunkSpawnMode::StartingPoint,
            lifetime: 400,
            floor_percent: 0.5,
            brush_size: 0,
            symmetry: Symmetry::None,
        })
    }

    /// Sprawling caverns - long-lived drunkards from random locations
    pub fn open_halls() -> Self {
        Self::new(DrunkardSettings {
            spawn_mode: DrunkSpawnMode::Random,
            lifetime: 400,
            floor_percent: 0.5,
            brush_size: 0,
            symmetry: Symmetry::None,
        })
    }

    /// Cramped winding tunnels - short-lived drunkards from random locations
    pub fn winding_passages() -> Self {
        Self::new(DrunkardSettings {
            spawn_mode: DrunkSpawnMode::Random,
            lifetime: 100,
            floor_percent: 0.4,
            brush_size: 0,
            symmetry: Symmetry::None,
        })
    }

    /// Wide passages using larger brush
    pub fn fat_passages() -> Self {
        Self::new(DrunkardSettings {
            spawn_mode: DrunkSpawnMode::Random,
            lifetime: 100,
            floor_percent: 0.4,
            brush_size: 1,
            symmetry: Symmetry::None,
        })
    }

    /// Symmetric mirrored pattern
    pub fn fearful_symmetry() -> Self {
        Self::new(DrunkardSettings {
            spawn_mode: DrunkSpawnMode::Random,
            lifetime: 100,
            floor_percent: 0.4,
            brush_size: 0,
            symmetry: Symmetry::Both,
        })
    }
}

fn count_floors(map: &Map) -> usize {
    map.tiles.iter().filter(|t| **t == TileType::Floor).count()
}

fn random_floor_tile(map: &Map, rng: &mut GameRng) -> (i32, i32) {
    loop {
        let x = rng.0.gen_range(1..MAP_WIDTH as i32 - 1);
        let y = rng.0.gen_range(1..MAP_HEIGHT as i32 - 1);
        let idx = map.xy_idx(x, y);
        if map.tiles[idx] == TileType::Floor {
            return (x, y);
        }
    }
}

impl InitialMapBuilder for DrunkardsWalkBuilder {
    fn build_map(&mut self, rng: &mut GameRng, build_data: &mut BuilderMap) {
        build_data.take_snapshot();

        let starting_position = (MAP_WIDTH as i32 / 2, MAP_HEIGHT as i32 / 2);

        // Start with floor at center
        let start_idx = build_data.map.xy_idx(starting_position.0, starting_position.1);
        build_data.map.tiles[start_idx] = TileType::Floor;

        let total_tiles = (MAP_WIDTH * MAP_HEIGHT) as f32;
        let target_floor = (total_tiles * self.settings.floor_percent) as usize;

        let mut iterations = 0;
        while count_floors(&build_data.map) < target_floor && iterations < 10000 {
            // Determine spawn position
            let (mut x, mut y) = match self.settings.spawn_mode {
                DrunkSpawnMode::StartingPoint => starting_position,
                DrunkSpawnMode::Random => {
                    if count_floors(&build_data.map) == 1 {
                        starting_position
                    } else {
                        random_floor_tile(&build_data.map, rng)
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
                paint(
                    &mut build_data.map,
                    self.settings.symmetry,
                    self.settings.brush_size,
                    x,
                    y,
                );
            }

            iterations += 1;
            if iterations % 10 == 0 {
                build_data.take_snapshot();
            }
        }

        build_data.take_snapshot();

        // Use Dijkstra to find reachable tiles and cull unreachable
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

        // Place stairs at furthest point
        build_data.map.tiles[exit_idx] = TileType::DownStairs;
        build_data.starting_position = Some(starting_position);
        build_data.take_snapshot();
    }
}
