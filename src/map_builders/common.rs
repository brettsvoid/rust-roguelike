use crate::map::{Map, TileType, MAP_HEIGHT, MAP_WIDTH};
use crate::shapes::Rect;

#[derive(Clone, Copy, PartialEq)]
pub enum Symmetry {
    None,
    Horizontal,
    Vertical,
    Both,
}

pub fn paint(map: &mut Map, symmetry: Symmetry, brush_size: i32, x: i32, y: i32) {
    match symmetry {
        Symmetry::None => {
            apply_paint(map, brush_size, x, y);
        }
        Symmetry::Horizontal => {
            let center_x = MAP_WIDTH as i32 / 2;
            if x == center_x {
                apply_paint(map, brush_size, x, y);
            } else {
                let dist = (center_x - x).abs();
                apply_paint(map, brush_size, center_x + dist, y);
                apply_paint(map, brush_size, center_x - dist, y);
            }
        }
        Symmetry::Vertical => {
            let center_y = MAP_HEIGHT as i32 / 2;
            if y == center_y {
                apply_paint(map, brush_size, x, y);
            } else {
                let dist = (center_y - y).abs();
                apply_paint(map, brush_size, x, center_y + dist);
                apply_paint(map, brush_size, x, center_y - dist);
            }
        }
        Symmetry::Both => {
            let center_x = MAP_WIDTH as i32 / 2;
            let center_y = MAP_HEIGHT as i32 / 2;
            let dist_x = (center_x - x).abs();
            let dist_y = (center_y - y).abs();
            apply_paint(map, brush_size, center_x + dist_x, center_y + dist_y);
            apply_paint(map, brush_size, center_x - dist_x, center_y + dist_y);
            apply_paint(map, brush_size, center_x + dist_x, center_y - dist_y);
            apply_paint(map, brush_size, center_x - dist_x, center_y - dist_y);
        }
    }
}

pub fn apply_paint(map: &mut Map, brush_size: i32, x: i32, y: i32) {
    for dy in -brush_size..=brush_size {
        for dx in -brush_size..=brush_size {
            let px = x + dx;
            let py = y + dy;
            if px > 0 && px < MAP_WIDTH as i32 - 1 && py > 0 && py < MAP_HEIGHT as i32 - 1 {
                let idx = map.xy_idx(px, py);
                map.tiles[idx] = TileType::Floor;
            }
        }
    }
}

pub fn apply_room_to_map(map: &mut Map, room: &Rect) {
    for y in room.y1 + 1..=room.y2 {
        for x in room.x1 + 1..=room.x2 {
            let idx = map.xy_idx(x, y);
            map.tiles[idx] = TileType::Floor;
        }
    }
}

pub fn apply_horizontal_tunnel(map: &mut Map, x1: i32, x2: i32, y: i32) -> Vec<usize> {
    let mut corridor = Vec::new();
    for x in x1.min(x2)..=x1.max(x2) {
        let idx = map.xy_idx(x, y);
        if idx < map.tiles.len() && map.tiles[idx] != TileType::Floor {
            map.tiles[idx] = TileType::Floor;
            corridor.push(idx);
        }
    }
    corridor
}

pub fn apply_vertical_tunnel(map: &mut Map, y1: i32, y2: i32, x: i32) -> Vec<usize> {
    let mut corridor = Vec::new();
    for y in y1.min(y2)..=y1.max(y2) {
        let idx = map.xy_idx(x, y);
        if idx < map.tiles.len() && map.tiles[idx] != TileType::Floor {
            map.tiles[idx] = TileType::Floor;
            corridor.push(idx);
        }
    }
    corridor
}

pub fn draw_corridor(map: &mut Map, x1: i32, y1: i32, x2: i32, y2: i32) -> Vec<usize> {
    let mut corridor = Vec::new();
    let mut x = x1;
    let mut y = y1;

    while x != x2 || y != y2 {
        if x < x2 {
            x += 1;
        } else if x > x2 {
            x -= 1;
        } else if y < y2 {
            y += 1;
        } else if y > y2 {
            y -= 1;
        }

        let idx = map.xy_idx(x, y);
        if map.tiles[idx] != TileType::Floor {
            map.tiles[idx] = TileType::Floor;
            corridor.push(idx);
        }
    }
    corridor
}

/// Bresenham line algorithm for diagonal corridors
pub fn bresenham_line(x1: i32, y1: i32, x2: i32, y2: i32) -> Vec<(i32, i32)> {
    let mut points = Vec::new();
    let dx = (x2 - x1).abs();
    let dy = (y2 - y1).abs();
    let sx = if x1 < x2 { 1 } else { -1 };
    let sy = if y1 < y2 { 1 } else { -1 };
    let mut err = dx - dy;
    let mut x = x1;
    let mut y = y1;

    loop {
        points.push((x, y));
        if x == x2 && y == y2 {
            break;
        }
        let e2 = 2 * err;
        if e2 > -dy {
            err -= dy;
            x += sx;
        }
        if e2 < dx {
            err += dx;
            y += sy;
        }
    }
    points
}

/// Draw corridor using Bresenham line algorithm
pub fn draw_corridor_bresenham(map: &mut Map, x1: i32, y1: i32, x2: i32, y2: i32) -> Vec<usize> {
    let mut corridor = Vec::new();
    for (x, y) in bresenham_line(x1, y1, x2, y2) {
        let idx = map.xy_idx(x, y);
        if map.tiles[idx] != TileType::Floor {
            map.tiles[idx] = TileType::Floor;
            corridor.push(idx);
        }
    }
    corridor
}
