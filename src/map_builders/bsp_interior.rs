//! BSP interior builder.
//!
//! Like the BSP dungeon, but instead of placing rooms *inside* the slices,
//! every slice *becomes* a room - the only walls left are the thin borders
//! between slices. No wasted space anywhere.
//!
//! Good for: man-made interiors - the floor of a fortress, barracks, or
//! office-like warren of connected rooms.

use rand::Rng;

use crate::map::{TileType, MAP_HEIGHT, MAP_WIDTH};
use crate::rng::GameRng;
use crate::shapes::Rect;

use super::common::*;
use super::{BuilderMap, InitialMapBuilder};

const MIN_ROOM_SIZE: i32 = 8;

#[derive(Default)]
pub struct BspInteriorBuilder;

impl BspInteriorBuilder {
    pub fn new() -> Self {
        Self
    }
}

fn add_subrects_recursive(rects: &mut Vec<Rect>, rect: Rect, rng: &mut GameRng) {
    // Remove the parent rect from the list if present
    if let Some(idx) = rects.iter().position(|r| *r == rect) {
        rects.remove(idx);
    }

    let width = rect.x2 - rect.x1;
    let height = rect.y2 - rect.y1;
    let half_width = width / 2;
    let half_height = height / 2;

    let split = rng.0.gen_range(0..2);

    if split == 0 {
        // Horizontal split
        let h1 = Rect::new(rect.x1, rect.y1, half_width - 1, height);
        rects.push(h1.clone());
        if half_width > MIN_ROOM_SIZE {
            add_subrects_recursive(rects, h1, rng);
        }
        let h2 = Rect::new(rect.x1 + half_width, rect.y1, half_width, height);
        rects.push(h2.clone());
        if half_width > MIN_ROOM_SIZE {
            add_subrects_recursive(rects, h2, rng);
        }
    } else {
        // Vertical split
        let v1 = Rect::new(rect.x1, rect.y1, width, half_height - 1);
        rects.push(v1.clone());
        if half_height > MIN_ROOM_SIZE {
            add_subrects_recursive(rects, v1, rng);
        }
        let v2 = Rect::new(rect.x1, rect.y1 + half_height, width, half_height);
        rects.push(v2.clone());
        if half_height > MIN_ROOM_SIZE {
            add_subrects_recursive(rects, v2, rng);
        }
    }
}

impl InitialMapBuilder for BspInteriorBuilder {
    fn build_map(&mut self, rng: &mut GameRng, build_data: &mut BuilderMap) {
        build_data.take_snapshot();

        // Start with a single rect covering the map (with border)
        let mut rects = Vec::new();
        rects.push(Rect::new(1, 1, MAP_WIDTH as i32 - 2, MAP_HEIGHT as i32 - 2));
        let first_room = rects[0].clone();
        add_subrects_recursive(&mut rects, first_room, rng);

        let mut rooms = Vec::new();

        // Carve rooms from the subdivided rects
        for rect in &rects {
            for y in rect.y1 + 1..rect.y2 {
                for x in rect.x1 + 1..rect.x2 {
                    let idx = build_data.map.xy_idx(x, y);
                    if idx > 0 && idx < (MAP_WIDTH * MAP_HEIGHT - 1) {
                        build_data.map.tiles[idx] = TileType::Floor;
                    }
                }
            }
            rooms.push(rect.clone());
            build_data.take_snapshot();
        }

        // Sort rooms by x position for corridor generation
        rooms.sort_by(|a, b| a.x1.cmp(&b.x1));

        // Connect rooms with corridors
        for i in 0..rooms.len().saturating_sub(1) {
            let room = &rooms[i];
            let next_room = &rooms[i + 1];
            let start_x = room.x1 + rng.0.gen_range(0..(room.x2 - room.x1).max(1));
            let start_y = room.y1 + rng.0.gen_range(0..(room.y2 - room.y1).max(1));
            let end_x = next_room.x1 + rng.0.gen_range(0..(next_room.x2 - next_room.x1).max(1));
            let end_y = next_room.y1 + rng.0.gen_range(0..(next_room.y2 - next_room.y1).max(1));
            draw_corridor(&mut build_data.map, start_x, start_y, end_x, end_y);
            build_data.take_snapshot();
        }

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
