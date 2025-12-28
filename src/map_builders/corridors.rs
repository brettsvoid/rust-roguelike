use rand::Rng;

use crate::rng::GameRng;

use super::common::{apply_horizontal_tunnel, apply_vertical_tunnel, draw_corridor};
use super::{BuilderMap, MetaMapBuilder};

// ============================================================================
// DoglegCorridors - L-shaped corridors between rooms
// ============================================================================

pub struct DoglegCorridors;

impl DoglegCorridors {
    pub fn new() -> Box<Self> {
        Box::new(Self)
    }
}

impl MetaMapBuilder for DoglegCorridors {
    fn build_map(&mut self, rng: &mut GameRng, build_data: &mut BuilderMap) {
        if let Some(rooms) = build_data.rooms.clone() {
            for i in 0..rooms.len().saturating_sub(1) {
                let (x1, y1) = rooms[i].center();
                let (x2, y2) = rooms[i + 1].center();

                // Randomly choose horizontal-first or vertical-first
                if rng.0.gen_bool(0.5) {
                    apply_horizontal_tunnel(&mut build_data.map, x1, x2, y1);
                    apply_vertical_tunnel(&mut build_data.map, y1, y2, x2);
                } else {
                    apply_vertical_tunnel(&mut build_data.map, y1, y2, x1);
                    apply_horizontal_tunnel(&mut build_data.map, x1, x2, y2);
                }
            }
        }
        build_data.take_snapshot();
    }
}

// ============================================================================
// BspCorridors - Random point-to-point corridors (BSP style)
// ============================================================================

pub struct BspCorridors;

impl BspCorridors {
    pub fn new() -> Box<Self> {
        Box::new(Self)
    }
}

impl MetaMapBuilder for BspCorridors {
    fn build_map(&mut self, rng: &mut GameRng, build_data: &mut BuilderMap) {
        if let Some(rooms) = build_data.rooms.clone() {
            for i in 0..rooms.len().saturating_sub(1) {
                let room = &rooms[i];
                let next_room = &rooms[i + 1];

                // Random point within current room
                let room_width = i32::abs(room.x1 - room.x2).max(1);
                let room_height = i32::abs(room.y1 - room.y2).max(1);
                let start_x = room.x1 + rng.0.gen_range(0..room_width);
                let start_y = room.y1 + rng.0.gen_range(0..room_height);

                // Random point within next room
                let next_width = i32::abs(next_room.x1 - next_room.x2).max(1);
                let next_height = i32::abs(next_room.y1 - next_room.y2).max(1);
                let end_x = next_room.x1 + rng.0.gen_range(0..next_width);
                let end_y = next_room.y1 + rng.0.gen_range(0..next_height);

                draw_corridor(&mut build_data.map, start_x, start_y, end_x, end_y);
            }
        }
        build_data.take_snapshot();
    }
}

// ============================================================================
// StraightLineCorridors - Direct center-to-center corridors
// ============================================================================

pub struct StraightLineCorridors;

impl StraightLineCorridors {
    pub fn new() -> Box<Self> {
        Box::new(Self)
    }
}

impl MetaMapBuilder for StraightLineCorridors {
    fn build_map(&mut self, _rng: &mut GameRng, build_data: &mut BuilderMap) {
        if let Some(rooms) = build_data.rooms.clone() {
            for i in 0..rooms.len().saturating_sub(1) {
                let (x1, y1) = rooms[i].center();
                let (x2, y2) = rooms[i + 1].center();

                // Direct line between room centers
                draw_corridor(&mut build_data.map, x1, y1, x2, y2);
            }
        }
        build_data.take_snapshot();
    }
}

// ============================================================================
// NearestCorridors - Connect each room to its nearest neighbor
// ============================================================================

pub struct NearestCorridors;

impl NearestCorridors {
    pub fn new() -> Box<Self> {
        Box::new(Self)
    }
}

impl MetaMapBuilder for NearestCorridors {
    fn build_map(&mut self, _rng: &mut GameRng, build_data: &mut BuilderMap) {
        if let Some(rooms) = build_data.rooms.clone() {
            let mut connected: Vec<bool> = vec![false; rooms.len()];

            for i in 0..rooms.len() {
                let (x1, y1) = rooms[i].center();
                let mut best_distance = i32::MAX;
                let mut best_idx = None;

                // Find nearest unconnected room
                for j in 0..rooms.len() {
                    if i != j && !connected[j] {
                        let (x2, y2) = rooms[j].center();
                        let distance = (x1 - x2).abs() + (y1 - y2).abs();
                        if distance < best_distance {
                            best_distance = distance;
                            best_idx = Some(j);
                        }
                    }
                }

                if let Some(j) = best_idx {
                    let (x2, y2) = rooms[j].center();
                    draw_corridor(&mut build_data.map, x1, y1, x2, y2);
                    connected[i] = true;
                }
            }
        }
        build_data.take_snapshot();
    }
}
