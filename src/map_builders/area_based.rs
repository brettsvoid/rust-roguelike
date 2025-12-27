use crate::map::{TileType, MAP_HEIGHT, MAP_WIDTH};
use crate::pathfinding::dijkstra_map;
use crate::rng::GameRng;

use super::{BuilderMap, MetaMapBuilder};

// ============================================================================
// Area-Based Starting Position
// ============================================================================

pub enum XStart {
    Left,
    Center,
    Right,
}

pub enum YStart {
    Top,
    Center,
    Bottom,
}

pub struct AreaStartingPosition {
    x: XStart,
    y: YStart,
}

impl AreaStartingPosition {
    pub fn new(x: XStart, y: YStart) -> Box<Self> {
        Box::new(Self { x, y })
    }
}

impl MetaMapBuilder for AreaStartingPosition {
    fn build_map(&mut self, _rng: &mut GameRng, build_data: &mut BuilderMap) {
        let seed_x = match self.x {
            XStart::Left => 1,
            XStart::Center => MAP_WIDTH as i32 / 2,
            XStart::Right => MAP_WIDTH as i32 - 2,
        };
        let seed_y = match self.y {
            YStart::Top => 1,
            YStart::Center => MAP_HEIGHT as i32 / 2,
            YStart::Bottom => MAP_HEIGHT as i32 - 2,
        };

        // Find nearest floor tile to seed position
        let mut available = Vec::new();
        for (idx, tile) in build_data.map.tiles.iter().enumerate() {
            if *tile == TileType::Floor {
                available.push(idx);
            }
        }

        if available.is_empty() {
            build_data.starting_position = Some((seed_x, seed_y));
            return;
        }

        // Find closest floor tile to seed
        let seed_idx = build_data.map.xy_idx(seed_x, seed_y);
        let mut closest_idx = available[0];
        let mut closest_dist = i32::MAX;

        for idx in available {
            let x = (idx % MAP_WIDTH) as i32;
            let y = (idx / MAP_WIDTH) as i32;
            let dist = (x - seed_x).abs() + (y - seed_y).abs();
            if dist < closest_dist {
                closest_dist = dist;
                closest_idx = idx;
            }
        }

        let x = (closest_idx % MAP_WIDTH) as i32;
        let y = (closest_idx / MAP_WIDTH) as i32;
        build_data.starting_position = Some((x, y));
    }
}

// ============================================================================
// Cull Unreachable Areas
// ============================================================================

pub struct CullUnreachable;

impl CullUnreachable {
    pub fn new() -> Box<Self> {
        Box::new(Self)
    }
}

impl MetaMapBuilder for CullUnreachable {
    fn build_map(&mut self, _rng: &mut GameRng, build_data: &mut BuilderMap) {
        let start_pos = build_data
            .starting_position
            .unwrap_or((MAP_WIDTH as i32 / 2, MAP_HEIGHT as i32 / 2));
        let start_idx = build_data.map.xy_idx(start_pos.0, start_pos.1);

        let dijkstra = dijkstra_map(&build_data.map, &[start_idx]);

        for (idx, &dist) in dijkstra.iter().enumerate() {
            if dist == f32::MAX && build_data.map.tiles[idx] == TileType::Floor {
                build_data.map.tiles[idx] = TileType::Wall;
            }
        }

        build_data.take_snapshot();
    }
}

// ============================================================================
// Distant Exit (place stairs at furthest point from start)
// ============================================================================

pub struct DistantExit;

impl DistantExit {
    pub fn new() -> Box<Self> {
        Box::new(Self)
    }
}

impl MetaMapBuilder for DistantExit {
    fn build_map(&mut self, _rng: &mut GameRng, build_data: &mut BuilderMap) {
        let start_pos = build_data
            .starting_position
            .unwrap_or((MAP_WIDTH as i32 / 2, MAP_HEIGHT as i32 / 2));
        let start_idx = build_data.map.xy_idx(start_pos.0, start_pos.1);

        let dijkstra = dijkstra_map(&build_data.map, &[start_idx]);

        let mut exit_idx = 0;
        let mut max_distance = 0.0f32;

        for (idx, &dist) in dijkstra.iter().enumerate() {
            if dist < f32::MAX && dist > max_distance {
                max_distance = dist;
                exit_idx = idx;
            }
        }

        if max_distance > 0.0 {
            build_data.map.tiles[exit_idx] = TileType::DownStairs;
        }

        build_data.take_snapshot();
    }
}

// ============================================================================
// Voronoi Spawning (spawn entities across the map using grid sections)
// ============================================================================

use rand::Rng;

pub struct VoronoiSpawning;

impl VoronoiSpawning {
    pub fn new() -> Box<Self> {
        Box::new(Self)
    }
}

impl MetaMapBuilder for VoronoiSpawning {
    fn build_map(&mut self, rng: &mut GameRng, build_data: &mut BuilderMap) {
        let start_pos = build_data
            .starting_position
            .unwrap_or((MAP_WIDTH as i32 / 2, MAP_HEIGHT as i32 / 2));
        let start_idx = build_data.map.xy_idx(start_pos.0, start_pos.1);

        let dijkstra = dijkstra_map(&build_data.map, &[start_idx]);

        // Divide map into 4x4 grid sections
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
                        let idx = build_data.map.xy_idx(x as i32, y as i32);
                        if build_data.map.tiles[idx] == TileType::Floor
                            && dijkstra[idx] < f32::MAX
                            && idx != start_idx
                        {
                            region_tiles.push(idx);
                        }
                    }
                }

                // Spawn 0-2 entities in this region
                if !region_tiles.is_empty() {
                    let num_spawns = rng.0.gen_range(0..=2);
                    for _ in 0..num_spawns {
                        let spawn_idx = region_tiles[rng.0.gen_range(0..region_tiles.len())];
                        let roll = rng.0.gen_range(0..100);
                        let name = if roll < 10 {
                            "Health Potion"
                        } else if roll < 20 {
                            "Magic Missile Scroll"
                        } else if roll < 60 {
                            "Goblin"
                        } else {
                            "Orc"
                        };
                        build_data.spawn_list.push((spawn_idx, name.to_string()));
                    }
                }
            }
        }
    }
}
