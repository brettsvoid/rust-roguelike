use bevy::prelude::*;
use rand::Rng;

use crate::map::{Map, TileType, MAP_HEIGHT, MAP_WIDTH};
use crate::rng::GameRng;
use crate::shapes::Rect;
use crate::spawner;

use super::common::*;
use super::MapBuilder;

const MAX_ROOMS: i32 = 30;
const MIN_SIZE: i32 = 6;
const MAX_SIZE: i32 = 10;

pub struct SimpleMapBuilder {
    map: Map,
    rooms: Vec<Rect>,
    depth: i32,
    history: Vec<Map>,
}

impl SimpleMapBuilder {
    pub fn new(depth: i32) -> Self {
        Self {
            map: Map::new(MAP_WIDTH, MAP_HEIGHT, depth),
            rooms: Vec::new(),
            depth,
            history: Vec::new(),
        }
    }
}

impl MapBuilder for SimpleMapBuilder {
    fn build_map(&mut self, rng: &mut GameRng) {
        // Take initial snapshot (all walls)
        self.take_snapshot();

        for _ in 0..MAX_ROOMS {
            let w = rng.0.gen_range(MIN_SIZE..=MAX_SIZE);
            let h = rng.0.gen_range(MIN_SIZE..=MAX_SIZE);
            let x = rng.0.gen_range(1..MAP_WIDTH as i32 - w - 1);
            let y = rng.0.gen_range(1..MAP_HEIGHT as i32 - h - 1);
            let new_room = Rect::new(x, y, w, h);

            let ok = !self.rooms.iter().any(|r| new_room.intersect(r));
            if ok {
                apply_room_to_map(&mut self.map, &new_room);
                self.take_snapshot();

                if !self.rooms.is_empty() {
                    let (new_x, new_y) = new_room.center();
                    let (prev_x, prev_y) = self.rooms.last().unwrap().center();
                    if rng.0.gen_bool(0.5) {
                        apply_horizontal_tunnel(&mut self.map, prev_x, new_x, prev_y);
                        apply_vertical_tunnel(&mut self.map, prev_y, new_y, new_x);
                    } else {
                        apply_vertical_tunnel(&mut self.map, prev_y, new_y, prev_x);
                        apply_horizontal_tunnel(&mut self.map, prev_x, new_x, new_y);
                    }
                    self.take_snapshot();
                }
                self.rooms.push(new_room);
            }
        }

        // Place stairs in last room
        let (stairs_x, stairs_y) = self.rooms.last().unwrap().center();
        let stairs_idx = self.map.xy_idx(stairs_x, stairs_y);
        self.map.tiles[stairs_idx] = TileType::DownStairs;
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
        self.rooms[0].center()
    }

    fn get_snapshot_history(&self) -> Vec<Map> {
        self.history.clone()
    }

    fn take_snapshot(&mut self) {
        self.history.push(self.map.clone());
    }

    fn get_spawn_regions(&self) -> Vec<crate::shapes::Rect> {
        self.rooms.clone()
    }
}
