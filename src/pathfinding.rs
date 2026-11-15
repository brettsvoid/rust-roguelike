use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

use crate::map::{Map, TileType, MAP_HEIGHT, MAP_WIDTH};

#[derive(Clone, Eq, PartialEq)]
struct Node {
    idx: usize,
    f_score: i32, // g + h in hundredths, so a diagonal (1.45) never ties a straight step
}

impl Ord for Node {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse ordering for min-heap (lowest f_score first)
        other.f_score.cmp(&self.f_score)
    }
}

impl PartialOrd for Node {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// A* pathfinding algorithm. Returns path from start to end (inclusive), or None if no path exists.
// Toolbox: monsters currently use a_star_ignoring_entities; this variant
// respects entity blocking.
#[allow(dead_code)]
pub fn a_star(map: &Map, start: usize, end: usize) -> Option<Vec<usize>> {
    a_star_with(map, start, end, |idx| map.get_available_exits(idx))
}

/// A* pathfinding that ignores entities (only considers walls as obstacles).
/// Use this for AI pathing so monsters can path through each other's positions.
pub fn a_star_ignoring_entities(map: &Map, start: usize, end: usize) -> Option<Vec<usize>> {
    a_star_with(map, start, end, |idx| map.get_available_exits_ignoring_entities(idx))
}

/// A* over walls only, through tiles the player has seen, so a click-to-move
/// path never gives away unexplored rooms.
pub fn a_star_explored(map: &Map, start: usize, end: usize) -> Option<Vec<usize>> {
    a_star_explored_avoiding(map, start, end, |_| false)
}

/// `a_star_explored` that also keeps off the tiles `avoid` names.
pub fn a_star_explored_avoiding(
    map: &Map,
    start: usize,
    end: usize,
    avoid: impl Fn(usize) -> bool,
) -> Option<Vec<usize>> {
    a_star_with(map, start, end, |idx| {
        let mut exits = map.get_available_exits_ignoring_entities(idx);
        exits.retain(|&(next, _)| map.revealed_tiles[next] && !avoid(next));
        exits
    })
}

fn a_star_with(
    map: &Map,
    start: usize,
    end: usize,
    exits: impl Fn(usize) -> Vec<(usize, f32)>,
) -> Option<Vec<usize>> {
    let mut open_set = BinaryHeap::new();
    let mut came_from: HashMap<usize, usize> = HashMap::new();
    let mut g_score: HashMap<usize, f32> = HashMap::new();

    g_score.insert(start, 0.0);
    open_set.push(Node {
        idx: start,
        f_score: (map.get_pathing_distance(start, end) * 100.0) as i32,
    });

    while let Some(current) = open_set.pop() {
        if current.idx == end {
            // Reconstruct path
            let mut path = vec![current.idx];
            let mut current_idx = current.idx;
            while let Some(&prev) = came_from.get(&current_idx) {
                path.push(prev);
                current_idx = prev;
            }
            path.reverse();
            return Some(path);
        }

        let current_g = *g_score.get(&current.idx).unwrap_or(&f32::INFINITY);

        for (neighbor_idx, cost) in exits(current.idx) {
            let tentative_g = current_g + cost;
            let neighbor_g = *g_score.get(&neighbor_idx).unwrap_or(&f32::INFINITY);

            if tentative_g < neighbor_g {
                came_from.insert(neighbor_idx, current.idx);
                g_score.insert(neighbor_idx, tentative_g);

                let h = map.get_pathing_distance(neighbor_idx, end);
                let f = tentative_g + h;

                open_set.push(Node {
                    idx: neighbor_idx,
                    f_score: (f * 100.0) as i32,
                });
            }
        }
    }

    None // No path found
}

/// Dijkstra map - computes distances from start position(s) to all reachable tiles.
/// Returns a vector of distances (f32::MAX for unreachable tiles).
pub fn dijkstra_map(map: &Map, starts: &[usize]) -> Vec<f32> {
    let mut distances = vec![f32::MAX; map.tiles.len()];
    let mut open = Vec::new();

    for &start in starts {
        distances[start] = 0.0;
        open.push(start);
    }

    while let Some(current) = open.pop() {
        let current_dist = distances[current];
        let cx = (current % MAP_WIDTH) as i32;
        let cy = (current / MAP_WIDTH) as i32;

        for dy in -1..=1i32 {
            for dx in -1..=1i32 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let nx = cx + dx;
                let ny = cy + dy;
                if nx < 0 || nx >= MAP_WIDTH as i32 || ny < 0 || ny >= MAP_HEIGHT as i32 {
                    continue;
                }
                let neighbor_idx = map.xy_idx(nx, ny);
                if map.tiles[neighbor_idx] != TileType::Wall {
                    let new_dist = current_dist + 1.0;
                    if new_dist < distances[neighbor_idx] {
                        distances[neighbor_idx] = new_dist;
                        open.push(neighbor_idx);
                    }
                }
            }
        }
    }

    distances
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 7x5 room of floor walled round the edge, all of it seen.
    fn room() -> Map {
        let mut map = Map::new(7, 5, 2);
        for y in 1..4 {
            for x in 1..6 {
                let idx = map.xy_idx(x, y);
                map.tiles[idx] = TileType::Floor;
            }
        }
        map.revealed_tiles.fill(true);
        map
    }

    fn cells(map: &Map, path: &[usize]) -> Vec<(i32, i32)> {
        path.iter().map(|&i| (i as i32 % map.width, i as i32 / map.width)).collect()
    }

    #[test]
    fn explored_path_runs_straight_across_an_open_room() {
        let map = room();
        let path = a_star_explored(&map, map.xy_idx(1, 2), map.xy_idx(5, 2)).unwrap();
        assert_eq!(cells(&map, &path), vec![(1, 2), (2, 2), (3, 2), (4, 2), (5, 2)]);
    }

    #[test]
    fn explored_path_goes_round_unseen_tiles() {
        let mut map = room();
        for y in 1..3 {
            let idx = map.xy_idx(3, y);
            map.revealed_tiles[idx] = false;
        }
        let path = a_star_explored(&map, map.xy_idx(1, 1), map.xy_idx(5, 1)).unwrap();
        assert_eq!(cells(&map, &path), vec![(1, 1), (2, 2), (3, 3), (4, 2), (5, 1)]);
    }

    #[test]
    fn no_explored_path_when_unseen_tiles_cut_the_room() {
        let mut map = room();
        for y in 1..4 {
            let idx = map.xy_idx(3, y);
            map.revealed_tiles[idx] = false;
        }
        assert_eq!(a_star_explored(&map, map.xy_idx(1, 2), map.xy_idx(5, 2)), None);
    }
}
