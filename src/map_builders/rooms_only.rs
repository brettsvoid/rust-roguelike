use rand::Rng;

use crate::map::{Map, TileType, MAP_HEIGHT, MAP_WIDTH};
use crate::rng::GameRng;
use crate::shapes::Rect;

use super::common::apply_room_to_map;
use super::{BuilderMap, InitialMapBuilder};

// ============================================================================
// SimpleMapRoomsBuilder - Generates rooms WITHOUT corridors
// ============================================================================

const MAX_ROOMS: i32 = 30;
const MIN_SIZE: i32 = 6;
const MAX_SIZE: i32 = 10;

pub struct SimpleMapRoomsBuilder {
    depth: i32,
}

impl SimpleMapRoomsBuilder {
    pub fn new(depth: i32) -> Self {
        Self { depth }
    }
}

impl InitialMapBuilder for SimpleMapRoomsBuilder {
    fn build_map(&mut self, rng: &mut GameRng, build_data: &mut BuilderMap) {
        build_data.take_snapshot();

        let mut rooms = Vec::new();

        for _ in 0..MAX_ROOMS {
            let w = rng.0.gen_range(MIN_SIZE..=MAX_SIZE);
            let h = rng.0.gen_range(MIN_SIZE..=MAX_SIZE);
            let x = rng.0.gen_range(1..MAP_WIDTH as i32 - w - 1);
            let y = rng.0.gen_range(1..MAP_HEIGHT as i32 - h - 1);
            let new_room = Rect::new(x, y, w, h);

            let ok = !rooms.iter().any(|r: &Rect| new_room.intersect(r));
            if ok {
                apply_room_to_map(&mut build_data.map, &new_room);
                rooms.push(new_room);
            }
        }

        // No corridor generation - rooms only!
        build_data.rooms = Some(rooms);
        build_data.take_snapshot();
    }
}

// ============================================================================
// BspRoomsBuilder - BSP room subdivision WITHOUT corridors
// ============================================================================

pub struct BspRoomsBuilder {
    depth: i32,
}

impl BspRoomsBuilder {
    pub fn new(depth: i32) -> Self {
        Self { depth }
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
}

impl InitialMapBuilder for BspRoomsBuilder {
    fn build_map(&mut self, rng: &mut GameRng, build_data: &mut BuilderMap) {
        build_data.take_snapshot();

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

            if Self::is_possible(&build_data.map, &candidate) {
                apply_room_to_map(&mut build_data.map, &candidate);
                rooms.push(candidate);
                Self::add_subrects(&mut rects, rect);
            }
            n_rooms += 1;
        }

        // No corridor generation - rooms only!
        // Don't sort rooms here - let RoomSorter handle that
        build_data.rooms = Some(rooms);
        build_data.take_snapshot();
    }
}
