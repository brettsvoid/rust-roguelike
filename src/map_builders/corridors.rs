use rand::Rng;

use crate::rng::GameRng;

use super::common::{apply_horizontal_tunnel, apply_vertical_tunnel, draw_corridor, draw_corridor_bresenham};
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
        let mut corridors: Vec<Vec<usize>> = Vec::new();

        if let Some(rooms) = build_data.rooms.clone() {
            for i in 0..rooms.len().saturating_sub(1) {
                let (x1, y1) = rooms[i].center();
                let (x2, y2) = rooms[i + 1].center();

                // Randomly choose horizontal-first or vertical-first
                if rng.0.gen_bool(0.5) {
                    let mut c1 = apply_horizontal_tunnel(&mut build_data.map, x1, x2, y1);
                    let mut c2 = apply_vertical_tunnel(&mut build_data.map, y1, y2, x2);
                    c1.append(&mut c2);
                    corridors.push(c1);
                } else {
                    let mut c1 = apply_vertical_tunnel(&mut build_data.map, y1, y2, x1);
                    let mut c2 = apply_horizontal_tunnel(&mut build_data.map, x1, x2, y2);
                    c1.append(&mut c2);
                    corridors.push(c1);
                }
            }
        }

        build_data.corridors = Some(corridors);
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
        let mut corridors: Vec<Vec<usize>> = Vec::new();

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

                let corridor = draw_corridor(&mut build_data.map, start_x, start_y, end_x, end_y);
                corridors.push(corridor);
            }
        }

        build_data.corridors = Some(corridors);
        build_data.take_snapshot();
    }
}

// ============================================================================
// StraightLineCorridors - Direct center-to-center corridors using Bresenham
// ============================================================================

pub struct StraightLineCorridors;

impl StraightLineCorridors {
    pub fn new() -> Box<Self> {
        Box::new(Self)
    }
}

impl MetaMapBuilder for StraightLineCorridors {
    fn build_map(&mut self, _rng: &mut GameRng, build_data: &mut BuilderMap) {
        let mut corridors: Vec<Vec<usize>> = Vec::new();

        if let Some(rooms) = build_data.rooms.clone() {
            for i in 0..rooms.len().saturating_sub(1) {
                let (x1, y1) = rooms[i].center();
                let (x2, y2) = rooms[i + 1].center();

                // Direct diagonal line between room centers using Bresenham
                let corridor = draw_corridor_bresenham(&mut build_data.map, x1, y1, x2, y2);
                corridors.push(corridor);
            }
        }

        build_data.corridors = Some(corridors);
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
        let mut corridors: Vec<Vec<usize>> = Vec::new();

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
                    let corridor = draw_corridor(&mut build_data.map, x1, y1, x2, y2);
                    corridors.push(corridor);
                    connected[i] = true;
                }
            }
        }

        build_data.corridors = Some(corridors);
        build_data.take_snapshot();
    }
}

// ============================================================================
// CorridorSpawner - Spawn entities in corridors
// ============================================================================

pub struct CorridorSpawner;

impl CorridorSpawner {
    pub fn new() -> Box<Self> {
        Box::new(Self)
    }
}

impl MetaMapBuilder for CorridorSpawner {
    fn build_map(&mut self, rng: &mut GameRng, build_data: &mut BuilderMap) {
        if let Some(corridors) = &build_data.corridors {
            for corridor in corridors.iter() {
                // Only spawn in corridors with at least 2 tiles
                if corridor.len() < 2 {
                    continue;
                }

                // 25% chance to spawn something in each corridor
                if !rng.0.gen_bool(0.25) {
                    continue;
                }

                // Pick a random tile in the corridor (not the endpoints)
                let spawn_idx = if corridor.len() > 2 {
                    corridor[rng.0.gen_range(1..corridor.len() - 1)]
                } else {
                    corridor[0]
                };

                // Add to spawn list - 50% monster, 50% item
                if rng.0.gen_bool(0.5) {
                    // Monster
                    if rng.0.gen_bool(0.5) {
                        build_data.spawn_list.push((spawn_idx, "Goblin".to_string()));
                    } else {
                        build_data.spawn_list.push((spawn_idx, "Orc".to_string()));
                    }
                } else {
                    // Item
                    let roll = rng.0.gen_range(0..6);
                    let item = match roll {
                        0 => "Health Potion",
                        1 => "Rations",
                        2 => "Magic Missile Scroll",
                        3 => "Dagger",
                        4 => "Shield",
                        _ => "Bear Trap",
                    };
                    build_data.spawn_list.push((spawn_idx, item.to_string()));
                }
            }
        }
    }
}
