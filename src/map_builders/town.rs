use std::collections::HashSet;

use rand::Rng;

use crate::map::TileType;
use crate::rng::GameRng;

use super::{BuilderMap, InitialMapBuilder};

pub struct TownBuilder {}

impl TownBuilder {
    pub fn new() -> Self {
        TownBuilder {}
    }
}

impl InitialMapBuilder for TownBuilder {
    fn build_map(&mut self, rng: &mut GameRng, build_data: &mut BuilderMap) {
        self.build_town(rng, build_data);
    }
}

impl TownBuilder {
    fn build_town(&mut self, rng: &mut GameRng, build_data: &mut BuilderMap) {
        // Start with grass everywhere
        self.grass_layer(build_data);
        build_data.take_snapshot();

        // Add water on the left side with a natural coastline
        self.water_and_coastline(rng, build_data);
        build_data.take_snapshot();

        // Add town walls and main road
        let (mut available_building_tiles, wall_gap_y) = self.town_walls(rng, build_data);
        build_data.take_snapshot();

        // Build some buildings
        let mut buildings = self.buildings(rng, build_data, &mut available_building_tiles);
        build_data.take_snapshot();

        if buildings.is_empty() {
            // Fallback if no buildings could be placed
            build_data.starting_position = Some((15, wall_gap_y));
        } else {
            // Find the largest building (the pub) and set starting position
            let building_sizes: Vec<(usize, i32)> = buildings
                .iter()
                .enumerate()
                .map(|(i, b)| (i, b.2 * b.3))
                .collect();
            let pub_building = building_sizes
                .iter()
                .max_by_key(|(_i, size)| *size)
                .map(|(i, _)| *i)
                .unwrap_or(0);

            // Set starting position in center of the pub (interior)
            let pub_rect = buildings[pub_building];
            build_data.starting_position =
                Some((pub_rect.0 + pub_rect.2 / 2, pub_rect.1 + pub_rect.3 / 2));
        }

        // Build doors and get their positions for path generation
        let door_positions = self.add_doors(rng, build_data, &buildings, wall_gap_y);
        build_data.take_snapshot();

        // Create paths from doors to the main road
        self.add_paths(build_data, &door_positions, wall_gap_y);
        build_data.take_snapshot();

        // Place exit at edge of the main road (on land, not water)
        // The road runs from x=12 to x=30, so place exit at x=12 on the road
        let exit_x = 12;
        let exit_y = wall_gap_y;
        let exit_idx = build_data.map.xy_idx(exit_x, exit_y);
        build_data.map.tiles[exit_idx] = TileType::DownStairs;

        // Don't spawn monsters in town - use an empty rooms list
        build_data.rooms = Some(Vec::new());

        build_data.take_snapshot();
    }

    fn grass_layer(&self, build_data: &mut BuilderMap) {
        for tile in build_data.map.tiles.iter_mut() {
            *tile = TileType::Grass;
        }
    }

    fn water_and_coastline(&self, rng: &mut GameRng, build_data: &mut BuilderMap) {
        let map = &mut build_data.map;

        // Use sine wave for natural coastline variation
        for y in 0..map.height {
            // Base coastline at x=10, with sine wave variation
            let wave = (y as f32 * 0.15).sin() * 3.0;
            let coastline = 10 + wave as i32;

            for x in 0..coastline {
                let idx = map.xy_idx(x, y);
                // Shallow water near coast, deep water further out
                if x < coastline - 2 {
                    map.tiles[idx] = TileType::DeepWater;
                } else {
                    map.tiles[idx] = TileType::ShallowWater;
                }
            }
        }

        // Add some piers extending into the water
        let num_piers = rng.0.gen_range(2..=4);
        for _ in 0..num_piers {
            let pier_y = rng.0.gen_range(5..map.height - 5);
            let pier_start = 12; // Start from land
            let pier_end = rng.0.gen_range(2..6);

            for x in pier_end..pier_start {
                let idx = map.xy_idx(x, pier_y);
                map.tiles[idx] = TileType::Bridge;
            }
        }
    }

    fn town_walls(
        &self,
        rng: &mut GameRng,
        build_data: &mut BuilderMap,
    ) -> (HashSet<usize>, i32) {
        let map = &mut build_data.map;
        let mut available_building_tiles = HashSet::new();

        // Wall runs vertically at x=30
        let wall_x = 30;
        let gate_y = rng.0.gen_range(map.height / 4..map.height * 3 / 4);

        for y in 0..map.height {
            // Gate gap in the middle
            if y >= gate_y - 2 && y <= gate_y + 2 {
                let idx = map.xy_idx(wall_x, y);
                map.tiles[idx] = TileType::Road;
            } else {
                let idx = map.xy_idx(wall_x, y);
                map.tiles[idx] = TileType::Wall;
            }
        }

        // Add a main road leading from gate to the left side (toward piers)
        for x in 12..=wall_x {
            let idx = map.xy_idx(x, gate_y);
            map.tiles[idx] = TileType::Road;
        }

        // Mark tiles available for building (inside walls, not water, not roads)
        for y in 2..map.height - 2 {
            for x in 13..wall_x - 2 {
                let idx = map.xy_idx(x, y);
                if map.tiles[idx] == TileType::Grass {
                    available_building_tiles.insert(idx);
                }
            }
        }

        (available_building_tiles, gate_y)
    }

