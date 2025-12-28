use rand::Rng;

use crate::map::{Map, TileType, MAP_HEIGHT, MAP_WIDTH};
use crate::rng::GameRng;

use super::common::Symmetry;
use super::{BuilderMap, MetaMapBuilder};

// ============================================================================
// CellularAutomataEroder - Single iteration of CA for organic erosion
// ============================================================================

pub struct CellularAutomataEroder {
    iterations: i32,
}

impl CellularAutomataEroder {
    pub fn new() -> Box<Self> {
        Box::new(Self { iterations: 1 })
    }

    pub fn with_iterations(iterations: i32) -> Box<Self> {
        Box::new(Self { iterations })
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
}

impl MetaMapBuilder for CellularAutomataEroder {
    fn build_map(&mut self, _rng: &mut GameRng, build_data: &mut BuilderMap) {
        for _ in 0..self.iterations {
            let mut new_tiles = build_data.map.tiles.clone();

            for y in 1..MAP_HEIGHT as i32 - 1 {
                for x in 1..MAP_WIDTH as i32 - 1 {
                    let idx = build_data.map.xy_idx(x, y);
                    let neighbors = Self::count_wall_neighbors(&build_data.map, x, y);

                    if neighbors > 4 || neighbors == 0 {
                        new_tiles[idx] = TileType::Wall;
                    } else {
                        new_tiles[idx] = TileType::Floor;
                    }
                }
            }

            build_data.map.tiles = new_tiles;
        }
        build_data.take_snapshot();
    }
}

// ============================================================================
// DrunkardsWalkEroder - Drunkard walk erosion on existing map
// ============================================================================

pub struct DrunkardsWalkEroder {
    /// Target percentage of floor tiles
    floor_percent: f32,
    /// Brush size for painting
    brush_size: i32,
    /// Symmetry mode
    symmetry: Symmetry,
}

impl DrunkardsWalkEroder {
    pub fn new() -> Box<Self> {
        Box::new(Self {
            floor_percent: 0.4,
            brush_size: 1,
            symmetry: Symmetry::None,
        })
    }

    pub fn light() -> Box<Self> {
        Box::new(Self {
            floor_percent: 0.35,
            brush_size: 1,
            symmetry: Symmetry::None,
        })
    }

    pub fn heavy() -> Box<Self> {
        Box::new(Self {
            floor_percent: 0.5,
            brush_size: 2,
            symmetry: Symmetry::None,
        })
    }

    pub fn symmetric() -> Box<Self> {
        Box::new(Self {
            floor_percent: 0.4,
            brush_size: 1,
            symmetry: Symmetry::Both,
        })
    }

    fn count_floors(map: &Map) -> usize {
        map.tiles.iter().filter(|t| **t == TileType::Floor).count()
    }

    fn paint_tile(map: &mut Map, brush_size: i32, x: i32, y: i32) {
        for dy in -brush_size..=brush_size {
            for dx in -brush_size..=brush_size {
                let px = x + dx;
                let py = y + dy;
                if px > 0 && px < MAP_WIDTH as i32 - 1 && py > 0 && py < MAP_HEIGHT as i32 - 1 {
                    let idx = map.xy_idx(px, py);
                    map.tiles[idx] = TileType::Floor;
                }
            }
        }
    }

    fn paint_with_symmetry(map: &mut Map, symmetry: Symmetry, brush_size: i32, x: i32, y: i32) {
        match symmetry {
            Symmetry::None => {
                Self::paint_tile(map, brush_size, x, y);
            }
            Symmetry::Horizontal => {
                let center_x = MAP_WIDTH as i32 / 2;
                let dist = (center_x - x).abs();
                Self::paint_tile(map, brush_size, center_x + dist, y);
                Self::paint_tile(map, brush_size, center_x - dist, y);
            }
            Symmetry::Vertical => {
                let center_y = MAP_HEIGHT as i32 / 2;
                let dist = (center_y - y).abs();
                Self::paint_tile(map, brush_size, x, center_y + dist);
                Self::paint_tile(map, brush_size, x, center_y - dist);
            }
            Symmetry::Both => {
                let center_x = MAP_WIDTH as i32 / 2;
                let center_y = MAP_HEIGHT as i32 / 2;
                let dist_x = (center_x - x).abs();
                let dist_y = (center_y - y).abs();
                Self::paint_tile(map, brush_size, center_x + dist_x, center_y + dist_y);
                Self::paint_tile(map, brush_size, center_x - dist_x, center_y + dist_y);
                Self::paint_tile(map, brush_size, center_x + dist_x, center_y - dist_y);
                Self::paint_tile(map, brush_size, center_x - dist_x, center_y - dist_y);
            }
        }
    }
}

impl MetaMapBuilder for DrunkardsWalkEroder {
    fn build_map(&mut self, rng: &mut GameRng, build_data: &mut BuilderMap) {
        let total_tiles = (MAP_WIDTH * MAP_HEIGHT) as f32;
        let target_floors = (total_tiles * self.floor_percent) as usize;

        // Find a starting floor tile
        let mut start_x = MAP_WIDTH as i32 / 2;
        let mut start_y = MAP_HEIGHT as i32 / 2;

        // Look for an existing floor tile to start from
        for y in 1..MAP_HEIGHT as i32 - 1 {
            for x in 1..MAP_WIDTH as i32 - 1 {
                let idx = build_data.map.xy_idx(x, y);
                if build_data.map.tiles[idx] == TileType::Floor {
                    start_x = x;
                    start_y = y;
                    break;
                }
            }
        }

        let mut x = start_x;
        let mut y = start_y;

        // Drunkard walk until we reach target floor percentage
        let mut iterations = 0;
        let max_iterations = 100000;

        while Self::count_floors(&build_data.map) < target_floors && iterations < max_iterations {
            // Random walk
            let direction = rng.0.gen_range(0..4);
            match direction {
                0 if y > 1 => y -= 1,
                1 if y < MAP_HEIGHT as i32 - 2 => y += 1,
                2 if x > 1 => x -= 1,
                3 if x < MAP_WIDTH as i32 - 2 => x += 1,
                _ => {}
            }

            Self::paint_with_symmetry(
                &mut build_data.map,
                self.symmetry,
                self.brush_size,
                x,
                y,
            );

            iterations += 1;
        }

        build_data.take_snapshot();
    }
}
