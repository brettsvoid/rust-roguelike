use rand::Rng;

use crate::map::{Map, TileType, MAP_HEIGHT, MAP_WIDTH};
use crate::rng::GameRng;
use crate::shapes::Rect;

use super::{BuilderMap, MetaMapBuilder};

// ============================================================================
// RoomExploder - Spawn drunkard walks from room centers
// ============================================================================

pub struct RoomExploder;

impl RoomExploder {
    pub fn new() -> Box<Self> {
        Box::new(Self)
    }
}

impl MetaMapBuilder for RoomExploder {
    fn build_map(&mut self, rng: &mut GameRng, build_data: &mut BuilderMap) {
        if let Some(rooms) = build_data.rooms.clone() {
            for room in rooms.iter() {
                let (start_x, start_y) = room.center();

                // Spawn 1-3 random walkers per room
                let num_walkers = rng.0.gen_range(1..=3);
                for _ in 0..num_walkers {
                    let mut x = start_x;
                    let mut y = start_y;

                    // 20-step drunkard walk
                    for _ in 0..20 {
                        let direction = rng.0.gen_range(0..4);
                        match direction {
                            0 if y > 1 => y -= 1,
                            1 if y < MAP_HEIGHT as i32 - 2 => y += 1,
                            2 if x > 1 => x -= 1,
                            3 if x < MAP_WIDTH as i32 - 2 => x += 1,
                            _ => {}
                        }

                        let idx = build_data.map.xy_idx(x, y);
                        build_data.map.tiles[idx] = TileType::Floor;
                    }
                }
            }
        }
        build_data.take_snapshot();
    }
}

// ============================================================================
// RoomCornerRounder - Smooth rectangular room corners
// ============================================================================

pub struct RoomCornerRounder;

impl RoomCornerRounder {
    pub fn new() -> Box<Self> {
        Box::new(Self)
    }

    fn fill_if_corner(&self, build_data: &mut BuilderMap, x: i32, y: i32) {
        if x < 1 || x >= MAP_WIDTH as i32 - 1 || y < 1 || y >= MAP_HEIGHT as i32 - 1 {
            return;
        }

        let idx = build_data.map.xy_idx(x, y);
        if build_data.map.tiles[idx] == TileType::Floor {
            // Count adjacent walls (4-directional)
            let mut wall_count = 0;
            let neighbors = [
                build_data.map.xy_idx(x - 1, y),
                build_data.map.xy_idx(x + 1, y),
                build_data.map.xy_idx(x, y - 1),
                build_data.map.xy_idx(x, y + 1),
            ];

            for n_idx in neighbors {
                if build_data.map.tiles[n_idx] == TileType::Wall {
                    wall_count += 1;
                }
            }

            // If exactly 2 adjacent walls, this is a corner - fill it
            if wall_count == 2 {
                build_data.map.tiles[idx] = TileType::Wall;
            }
        }
    }
}

impl MetaMapBuilder for RoomCornerRounder {
    fn build_map(&mut self, _rng: &mut GameRng, build_data: &mut BuilderMap) {
        if let Some(rooms) = build_data.rooms.clone() {
            for room in rooms.iter() {
                // Check all corner tiles of this room
                // Inner corners (just inside the room boundaries)
                self.fill_if_corner(build_data, room.x1 + 1, room.y1 + 1);
                self.fill_if_corner(build_data, room.x2, room.y1 + 1);
                self.fill_if_corner(build_data, room.x1 + 1, room.y2);
                self.fill_if_corner(build_data, room.x2, room.y2);
            }
        }
        build_data.take_snapshot();
    }
}

// ============================================================================
// RoomDrawer - Redraws rooms with configurable shapes
// ============================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoomShape {
    Rectangle,
    Circle,
}

pub struct RoomDrawer {
    shape: RoomShape,
}

impl RoomDrawer {
    pub fn new() -> Box<Self> {
        Box::new(Self {
            shape: RoomShape::Rectangle,
        })
    }

    pub fn circles() -> Box<Self> {
        Box::new(Self {
            shape: RoomShape::Circle,
        })
    }

    fn draw_rectangle(map: &mut Map, room: &Rect) {
        for y in room.y1 + 1..=room.y2 {
            for x in room.x1 + 1..=room.x2 {
                let idx = map.xy_idx(x, y);
                map.tiles[idx] = TileType::Floor;
            }
        }
    }

    fn draw_circle(map: &mut Map, room: &Rect) {
        let (center_x, center_y) = room.center();
        let radius = i32::min(room.x2 - room.x1, room.y2 - room.y1) as f32 / 2.0;

        for y in room.y1..=room.y2 {
            for x in room.x1..=room.x2 {
                let dx = x - center_x;
                let dy = y - center_y;
                let distance = ((dx * dx + dy * dy) as f32).sqrt();

                if distance <= radius {
                    let idx = map.xy_idx(x, y);
                    map.tiles[idx] = TileType::Floor;
                }
            }
        }
    }
}

impl MetaMapBuilder for RoomDrawer {
    fn build_map(&mut self, _rng: &mut GameRng, build_data: &mut BuilderMap) {
        if let Some(rooms) = build_data.rooms.clone() {
            for room in rooms.iter() {
                match self.shape {
                    RoomShape::Rectangle => Self::draw_rectangle(&mut build_data.map, room),
                    RoomShape::Circle => Self::draw_circle(&mut build_data.map, room),
                }
            }
        }
        build_data.take_snapshot();
    }
}
