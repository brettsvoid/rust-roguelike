use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

use crate::map::{Map, TileType, MAP_HEIGHT, MAP_WIDTH};

#[derive(Clone, Eq, PartialEq)]
struct Node {
    idx: usize,
    f_score: i32, // g + h
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
pub fn a_star(map: &Map, start: usize, end: usize) -> Option<Vec<usize>> {
    let mut open_set = BinaryHeap::new();
    let mut came_from: HashMap<usize, usize> = HashMap::new();
    let mut g_score: HashMap<usize, f32> = HashMap::new();

    g_score.insert(start, 0.0);
    open_set.push(Node {
        idx: start,
        f_score: map.get_pathing_distance(start, end) as i32,
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

        for (neighbor_idx, cost) in map.get_available_exits(current.idx) {
            let tentative_g = current_g + cost;
            let neighbor_g = *g_score.get(&neighbor_idx).unwrap_or(&f32::INFINITY);

            if tentative_g < neighbor_g {
                came_from.insert(neighbor_idx, current.idx);
                g_score.insert(neighbor_idx, tentative_g);

                let h = map.get_pathing_distance(neighbor_idx, end);
                let f = tentative_g + h;

                open_set.push(Node {
                    idx: neighbor_idx,
                    f_score: f as i32,
                });
            }
        }
    }

    None // No path found
}

/// A* pathfinding that ignores entities (only considers walls as obstacles).
/// Use this for AI pathing so monsters can path through each other's positions.
pub fn a_star_ignoring_entities(map: &Map, start: usize, end: usize) -> Option<Vec<usize>> {
    let mut open_set = BinaryHeap::new();
    let mut came_from: HashMap<usize, usize> = HashMap::new();
    let mut g_score: HashMap<usize, f32> = HashMap::new();

    g_score.insert(start, 0.0);
    open_set.push(Node {
        idx: start,
        f_score: map.get_pathing_distance(start, end) as i32,
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

        for (neighbor_idx, cost) in map.get_available_exits_ignoring_entities(current.idx) {
            let tentative_g = current_g + cost;
            let neighbor_g = *g_score.get(&neighbor_idx).unwrap_or(&f32::INFINITY);

            if tentative_g < neighbor_g {
                came_from.insert(neighbor_idx, current.idx);
                g_score.insert(neighbor_idx, tentative_g);

                let h = map.get_pathing_distance(neighbor_idx, end);
                let f = tentative_g + h;

                open_set.push(Node {
                    idx: neighbor_idx,
                    f_score: f as i32,
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
