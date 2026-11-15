//! Action point rules for combat, as pure functions over plain data: what each
//! action costs (`Action::spend`), where a unit can move on its AP
//! (`move_range`) and where a confused unit stumbles (`confused_step`). The
//! systems that apply them live in `game_flow`, `player` and `monsters`.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap};

use bevy::prelude::*;

use crate::map::Map;

/// Action points every unit gets at the start of its side's phase.
pub const AP_PER_ROUND: u8 = 2;

/// Tiles per action point for heroes, and for monsters whose raws set none.
pub const DEFAULT_MOVEMENT: i32 = 4;

/// What a unit spends its action points on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Move within one-AP range (also opening a door).
    Move,
    /// Move within two-AP range.
    Dash,
    /// Melee; ends the unit's turn.
    Attack,
    /// Use, drop, remove or pick up an item.
    UseItem,
    /// End this unit's turn, keeping nothing.
    Wait,
}

impl Action {
    /// The move that reaches a tile costing `ap`.
    pub fn moving(ap: u8) -> Action {
        if ap >= 2 { Action::Dash } else { Action::Move }
    }

    /// The AP left after taking this action with `ap`, or `None` if the unit
    /// cannot afford it. Attacking and waiting need at least one AP and spend
    /// the rest.
    pub fn spend(self, ap: u8) -> Option<u8> {
        match self {
            Action::Move | Action::UseItem => ap.checked_sub(1),
            Action::Dash => ap.checked_sub(2),
            Action::Attack | Action::Wait => (ap > 0).then_some(0),
        }
    }
}

/// Where a unit can move this turn: each reachable tile with its AP cost and
/// the tile it is entered from. The unit's own tile is not in it.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct MoveRange {
    cells: BTreeMap<(i32, i32), (u8, (i32, i32))>,
}

impl MoveRange {
    /// The AP it costs to move to `cell`, `None` out of range.
    pub fn ap(&self, cell: (i32, i32)) -> Option<u8> {
        self.cells.get(&cell).map(|c| c.0)
    }

    /// Every reachable tile with its AP cost, in a fixed order.
    pub fn cells(&self) -> impl Iterator<Item = ((i32, i32), u8)> + '_ {
        self.cells.iter().map(|(&cell, &(ap, _))| (cell, ap))
    }

    /// The cheapest path to `cell`, without the start; empty out of range.
    pub fn path(&self, cell: (i32, i32)) -> Vec<(i32, i32)> {
        let mut path = Vec::new();
        let mut at = cell;
        while let Some(&(_, prev)) = self.cells.get(&at) {
            path.push(at);
            at = prev;
        }
        path.reverse();
        path
    }
}

/// The tiles a unit at `from` reaches on `ap` action points of `movement`
/// tiles each: a cheapest-first flood fill over `Map::get_available_exits`, so
/// walls and blocking entities stop it and a diagonal step costs 1.45 like the
/// pathfinder. `open` rules out more tiles (other heroes, unexplored ground).
pub fn move_range(map: &Map, from: (i32, i32), movement: i32, ap: u8, open: impl Fn((i32, i32)) -> bool) -> MoveRange {
    // Hundredths of a tile, so costs stay whole numbers.
    let per_ap = movement.max(1) as u32 * 100;
    let budget = per_ap * ap as u32;
    let mut cost = BTreeMap::from([(from, 0)]);
    let mut cells = BTreeMap::new();
    let mut frontier = BinaryHeap::from([Reverse((0, from))]);
    while let Some(Reverse((spent, at))) = frontier.pop() {
        if spent > cost[&at] {
            continue;
        }
        for (idx, step) in map.get_available_exits(map.xy_idx(at.0, at.1)) {
            let next = (idx as i32 % map.width, idx as i32 / map.width);
            let total = spent + (step * 100.0).round() as u32;
            if total > budget || cost.get(&next).is_some_and(|&c| c <= total) || !open(next) {
                continue;
            }
            cost.insert(next, total);
            cells.insert(next, (total.div_ceil(per_ap) as u8, at));
            frontier.push(Reverse((total, next)));
        }
    }
    MoveRange { cells }
}

