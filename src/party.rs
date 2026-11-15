//! The party: the heroes the player commands, one of them the active unit.
//!
//! Every hero carries `PartyMember`. `ActiveUnit` names the one that moves and
//! acts on the player's input; exploring, the others follow it (`follow`). Vision is the union of
//! every living member's viewshed. A dead member is despawned; the run ends
//! when none is left.

use bevy::color::palettes::basic;
use bevy::prelude::*;

use crate::map::{tile_walkable, Map};

pub const PARTY_SIZE: usize = 4;

/// Name and colour of each hero, by slot.
pub const HEROES: [(&str, Srgba); PARTY_SIZE] = [
    ("Aldric", basic::YELLOW),
    ("Brenna", basic::AQUA),
    ("Corin", basic::LIME),
    ("Dagny", basic::FUCHSIA),
];

/// A hero in the party. `slot` fixes the order for the HUD and Tab.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct PartyMember {
    pub slot: usize,
}

/// The member the player commands. `None` outside a run.
#[derive(Resource, Default, Debug)]
pub struct ActiveUnit(pub Option<Entity>);

pub struct PartyPlugin;

impl Plugin for PartyPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ActiveUnit>();
    }
}

/// Up to `count` walkable tiles for the party: `start` first, then the
/// nearest walkable tiles by breadth-first search in a fixed order.
pub fn spawn_tiles(map: &Map, start: (i32, i32), count: usize) -> Vec<(i32, i32)> {
    const STEPS: [(i32, i32); 8] = [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (-1, 1), (1, -1), (-1, -1)];
    let in_map = |(x, y): (i32, i32)| x >= 0 && y >= 0 && x < map.width && y < map.height;
    let mut found = vec![start];
    let mut i = 0;
    while found.len() < count && i < found.len() {
        let (x, y) = found[i];
        for (dx, dy) in STEPS {
            let next = (x + dx, y + dy);
            if found.len() < count
                && in_map(next)
                && tile_walkable(map.tiles[map.xy_idx(next.0, next.1)])
                && !found.contains(&next)
            {
                found.push(next);
            }
        }
        i += 1;
    }
    found
}

/// The living member after `current` in slot order, wrapping round. Members
/// are `(slot, id, alive)`. Without a current member, the first living one.
pub fn next_unit<T: Copy + Eq>(members: &[(usize, T, bool)], current: Option<T>) -> Option<T> {
    let current_slot = current.and_then(|c| members.iter().find(|m| m.1 == c)).map(|m| m.0);
    let living = || members.iter().filter(|m| m.2);
    current_slot
        .and_then(|s| living().filter(|m| m.0 > s).min_by_key(|m| m.0))
        .or_else(|| living().min_by_key(|m| m.0))
        .map(|m| m.1)
}

/// Where the followers stand after the leader stepped from `leader_from` to
/// `leader_to`, in the order given. The line forms nearest first (ties keep
/// the given order); each takes one `step_towards` the cell the hero ahead of
/// it stood on, so the party trails the leader in single file. A follower
/// waits rather than step onto another hero.
pub fn follow(
    leader_from: (i32, i32),
    leader_to: (i32, i32),
    followers: &[(i32, i32)],
    step_towards: impl Fn((i32, i32), (i32, i32)) -> Option<(i32, i32)>,
) -> Vec<(i32, i32)> {
    let mut cells = followers.to_vec();
    let mut line: Vec<usize> = (0..cells.len()).collect();
    line.sort_by_key(|&i| (cells[i].0 - leader_from.0).abs().max((cells[i].1 - leader_from.1).abs()));
    let mut ahead = leader_from;
    for i in line {
        let from = cells[i];
        let free = |cell: &(i32, i32)| *cell != leader_to && !cells.contains(cell);
        if let Some(next) = step_towards(from, ahead).filter(free) {
            cells[i] = next;
        }
        ahead = from;
    }
    cells
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::TileType;

    #[test]
    fn the_party_stands_on_and_round_the_start() {
        let mut map = Map::new(5, 5, 2);
        for (x, y) in [(2, 2), (3, 2), (1, 2), (2, 3), (3, 3)] {
            let idx = map.xy_idx(x, y);
            map.tiles[idx] = TileType::Floor;
        }
        assert_eq!(spawn_tiles(&map, (2, 2), 4), vec![(2, 2), (3, 2), (1, 2), (2, 3)]);
    }

    #[test]
    fn a_corridor_start_lines_the_party_up_along_it() {
        let mut map = Map::new(7, 3, 2);
        for x in 1..6 {
            let idx = map.xy_idx(x, 1);
            map.tiles[idx] = TileType::Floor;
        }
        assert_eq!(spawn_tiles(&map, (1, 1), 4), vec![(1, 1), (2, 1), (3, 1), (4, 1)]);
        // Fewer free tiles than heroes: only those come back.
        assert_eq!(spawn_tiles(&map, (1, 1), 9).len(), 5);
    }

    #[test]
    fn tab_cycles_through_the_living_in_slot_order() {
        let party = [(2, 'c', true), (0, 'a', true), (1, 'b', false), (3, 'd', true)];
        assert_eq!(next_unit(&party, Some('a')), Some('c'));
        assert_eq!(next_unit(&party, Some('c')), Some('d'));
        assert_eq!(next_unit(&party, Some('d')), Some('a'));
        // From a member who just died, on to the next living one.
        assert_eq!(next_unit(&party, Some('b')), Some('c'));
        assert_eq!(next_unit(&party, None), Some('a'));
        assert_eq!(next_unit(&[(0, 'a', false)], Some('a')), None);
    }

    /// One king's move from `from` towards `to` on an open floor.
    fn straight_at(from: (i32, i32), to: (i32, i32)) -> Option<(i32, i32)> {
        let next = (from.0 + (to.0 - from.0).signum(), from.1 + (to.1 - from.1).signum());
        (next != from).then_some(next)
    }

    #[test]
    fn followers_trail_the_leader_in_single_file() {
        // Leader steps (3,0) -> (4,0) with the line strung out behind.
        let cells = follow((3, 0), (4, 0), &[(2, 0), (1, 0), (0, 0)], straight_at);
        assert_eq!(cells, vec![(3, 0), (2, 0), (1, 0)]);
    }

    #[test]
    fn a_follower_closes_in_from_afar_and_never_shares_a_cell() {
        // The nearer follower leads the line whatever its slot.
        let cells = follow((5, 5), (6, 5), &[(5, 7), (5, 6)], straight_at);
        assert_eq!(cells, vec![(5, 6), (5, 5)]);
        // A follower swapped onto the leader's old cell stays; one far off closes in.
        let cells = follow((1, 1), (2, 1), &[(1, 1), (8, 1)], straight_at);
        assert_eq!(cells, vec![(1, 1), (7, 1)]);
        // Blocked by the map: it waits.
        assert_eq!(follow((1, 1), (2, 1), &[(0, 1)], |_, _| None), vec![(0, 1)]);
    }
}
