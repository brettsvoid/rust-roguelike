use bevy::prelude::*;
use rand::Rng;

use crate::map::{Map, TileType, MAP_HEIGHT, MAP_WIDTH};
use crate::rng::GameRng;
use crate::shapes::Rect;
use crate::spawner;

use super::common::*;
use super::{BuilderMap, InitialMapBuilder, MapBuilder};

pub struct BspDungeonBuilder {
    // Legacy fields for MapBuilder trait compatibility
    map: Map,
    rooms: Vec<Rect>,
    rects: Vec<Rect>,
    depth: i32,
    history: Vec<Map>,
}

impl BspDungeonBuilder {
    pub fn new(depth: i32) -> Self {
        Self {
            map: Map::new(MAP_WIDTH, MAP_HEIGHT, depth),
            rooms: Vec::new(),
            rects: Vec::new(),
            depth,
            history: Vec::new(),
        }
    }

    fn add_subrects(rects: &mut Vec<Rect>, rect: Rect) {
        let width = i32::abs(rect.x1 - rect.x2);
        let height = i32::abs(rect.y1 - rect.y2);
        let half_width = i32::max(width / 2, 1);
        let half_height = i32::max(height / 2, 1);

        rects.push(Rect::new(rect.x1, rect.y1, half_width, half_height));
        rects.push(Rect::new(
            rect.x1,
            rect.y1 + half_height,
            half_width,
            half_height,
        ));
        rects.push(Rect::new(
            rect.x1 + half_width,
            rect.y1,
            half_width,
            half_height,
        ));
        rects.push(Rect::new(
            rect.x1 + half_width,
            rect.y1 + half_height,
            half_width,
            half_height,
        ));
    }

    fn get_random_rect(rects: &[Rect], rng: &mut GameRng) -> Rect {
        if rects.len() == 1 {
            return rects[0].clone();
        }
        let idx = rng.0.gen_range(0..rects.len());
        rects[idx].clone()
    }

    fn get_random_sub_rect(rect: &Rect, rng: &mut GameRng) -> Rect {
        let rect_width = i32::abs(rect.x1 - rect.x2);
        let rect_height = i32::abs(rect.y1 - rect.y2);

        let w = i32::max(3, rng.0.gen_range(1..=i32::min(rect_width, 10)));
        let h = i32::max(3, rng.0.gen_range(1..=i32::min(rect_height, 10)));

        let x_offset = rng.0.gen_range(0..6);
        let y_offset = rng.0.gen_range(0..6);

        Rect::new(rect.x1 + x_offset, rect.y1 + y_offset, w, h)
    }

    fn is_possible(map: &Map, rect: &Rect) -> bool {
        // Check with 2-tile buffer to prevent overlaps
        let expanded_x1 = rect.x1 - 2;
        let expanded_y1 = rect.y1 - 2;
        let expanded_x2 = rect.x2 + 2;
        let expanded_y2 = rect.y2 + 2;

        for y in expanded_y1..=expanded_y2 {
            for x in expanded_x1..=expanded_x2 {
                if x > MAP_WIDTH as i32 - 2 {
                    return false;
                }
                if y > MAP_HEIGHT as i32 - 2 {
                    return false;
                }
                if x < 1 {
                    return false;
                }
                if y < 1 {
                    return false;
                }
                let idx = map.xy_idx(x, y);
                if map.tiles[idx] != TileType::Wall {
                    return false;
                }
            }
        }
        true
    }

    /// Core BSP generation logic shared by both traits
    fn generate_bsp(rng: &mut GameRng, map: &mut Map) -> Vec<Rect> {
        let mut rects = Vec::new();
        let mut rooms = Vec::new();

        // Start with single rect covering most of the map
        rects.push(Rect::new(2, 2, MAP_WIDTH as i32 - 5, MAP_HEIGHT as i32 - 5));
        let first_room = rects[0].clone();
        Self::add_subrects(&mut rects, first_room);

        let mut n_rooms = 0;
        while n_rooms < 240 {
            let rect = Self::get_random_rect(&rects, rng);
            let candidate = Self::get_random_sub_rect(&rect, rng);

            if Self::is_possible(map, &candidate) {
                apply_room_to_map(map, &candidate);
                rooms.push(candidate);
                Self::add_subrects(&mut rects, rect);
            }
            n_rooms += 1;
        }

        // Sort rooms by x position for corridor generation
        rooms.sort_by(|a, b| a.x1.cmp(&b.x1));

        // Connect rooms with corridors
        for i in 0..rooms.len().saturating_sub(1) {
            let room = &rooms[i];
            let next_room = &rooms[i + 1];
            let start_x = room.x1 + rng.0.gen_range(0..i32::abs(room.x1 - room.x2).max(1));
            let start_y = room.y1 + rng.0.gen_range(0..i32::abs(room.y1 - room.y2).max(1));
            let end_x =
                next_room.x1 + rng.0.gen_range(0..i32::abs(next_room.x1 - next_room.x2).max(1));
            let end_y =
                next_room.y1 + rng.0.gen_range(0..i32::abs(next_room.y1 - next_room.y2).max(1));
            draw_corridor(map, start_x, start_y, end_x, end_y);
        }

        rooms
    }
}

// ============================================================================
// New InitialMapBuilder trait implementation
// ============================================================================

impl InitialMapBuilder for BspDungeonBuilder {
    fn build_map(&mut self, rng: &mut GameRng, build_data: &mut BuilderMap) {
        build_data.take_snapshot();

        let rooms = Self::generate_bsp(rng, &mut build_data.map);

        // Set starting position to first room center
        if let Some(first_room) = rooms.first() {
            build_data.starting_position = Some(first_room.center());
        }

        // Place stairs in last room
        if let Some(last_room) = rooms.last() {
            let (stairs_x, stairs_y) = last_room.center();
            let stairs_idx = build_data.map.xy_idx(stairs_x, stairs_y);
            build_data.map.tiles[stairs_idx] = TileType::DownStairs;
        }

        build_data.rooms = Some(rooms);
        build_data.take_snapshot();
    }
}

// ============================================================================
// Legacy MapBuilder trait implementation (for backwards compatibility)
// ============================================================================

impl MapBuilder for BspDungeonBuilder {
    fn build_map(&mut self, rng: &mut GameRng) {
        self.take_snapshot();

        self.rooms = Self::generate_bsp(rng, &mut self.map);

        // Place stairs in last room
        if let Some(last_room) = self.rooms.last() {
            let (stairs_x, stairs_y) = last_room.center();
            let stairs_idx = self.map.xy_idx(stairs_x, stairs_y);
            self.map.tiles[stairs_idx] = TileType::DownStairs;
        }
        self.take_snapshot();
    }

    fn spawn_entities(&self, commands: &mut Commands, rng: &mut GameRng, font: &TextFont) {
        let mut monster_id: usize = 0;
        for room in self.rooms.iter().skip(1) {
            spawner::spawn_room(commands, rng, font, room, &mut monster_id, self.depth);
        }
    }

    fn get_map(&self) -> Map {
        self.map.clone()
    }

    fn get_starting_position(&self) -> (i32, i32) {
        self.rooms.first().map(|r| r.center()).unwrap_or((MAP_WIDTH as i32 / 2, MAP_HEIGHT as i32 / 2))
    }

    fn get_snapshot_history(&self) -> Vec<Map> {
        self.history.clone()
    }

    fn take_snapshot(&mut self) {
        self.history.push(self.map.clone());
    }

    fn get_spawn_regions(&self) -> Vec<Rect> {
        self.rooms.clone()
    }

    fn get_name(&self) -> &'static str {
        "BSP Dungeon"
    }
}
