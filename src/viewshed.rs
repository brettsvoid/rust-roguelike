use bevy::prelude::*;

use crate::map::{tile_opaque, Map, Position};

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

/// Walls, entities with `BlocksVisibility` and anything off the map block sight.
fn opaque(map: &Map, x: i32, y: i32) -> bool {
    if x < 0 || x >= map.width || y < 0 || y >= map.height {
        return true;
    }
    let idx = map.xy_idx(x, y);
    tile_opaque(map.tiles[idx]) || map.view_blocked.contains(&idx)
}

/// Whether nothing opaque stands between `from` and `to` on the Bresenham
/// line, drawn both ways: the line can pass different cells each way, and a
/// shot one way must also be possible back. The two end tiles do not count.
pub fn line_of_sight(map: &Map, from: (i32, i32), to: (i32, i32)) -> bool {
    let clear = |a: (i32, i32), b: (i32, i32)| {
        let line = bresenham_line(a.0, a.1, b.0, b.1);
        let between = line.len().saturating_sub(2);
        !line.iter().skip(1).take(between).any(|&(x, y)| opaque(map, x, y))
    };
    clear(from, to) && clear(to, from)
}

pub fn update_viewshed(map: Res<Map>, mut query: Query<(&Position, &mut Viewshed)>) {
    let is_opaque = |x: i32, y: i32| opaque(&map, x, y);

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::TileType;

    #[test]
    fn line_of_sight_is_the_same_both_ways() {
        let mut map = Map::new(5, 5, 1);
        for idx in 0..map.tiles.len() {
            map.tiles[idx] = TileType::Floor;
        }
        // Bresenham from (1, 1) to (3, 2) passes (2, 2); back it passes (2, 1).
        assert!(bresenham_line(1, 1, 3, 2).contains(&(2, 2)) && bresenham_line(3, 2, 1, 1).contains(&(2, 1)));
        assert!(line_of_sight(&map, (1, 1), (3, 2)));
        let idx = map.xy_idx(2, 2);
        map.tiles[idx] = TileType::Wall;
        assert!(!line_of_sight(&map, (1, 1), (3, 2)));
        assert!(!line_of_sight(&map, (3, 2), (1, 1)));
    }
}
