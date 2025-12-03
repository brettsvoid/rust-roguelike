use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

use crate::map::Map;

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
