//! Meta builders for maps without rooms (caves and other open "areas").
//!
//! - `AreaStartingPosition`: drop the player on the floor tile nearest a
//!   chosen spot (center, an edge, a corner)
//! - `CullUnreachable`: wall off any floor the player could never reach
//! - `DistantExit`: put the stairs on the farthest walkable tile
//! - `VoronoiSpawning`: sprinkle monsters and items region by region, rolled
//!   from the depth's spawn table
//!
//! Chain these after any initial builder that doesn't handle start/stairs/
//! spawns itself.

use crate::map::{Map, TileType, MAP_HEIGHT, MAP_WIDTH};
use crate::pathfinding::dijkstra_map;
use crate::raws::{get_spawn_table_for_depth, RAWS};
use crate::rng::GameRng;

use super::{BuilderMap, MetaMapBuilder};

// ============================================================================
// Area-Based Starting Position
// ============================================================================

// Toolbox: only Center is used today, the rest are options for new chains.
#[allow(dead_code)]
pub enum XStart {
    Left,
    Center,
    Right,
}

#[allow(dead_code)]
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

        let Some(idx) = start_in_main_area(&build_data.map, (seed_x, seed_y)) else {
            build_data.starting_position = Some((seed_x, seed_y));
            return;
        };
        build_data.starting_position = Some(((idx % MAP_WIDTH) as i32, (idx / MAP_WIDTH) as i32));
    }
}

/// The floor tile nearest `seed` (Manhattan) that lies in the main area: the
/// first connected area, nearest first, holding at least half the floor, else
/// the largest. The nearest tile alone can be a sealed pocket, and
/// `CullUnreachable` would then wall up the whole rest of the level.
fn start_in_main_area(map: &Map, (seed_x, seed_y): (i32, i32)) -> Option<usize> {
    let mut floor: Vec<usize> = (0..map.tiles.len()).filter(|&i| map.tiles[i] == TileType::Floor).collect();
    let distance = |idx: usize| ((idx % MAP_WIDTH) as i32 - seed_x).abs() + ((idx / MAP_WIDTH) as i32 - seed_y).abs();
    floor.sort_by_key(|&idx| (distance(idx), idx));

    let mut seen = vec![false; map.tiles.len()];
    let mut best: Option<(usize, usize)> = None;
    for &idx in &floor {
        if seen[idx] {
            continue;
        }
        let reach = dijkstra_map(map, &[idx]);
        let mut size = 0;
        for (i, &d) in reach.iter().enumerate() {
            if d < f32::MAX && map.tiles[i] == TileType::Floor {
                seen[i] = true;
                size += 1;
            }
        }
        if best.is_none_or(|(_, most)| size > most) {
            best = Some((idx, size));
        }
        if size * 2 >= floor.len() {
            break;
        }
    }
    best.map(|(idx, _)| idx)
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
            // One way down: stairs an earlier builder placed give way.
            for tile in build_data.map.tiles.iter_mut().filter(|t| **t == TileType::DownStairs) {
                *tile = TileType::Floor;
            }
            build_data.map.tiles[exit_idx] = TileType::DownStairs;
        }

        build_data.take_snapshot();
    }
}

// ============================================================================
// Voronoi Spawning (spawn entities across the map using grid sections)
// ============================================================================

use rand::RngExt;

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
        let spawn_table = get_spawn_table_for_depth(&RAWS.lock().unwrap(), build_data.depth);

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
                    let num_spawns = rng.0.random_range(0..=2);
                    for _ in 0..num_spawns {
                        let spawn_idx = region_tiles[rng.0.random_range(0..region_tiles.len())];
                        if let Some(name) = spawn_table.roll(rng) {
                            build_data.spawn_list.push((spawn_idx, name));
                        }
                    }
                }
            }
        }
    }
}
