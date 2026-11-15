//! The starting town (depth 1).
//!
//! Not procedural chaos like the dungeons - a hand-shaped map: grass with a
//! river, a main road with a bridge, and a handful of buildings (pub,
//! temple, shops, hovels) sized by importance and linked to the road by
//! paths. The player starts at the pub; the "dungeon entrance" leads down.

use std::collections::HashSet;

use rand::seq::SliceRandom;
use rand::Rng;

use crate::map::TileType;
use crate::rng::GameRng;

use super::{BuilderMap, InitialMapBuilder};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BuildingTag {
    Pub,
    Temple,
    Blacksmith,
    Clothier,
    Alchemist,
    PlayerHouse,
    Hovel,
    Abandoned,
}

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
        let buildings = self.buildings(rng, build_data, &mut available_building_tiles);
        build_data.take_snapshot();

        if buildings.is_empty() {
            // Fallback if no buildings could be placed
            build_data.starting_position = Some((15, wall_gap_y));
        } else {
            // Find the pub (first building, largest) and set starting position there
            let pub_rect = &buildings[0];
            build_data.starting_position =
                Some((pub_rect.0 + pub_rect.2 / 2, pub_rect.1 + pub_rect.3 / 2));
        }

        // Build doors and get their positions for path generation
        let door_positions = self.add_doors(rng, build_data, &buildings, wall_gap_y);

        // Populate buildings with NPCs and props
        self.populate_buildings(rng, build_data, &buildings);
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
    ) -> Vec<(i32, i32, i32, i32, BuildingTag)> {
        let mut buildings: Vec<(i32, i32, i32, i32)> = Vec::new();
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

        // Sort buildings by size (largest first)
        buildings.sort_by(|a, b| (b.2 * b.3).cmp(&(a.2 * a.3)));

        // Assign tags based on sorted order
        let tags = [
            BuildingTag::Pub,
            BuildingTag::Temple,
            BuildingTag::Blacksmith,
            BuildingTag::Clothier,
            BuildingTag::Alchemist,
            BuildingTag::PlayerHouse,
            BuildingTag::Hovel,
            BuildingTag::Abandoned,
        ];

        buildings
            .into_iter()
            .enumerate()
            .map(|(i, (x, y, w, h))| {
                let tag = if i < tags.len() {
                    tags[i]
                } else {
                    BuildingTag::Abandoned
                };
                (x, y, w, h, tag)
            })
            .collect()
    }

    fn add_doors(
        &self,
        rng: &mut GameRng,
        build_data: &mut BuilderMap,
        buildings: &[(i32, i32, i32, i32, BuildingTag)],
        main_road_y: i32,
    ) -> Vec<(i32, i32)> {
        let map = &mut build_data.map;
        let mut door_positions = Vec::new();

        for (x, y, w, h, _tag) in buildings.iter() {
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
            let px = start_x;
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

    fn populate_buildings(
        &self,
        rng: &mut GameRng,
        build_data: &mut BuilderMap,
        buildings: &[(i32, i32, i32, i32, BuildingTag)],
    ) {
        for (x, y, w, h, tag) in buildings {
            match tag {
                BuildingTag::Pub => self.build_pub(rng, build_data, *x, *y, *w, *h),
                BuildingTag::Temple => self.build_temple(rng, build_data, *x, *y, *w, *h),
                BuildingTag::Blacksmith => self.build_blacksmith(rng, build_data, *x, *y, *w, *h),
                BuildingTag::Clothier => self.build_clothier(rng, build_data, *x, *y, *w, *h),
                BuildingTag::Alchemist => self.build_alchemist(rng, build_data, *x, *y, *w, *h),
                BuildingTag::PlayerHouse => self.build_player_house(rng, build_data, *x, *y, *w, *h),
                BuildingTag::Hovel => self.build_hovel(rng, build_data, *x, *y, *w, *h),
                BuildingTag::Abandoned => self.build_abandoned(rng, build_data, *x, *y, *w, *h),
            }
        }
    }

    /// Get interior tiles of a building (excluding walls), shuffled randomly
    fn get_interior_tiles(&self, rng: &mut GameRng, build_data: &BuilderMap, x: i32, y: i32, w: i32, h: i32) -> Vec<usize> {
        let mut tiles = Vec::new();
        for by in (y + 1)..(y + h - 1) {
            for bx in (x + 1)..(x + w - 1) {
                tiles.push(build_data.map.xy_idx(bx, by));
            }
        }
        tiles.shuffle(&mut rng.0);
        tiles
    }

    fn build_pub(
        &self,
        rng: &mut GameRng,
        build_data: &mut BuilderMap,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
    ) {
        // Place barkeep in center
        let center_idx = build_data.map.xy_idx(x + w / 2, y + h / 2);
        build_data.spawn_list.push((center_idx, "Barkeep".to_string()));

        // Get interior tiles for props (shuffled for random placement)
        let mut interior = self.get_interior_tiles(rng, build_data, x, y, w, h);
        // Remove center (where barkeep is)
        interior.retain(|&idx| idx != center_idx);

        // Add patrons
        let num_patrons = rng.0.gen_range(1..=3);
        for _ in 0..num_patrons {
            if let Some(&idx) = interior.first() {
                build_data.spawn_list.push((idx, "Patron".to_string()));
                interior.remove(0);
            }
        }

        // Add props (tables, chairs, kegs)
        let props = ["Table", "Chair", "Keg"];
        for prop in props {
            if let Some(&idx) = interior.first() {
                build_data.spawn_list.push((idx, prop.to_string()));
                interior.remove(0);
            }
        }
    }

    fn build_temple(&self, rng: &mut GameRng, build_data: &mut BuilderMap, x: i32, y: i32, w: i32, h: i32) {
        // Place priest in center
        let center_idx = build_data.map.xy_idx(x + w / 2, y + h / 2);
        build_data.spawn_list.push((center_idx, "Priest".to_string()));

        // Add candles
        let mut interior = self.get_interior_tiles(rng, build_data, x, y, w, h);
        interior.retain(|&idx| idx != center_idx);

        for _ in 0..2 {
            if let Some(&idx) = interior.first() {
                build_data.spawn_list.push((idx, "Candle".to_string()));
                interior.remove(0);
            }
        }
    }

    fn build_blacksmith(&self, rng: &mut GameRng, build_data: &mut BuilderMap, x: i32, y: i32, w: i32, h: i32) {
        // Place blacksmith in center
        let center_idx = build_data.map.xy_idx(x + w / 2, y + h / 2);
        build_data.spawn_list.push((center_idx, "Blacksmith".to_string()));

        // Add props
        let mut interior = self.get_interior_tiles(rng, build_data, x, y, w, h);
        interior.retain(|&idx| idx != center_idx);

        let props = ["Anvil", "Water Trough", "Weapon Rack", "Armor Stand"];
        for prop in props {
            if let Some(&idx) = interior.first() {
                build_data.spawn_list.push((idx, prop.to_string()));
                interior.remove(0);
            }
        }
    }

    fn build_clothier(&self, rng: &mut GameRng, build_data: &mut BuilderMap, x: i32, y: i32, w: i32, h: i32) {
        // Place clothier in center
        let center_idx = build_data.map.xy_idx(x + w / 2, y + h / 2);
        build_data.spawn_list.push((center_idx, "Clothier".to_string()));

        // Add props
        let mut interior = self.get_interior_tiles(rng, build_data, x, y, w, h);
        interior.retain(|&idx| idx != center_idx);

        let props = ["Loom", "Hide Rack", "Cabinet"];
        for prop in props {
            if let Some(&idx) = interior.first() {
                build_data.spawn_list.push((idx, prop.to_string()));
                interior.remove(0);
            }
        }
    }

    fn build_alchemist(&self, rng: &mut GameRng, build_data: &mut BuilderMap, x: i32, y: i32, w: i32, h: i32) {
        // Place alchemist in center
        let center_idx = build_data.map.xy_idx(x + w / 2, y + h / 2);
        build_data.spawn_list.push((center_idx, "Alchemist".to_string()));

        // Add props
        let mut interior = self.get_interior_tiles(rng, build_data, x, y, w, h);
        interior.retain(|&idx| idx != center_idx);

        let props = ["Chemistry Set", "Dead Thing", "Cabinet"];
        for prop in props {
            if let Some(&idx) = interior.first() {
                build_data.spawn_list.push((idx, prop.to_string()));
                interior.remove(0);
            }
        }
    }

    fn build_player_house(&self, rng: &mut GameRng, build_data: &mut BuilderMap, x: i32, y: i32, w: i32, h: i32) {
        // Place mom in center
        let center_idx = build_data.map.xy_idx(x + w / 2, y + h / 2);
        build_data.spawn_list.push((center_idx, "Mom".to_string()));

        // Add props
        let mut interior = self.get_interior_tiles(rng, build_data, x, y, w, h);
        interior.retain(|&idx| idx != center_idx);

        let props = ["Bed", "Cabinet"];
        for prop in props {
            if let Some(&idx) = interior.first() {
                build_data.spawn_list.push((idx, prop.to_string()));
                interior.remove(0);
            }
        }
    }

    fn build_hovel(
        &self,
        rng: &mut GameRng,
        build_data: &mut BuilderMap,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
    ) {
        // Place peasant in center
        let center_idx = build_data.map.xy_idx(x + w / 2, y + h / 2);
        build_data.spawn_list.push((center_idx, "Peasant".to_string()));

        // Maybe add some basic furniture
        let mut interior = self.get_interior_tiles(rng, build_data, x, y, w, h);
        interior.retain(|&idx| idx != center_idx);

        if rng.0.gen_bool(0.5) && !interior.is_empty() {
            let idx = interior[0];
            build_data.spawn_list.push((idx, "Bed".to_string()));
        }
    }

    fn build_abandoned(
        &self,
        rng: &mut GameRng,
        build_data: &mut BuilderMap,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
    ) {
        // Spawn rats in abandoned buildings
        let interior = self.get_interior_tiles(rng, build_data, x, y, w, h);
        let num_rats = rng.0.gen_range(1..=3);

        for i in 0..num_rats {
            if let Some(&idx) = interior.get(i) {
                build_data.spawn_list.push((idx, "Rat".to_string()));
            }
        }
    }
}
