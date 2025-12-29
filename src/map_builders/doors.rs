use rand::Rng;

use crate::map::{TileType, MAP_HEIGHT, MAP_WIDTH};
use crate::rng::GameRng;

use super::{BuilderMap, MetaMapBuilder};

// ============================================================================
// DoorPlacement - Place doors at corridor entrances
// ============================================================================

pub struct DoorPlacement;

impl DoorPlacement {
    pub fn new() -> Box<Self> {
        Box::new(Self)
    }

    /// Check if a door can be placed at this position
    /// Valid positions have walls on opposite sides and floors on the other two sides
    fn door_possible(build_data: &BuilderMap, idx: usize) -> bool {
        let x = (idx % MAP_WIDTH) as i32;
        let y = (idx / MAP_WIDTH) as i32;

        // Must be on a floor tile
        if build_data.map.tiles[idx] != TileType::Floor {
            return false;
        }

        // Don't place doors at map edges
        if x < 1 || x >= MAP_WIDTH as i32 - 1 || y < 1 || y >= MAP_HEIGHT as i32 - 1 {
            return false;
        }

        // Get neighbor tile types
        let north_idx = build_data.map.xy_idx(x, y - 1);
        let south_idx = build_data.map.xy_idx(x, y + 1);
        let east_idx = build_data.map.xy_idx(x + 1, y);
        let west_idx = build_data.map.xy_idx(x - 1, y);

        let north = build_data.map.tiles[north_idx];
        let south = build_data.map.tiles[south_idx];
        let east = build_data.map.tiles[east_idx];
        let west = build_data.map.tiles[west_idx];

        // Check for horizontal doorway (walls N+S, floors E+W)
        let horizontal_door = north == TileType::Wall
            && south == TileType::Wall
            && east == TileType::Floor
            && west == TileType::Floor;

        // Check for vertical doorway (walls E+W, floors N+S)
        let vertical_door = east == TileType::Wall
            && west == TileType::Wall
            && north == TileType::Floor
            && south == TileType::Floor;

        horizontal_door || vertical_door
    }
}

impl MetaMapBuilder for DoorPlacement {
    fn build_map(&mut self, rng: &mut GameRng, build_data: &mut BuilderMap) {
        let mut doors_placed: Vec<usize> = Vec::new();

        // If we have corridor data, place at most one door per corridor
        if let Some(corridors) = &build_data.corridors {
            for corridor in corridors.iter() {
                // Find first valid door position in this corridor
                for idx in corridor.iter() {
                    if Self::door_possible(build_data, *idx) && !doors_placed.contains(idx) {
                        build_data.spawn_list.push((*idx, "Door".to_string()));
                        doors_placed.push(*idx);
                        break; // Only one door per corridor
                    }
                }
            }
        }

        // If no doors placed from corridors, scan the whole map sparingly
        if doors_placed.is_empty() {
            for y in 1..MAP_HEIGHT as i32 - 1 {
                for x in 1..MAP_WIDTH as i32 - 1 {
                    let idx = build_data.map.xy_idx(x, y);
                    if Self::door_possible(build_data, idx) && !doors_placed.contains(&idx) {
                        // 25% chance to place a door when scanning whole map
                        if rng.0.gen_range(0..4) == 0 {
                            build_data.spawn_list.push((idx, "Door".to_string()));
                            doors_placed.push(idx);
                        }
                    }
                }
            }
        }

        build_data.take_snapshot();
    }
}