/// Where a confused unit at `from` stumbles: one of its exits that `open`
/// allows, picked by `roll(count)`. `None` when it is hemmed in.
pub fn confused_step(
    map: &Map,
    from: (i32, i32),
    open: impl Fn((i32, i32)) -> bool,
    roll: impl FnOnce(usize) -> usize,
) -> Option<(i32, i32)> {
    let exits: Vec<_> = map
        .get_available_exits(map.xy_idx(from.0, from.1))
        .into_iter()
        .map(|(idx, _)| (idx as i32 % map.width, idx as i32 / map.width))
        .filter(|&cell| open(cell))
        .collect();
    (!exits.is_empty()).then(|| exits[roll(exits.len())])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::TileType;

    /// A `w` x `h` map: walls round the edge, floor inside, nothing blocked
    /// but the walls.
    fn open_map(w: i32, h: i32) -> Map {
        let mut map = Map::new(w as usize, h as usize, 2);
        for y in 1..h - 1 {
            for x in 1..w - 1 {
                let idx = map.xy_idx(x, y);
                map.tiles[idx] = TileType::Floor;
            }
        }
        map.populate_blocked();
        map
    }

    fn block(map: &mut Map, cells: &[(i32, i32)]) {
        for &(x, y) in cells {
            let idx = map.xy_idx(x, y);
            map.blocked_tiles[idx] = true;
        }
    }

    #[test]
    fn each_action_costs_its_ap() {
        assert_eq!(Action::Move.spend(2), Some(1));
        assert_eq!(Action::Move.spend(1), Some(0));
        assert_eq!(Action::Move.spend(0), None);
        assert_eq!(Action::UseItem.spend(2), Some(1));
        assert_eq!(Action::Dash.spend(2), Some(0));
        assert_eq!(Action::Dash.spend(1), None);
        // Attacking and waiting end the turn whatever is left.
        assert_eq!(Action::Attack.spend(2), Some(0));
        assert_eq!(Action::Attack.spend(1), Some(0));
        assert_eq!(Action::Attack.spend(0), None);
        assert_eq!(Action::Wait.spend(2), Some(0));
        assert_eq!(Action::moving(1), Action::Move);
        assert_eq!(Action::moving(2), Action::Dash);
    }

    #[test]
    fn straight_lines_cost_one_ap_up_to_movement_tiles_and_two_up_to_double() {
        let map = open_map(20, 5);
        let range = move_range(&map, (1, 2), 3, 2, |_| true);
        let along: Vec<_> = (2..10).map(|x| range.ap((x, 2))).collect();
        assert_eq!(along, [Some(1), Some(1), Some(1), Some(2), Some(2), Some(2), None, None]);
        assert_eq!(range.ap((1, 2)), None, "the unit's own tile is not a move");
        // With one AP left only the one-AP zone remains.
        let range = move_range(&map, (1, 2), 3, 1, |_| true);
        assert_eq!((range.ap((4, 2)), range.ap((5, 2))), (Some(1), None));
    }

    #[test]
    fn diagonals_cost_what_the_pathfinder_charges() {
        let map = open_map(12, 12);
        let range = move_range(&map, (5, 5), 3, 2, |_| true);
        // Two diagonals cost 2.9, inside 3; a third costs 4.35, so a dash.
        assert_eq!(range.ap((7, 7)), Some(1));
        assert_eq!(range.ap((8, 8)), Some(2));
        // Four diagonals, 5.8, still fit in a dash of 6; five do not.
        assert_eq!(range.ap((9, 9)), Some(2));
        assert_eq!(range.ap((10, 10)), None);
        // One diagonal and two straight: 3.45, just over one AP.
        assert_eq!(range.ap((8, 6)), Some(2));
    }

    #[test]
    fn walls_blocked_tiles_and_closed_off_cells_bend_or_stop_the_range() {
        let mut map = open_map(9, 5);
        // A wall across x = 4 with a gap at y = 3, and a monster at (2, 2).
        for y in 1..3 {
            let idx = map.xy_idx(4, y);
            map.tiles[idx] = TileType::Wall;
        }
        map.populate_blocked();
        block(&mut map, &[(2, 2)]);
        let range = move_range(&map, (1, 1), 4, 1, |cell| cell != (3, 3));
        assert_eq!(range.ap((4, 1)), None, "wall");
        assert_eq!(range.ap((2, 2)), None, "blocked by a monster");
        assert_eq!(range.ap((3, 3)), None, "closed off by `open`");
        // (3, 1) is two straight steps along the top row.
        assert_eq!(range.ap((3, 1)), Some(1));
        assert_eq!(range.path((3, 1)), vec![(2, 1), (3, 1)]);
        // Past the wall only through the gap, too far for one AP.
        assert_eq!(range.ap((5, 1)), None);
        let range = move_range(&map, (1, 1), 4, 2, |cell| cell != (3, 3));
        assert_eq!(range.ap((5, 1)), Some(2));
        assert_eq!(range.path((5, 1)).last(), Some(&(5, 1)));
        assert!(range.path((5, 1)).contains(&(4, 3)), "through the gap");
    }

    #[test]
    fn a_confused_unit_stumbles_to_a_rolled_open_neighbour() {
        let mut map = open_map(5, 5);
        block(&mut map, &[(1, 2)]);
        let open = |cell: (i32, i32)| cell != (3, 2);
        // Exits in `get_available_exits` order minus the blocked and closed:
        // (2, 1), (2, 3), (1, 1), (3, 1), (1, 3), (3, 3).
        assert_eq!(confused_step(&map, (2, 2), open, |n| { assert_eq!(n, 6); 0 }), Some((2, 1)));
        assert_eq!(confused_step(&map, (2, 2), open, |_| 5), Some((3, 3)));
        assert_eq!(confused_step(&map, (2, 2), |_| false, |_| 0), None);
    }
}
