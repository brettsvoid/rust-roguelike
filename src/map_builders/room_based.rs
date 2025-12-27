use crate::map::TileType;
use crate::rng::GameRng;

use super::{BuilderMap, MetaMapBuilder};

// ============================================================================
// Room-Based Starting Position
// ============================================================================

pub struct RoomBasedStartingPosition;

impl RoomBasedStartingPosition {
    pub fn new() -> Box<Self> {
        Box::new(Self)
    }
}

impl MetaMapBuilder for RoomBasedStartingPosition {
    fn build_map(&mut self, _rng: &mut GameRng, build_data: &mut BuilderMap) {
        if let Some(ref rooms) = build_data.rooms {
            if let Some(first_room) = rooms.first() {
                build_data.starting_position = Some(first_room.center());
            }
        }
    }
}

// ============================================================================
// Room-Based Stairs (place in last room)
// ============================================================================

pub struct RoomBasedStairs;

impl RoomBasedStairs {
    pub fn new() -> Box<Self> {
        Box::new(Self)
    }
}

impl MetaMapBuilder for RoomBasedStairs {
    fn build_map(&mut self, _rng: &mut GameRng, build_data: &mut BuilderMap) {
        if let Some(ref rooms) = build_data.rooms {
            if let Some(last_room) = rooms.last() {
                let (x, y) = last_room.center();
                let idx = build_data.map.xy_idx(x, y);
                build_data.map.tiles[idx] = TileType::DownStairs;
            }
        }
        build_data.take_snapshot();
    }
}

// ============================================================================
// Room-Based Spawner (spawn in rooms, skip first)
// ============================================================================

use rand::Rng;

pub struct RoomBasedSpawner;

impl RoomBasedSpawner {
    pub fn new() -> Box<Self> {
        Box::new(Self)
    }
}

impl MetaMapBuilder for RoomBasedSpawner {
    fn build_map(&mut self, rng: &mut GameRng, build_data: &mut BuilderMap) {
        // Clone rooms to avoid borrow checker issues
        if let Some(rooms) = build_data.rooms.clone() {
            for room in rooms.iter().skip(1) {
                spawn_room_entities(build_data, &room, rng);
            }
        }
    }
}

fn spawn_room_entities(build_data: &mut BuilderMap, room: &crate::shapes::Rect, rng: &mut GameRng) {
    let num_spawns = rng.0.gen_range(0..=4);

    for _ in 0..num_spawns {
        let x = rng.0.gen_range(room.x1 + 1..room.x2);
        let y = rng.0.gen_range(room.y1 + 1..room.y2);
        let idx = build_data.map.xy_idx(x, y);

        if build_data.map.tiles[idx] == TileType::Floor {
            let roll = rng.0.gen_range(0..100);
            let name = if roll < 10 {
                "Health Potion"
            } else if roll < 20 {
                "Magic Missile Scroll"
            } else if roll < 60 {
                "Goblin"
            } else {
                "Orc"
            };
            build_data.spawn_list.push((idx, name.to_string()));
        }
    }
}
