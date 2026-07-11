//! The classic "rooms and corridors" builder.
//!
//! Scatters random rectangular rooms across the map (throwing away any that
//! overlap), then joins each new room to the previous one with an L-shaped
//! tunnel.
//!
//! Good for: a reliable, traditional dungeon look. This is the safe default
//! when other generators get too weird.

use rand::Rng;

use crate::map::{Map, TileType, MAP_HEIGHT, MAP_WIDTH};
use crate::rng::GameRng;
use crate::shapes::Rect;

use super::common::*;
use super::{BuilderMap, InitialMapBuilder};

const MAX_ROOMS: i32 = 30;
const MIN_SIZE: i32 = 6;
const MAX_SIZE: i32 = 10;

#[derive(Default)]
pub struct SimpleMapBuilder;

impl SimpleMapBuilder {
    pub fn new() -> Self {
        Self
    }

    /// Core room generation logic shared by both traits
    fn generate_rooms(rng: &mut GameRng, map: &mut Map) -> Vec<Rect> {
        let mut rooms = Vec::new();

        for _ in 0..MAX_ROOMS {
            let w = rng.0.gen_range(MIN_SIZE..=MAX_SIZE);
            let h = rng.0.gen_range(MIN_SIZE..=MAX_SIZE);
            let x = rng.0.gen_range(1..MAP_WIDTH as i32 - w - 1);
            let y = rng.0.gen_range(1..MAP_HEIGHT as i32 - h - 1);
            let new_room = Rect::new(x, y, w, h);

            let ok = !rooms.iter().any(|r| new_room.intersect(r));
            if ok {
                apply_room_to_map(map, &new_room);

                if !rooms.is_empty() {
                    let (new_x, new_y) = new_room.center();
                    let (prev_x, prev_y) = rooms.last().unwrap().center();
                    if rng.0.gen_bool(0.5) {
                        apply_horizontal_tunnel(map, prev_x, new_x, prev_y);
                        apply_vertical_tunnel(map, prev_y, new_y, new_x);
                    } else {
                        apply_vertical_tunnel(map, prev_y, new_y, prev_x);
                        apply_horizontal_tunnel(map, prev_x, new_x, new_y);
                    }
                }
                rooms.push(new_room);
            }
        }

        rooms
    }
}

impl InitialMapBuilder for SimpleMapBuilder {
    fn build_map(&mut self, rng: &mut GameRng, build_data: &mut BuilderMap) {
        build_data.take_snapshot();

        let rooms = Self::generate_rooms(rng, &mut build_data.map);

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