    fn buildings(
        &self,
        rng: &mut GameRng,
        build_data: &mut BuilderMap,
        available: &mut HashSet<usize>,
    ) -> Vec<(i32, i32, i32, i32)> {
        let mut buildings = Vec::new();
        let map = &mut build_data.map;

        // Try to place 12 buildings
        for _ in 0..12 {
            let w = rng.0.gen_range(4..8);
            let h = rng.0.gen_range(4..8);

            // Try random positions
            let mut placed = false;
            for _ in 0..50 {
                let x = rng.0.gen_range(14..28 - w);
                let y = rng.0.gen_range(3..map.height - 3 - h);

                // Check if area is available
                let mut can_place = true;
                for by in y..y + h {
                    for bx in x..x + w {
                        let idx = map.xy_idx(bx, by);
                        if !available.contains(&idx) {
                            can_place = false;
                            break;
                        }
                    }
                    if !can_place {
                        break;
                    }
                }

                if can_place {
                    // Place the building
                    buildings.push((x, y, w, h));

                    // Draw walls and floor
                    for by in y..y + h {
                        for bx in x..x + w {
                            let idx = map.xy_idx(bx, by);
                            if by == y || by == y + h - 1 || bx == x || bx == x + w - 1 {
                                map.tiles[idx] = TileType::Wall;
                            } else {
                                map.tiles[idx] = TileType::WoodFloor;
                            }
                            available.remove(&idx);
                        }
                    }

                    // Also remove tiles around building to create spacing
                    for by in (y - 1).max(0)..=(y + h).min(map.height - 1) {
                        for bx in (x - 1).max(0)..=(x + w).min(map.width - 1) {
                            let idx = map.xy_idx(bx, by);
                            available.remove(&idx);
                        }
                    }

                    placed = true;
                    break;
                }
            }

            if !placed {
                break; // Can't place more buildings
            }
        }

        buildings
    }

    fn add_doors(
        &self,
        rng: &mut GameRng,
        build_data: &mut BuilderMap,
        buildings: &[(i32, i32, i32, i32)],
        main_road_y: i32,
    ) -> Vec<(i32, i32)> {
        let map = &mut build_data.map;
        let mut door_positions = Vec::new();

        for (x, y, w, h) in buildings.iter() {
            // Calculate building center
            let center_y = *y + *h / 2;

            // Place door on the side closest to the main road
            let (door_x, door_y) = if main_road_y < *y {
                // Road is above building, put door on top
                (*x + rng.0.gen_range(1..*w - 1), *y)
            } else if main_road_y >= *y + *h {
                // Road is below building, put door on bottom
                (*x + rng.0.gen_range(1..*w - 1), *y + *h - 1)
            } else if main_road_y >= *y && main_road_y < *y + *h {
                // Road is at same level, prefer left side (toward road start)
                (*x, center_y.clamp(*y + 1, *y + *h - 2))
            } else {
                // Default to bottom
                (*x + rng.0.gen_range(1..*w - 1), *y + *h - 1)
            };

            let door_idx = map.xy_idx(door_x, door_y);
            map.tiles[door_idx] = TileType::Floor; // Door frame is a floor tile

            // Add door entity to spawn list
            build_data.spawn_list.push((door_idx, "Door".to_string()));

            // Store position just outside the door for path generation
            let outside_door = if door_y == *y {
                (door_x, door_y - 1) // Door on top, path starts above
            } else if door_y == *y + *h - 1 {
                (door_x, door_y + 1) // Door on bottom, path starts below
            } else if door_x == *x {
                (door_x - 1, door_y) // Door on left, path starts left
            } else {
                (door_x + 1, door_y) // Door on right, path starts right
            };
            door_positions.push(outside_door);
        }

        door_positions
    }

    fn add_paths(
        &self,
        build_data: &mut BuilderMap,
        door_positions: &[(i32, i32)],
        main_road_y: i32,
    ) {
        let map = &mut build_data.map;

        // Create paths from each door to the main road
        for &(start_x, start_y) in door_positions {
            // Path from door to main road at the same x position, then along to join
            let mut px = start_x;
            let mut py = start_y;

            // First, move vertically to the main road's y level
            let dy = if main_road_y > py { 1 } else { -1 };
            while py != main_road_y {
                let idx = map.xy_idx(px, py);
                // Only place road on grass tiles (don't overwrite buildings, water, etc.)
                if map.tiles[idx] == TileType::Grass {
                    map.tiles[idx] = TileType::Road;
                }
                py += dy;
            }

            // The main road already exists, no need to draw horizontal
        }
    }
}
