use bevy::prelude::*;

use crate::map::{Map, Position, TileType};

#[derive(Component, Default, Debug)]
pub struct Viewshed {
    pub range: i32,
    pub visible_tiles: Vec<(i32, i32)>,
    pub dirty: bool,
}

pub struct ViewshedPlugin;

impl Plugin for ViewshedPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, update_viewshed);
    }
}

fn update_viewshed(map: Res<Map>, mut query: Query<(&Position, &mut Viewshed)>) {
    let is_opaque = |x: i32, y: i32| {
        let idx = map.xy_idx(x, y);
        matches!(map.tiles[idx], TileType::Wall)
    };

    for (pos, mut viewshed) in &mut query {
        viewshed.visible_tiles.clear();
        viewshed.visible_tiles = calculate_fov(pos.x, pos.y, viewshed.range, is_opaque);
        viewshed
            .visible_tiles
            .retain(|p| p.0 >= 0 && p.0 < map.width && p.1 >= 0 && p.1 < map.height);
    }
}

/// Returns a vector of all points on a line from (x0, y0) to (x1, y1).
pub fn bresenham_line(x0: i32, y0: i32, x1: i32, y1: i32) -> Vec<(i32, i32)> {
    let mut points = Vec::new();
    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;

    let (mut x, mut y) = (x0, y0);

    loop {
        points.push((x, y));
        if x == x1 && y == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }

    points
}

/// Calculates FOV around (cx, cy) within the given radius.
/// `is_opaque` is a closure that tells us if a cell is blocking.
fn calculate_fov(
    cx: i32,
    cy: i32,
    radius: i32,
    is_opaque: impl Fn(i32, i32) -> bool,
) -> Vec<(i32, i32)> {
    let mut visible_cells = Vec::new();

    for x in (cx - radius)..=(cx + radius) {
        for y in (cy - radius)..=(cy + radius) {
            // Check circular boundary.
            let dx = x - cx;
            let dy = y - cy;
            if dx * dx + dy * dy <= radius * radius {
                // Get points on the line from center to this cell.
                let line = bresenham_line(cx, cy, x, y);
                let mut blocked = false;
                for &(lx, ly) in &line {
                    if is_opaque(lx, ly) {
                        // Mark the opaque cell (lx, ly) itself as visible (omit it if we don't
                        // want the opaque cell to be considered visible).
                        visible_cells.push((lx, ly));
                        blocked = true;
                        break;
                    } else {
                        // If not blocked, keep it in FOV.
                        visible_cells.push((lx, ly));
                    }
                }
                // Once blocked, no further cells on that line are added.
                if blocked {
                    continue;
                }
            }
        }
    }

    visible_cells
}
