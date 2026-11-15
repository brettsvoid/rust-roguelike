//! Cover and flanking: pure rules over the map. Walls and anything that
//! blocks sight (a closed door) give full cover; props give their raws
//! `cover`, indexed by cell in `Map::prop_cover`. Units give none. Cover is per
//! side: an object on the target's north, south, east or west side covers it
//! only from attackers beyond that side.

use bevy::prelude::*;
use serde::Deserialize;

use crate::map::{tile_opaque, Map};

/// How much an object shelters whoever stands beside it. Also the component a
/// prop carries (from the raws `cover`); a prop without one gives none.
#[derive(Component, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum Cover {
    #[default]
    None,
    Half,
    Full,
}

/// Aim taken off a shot at a target in half or full cover (XCOM's numbers).
pub const HALF_COVER_PENALTY: i32 = 20;
pub const FULL_COVER_PENALTY: i32 = 40;
/// Crit chance added against a flanked target.
pub const FLANK_CRIT_BONUS: i32 = 30;

impl Cover {
    /// The aim this cover takes off a shot, the `cover` input of `hit_chance`.
    pub fn penalty(self) -> i32 {
        match self {
            Cover::None => 0,
            Cover::Half => HALF_COVER_PENALTY,
            Cover::Full => FULL_COVER_PENALTY,
        }
    }
}

/// The four sides a cover object can be on: north, south, east, west.
pub const SIDES: [(i32, i32); 4] = [(0, -1), (0, 1), (1, 0), (-1, 0)];

/// The cover the cell itself gives to a unit beside it. Off the map counts as wall.
pub fn cover_at(map: &Map, (x, y): (i32, i32)) -> Cover {
    if x < 0 || x >= map.width || y < 0 || y >= map.height {
        return Cover::Full;
    }
    let idx = map.xy_idx(x, y);
    if tile_opaque(map.tiles[idx]) || map.view_blocked.contains(&idx) {
        Cover::Full
    } else {
        map.prop_cover.get(&idx).copied().unwrap_or_default()
    }
}

/// Every covered side of `cell` and its cover: the shields the hover shows.
pub fn cover_sides(map: &Map, (x, y): (i32, i32)) -> Vec<((i32, i32), Cover)> {
    SIDES
        .iter()
        .map(|&(dx, dy)| ((dx, dy), cover_at(map, (x + dx, y + dy))))
        .filter(|&(_, cover)| cover != Cover::None)
        .collect()
}

/// How a target stands against one attacker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exposure {
    /// No cover on any side.
    Open,
    /// The best cover on a side facing the attacker.
    Covered { cover: Cover, side: (i32, i32) },
    /// Cover, but none of it faces the attacker: no cover, and a crit bonus.
    Flanked,
}

impl Exposure {
    /// The `cover` input of `hit_chance`.
    pub fn aim_penalty(self) -> i32 {
        match self {
            Exposure::Covered { cover, .. } => cover.penalty(),
            _ => 0,
        }
    }

    /// Added to the attacker's crit chance.
    pub fn crit_bonus(self) -> i32 {
        if self == Exposure::Flanked { FLANK_CRIT_BONUS } else { 0 }
    }

    /// A short note for the attack log, if cover or a flank applied.
    pub fn note(self) -> Option<&'static str> {
        match self {
            Exposure::Open => None,
            Exposure::Covered { cover: Cover::Half, .. } => Some("half cover"),
            Exposure::Covered { .. } => Some("full cover"),
            Exposure::Flanked => Some("flanked"),
        }
    }
}

/// The XCOM rule: a cover object beside `target` counts when `attacker`
/// stands strictly beyond that side (a diagonal attacker can face two); the
/// best one that counts wins. Cover that faces no attacker leaves the target flanked.
pub fn exposure(map: &Map, target: (i32, i32), attacker: (i32, i32)) -> Exposure {
    let sides = cover_sides(map, target);
    if sides.is_empty() {
        return Exposure::Open;
    }
    let (ax, ay) = (attacker.0 - target.0, attacker.1 - target.1);
    sides
        .into_iter()
        .filter(|&((dx, dy), _)| dx * ax + dy * ay > 0)
        .max_by_key(|&(_, cover)| cover)
        .map_or(Exposure::Flanked, |(side, cover)| Exposure::Covered { cover, side })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::TileType;

    /// A 9 by 9 room of floor inside a wall ring; the target stands at (4, 4).
    fn room() -> Map {
        let mut map = Map::new(9, 9, 1);
        for y in 1..8 {
            for x in 1..8 {
                let idx = map.xy_idx(x, y);
                map.tiles[idx] = TileType::Floor;
            }
        }
        map
    }

    fn wall(map: &mut Map, cell: (i32, i32)) {
        let idx = map.xy_idx(cell.0, cell.1);
        map.tiles[idx] = TileType::Wall;
    }

    fn prop(map: &mut Map, cell: (i32, i32), cover: Cover) {
        let idx = map.xy_idx(cell.0, cell.1);
        map.prop_cover.insert(idx, cover);
    }

    const T: (i32, i32) = (4, 4);

    #[test]
    fn a_target_in_the_open_has_no_cover_and_is_not_flanked() {
        let map = room();
        assert_eq!(exposure(&map, T, (7, 4)), Exposure::Open);
        assert!(cover_sides(&map, T).is_empty());
    }

    #[test]
    fn a_wall_covers_fully_from_its_own_side_only() {
        for side in SIDES {
            let mut map = room();
            wall(&mut map, (T.0 + side.0, T.1 + side.1));
            let beyond = (T.0 + 3 * side.0, T.1 + 3 * side.1);
            assert_eq!(exposure(&map, T, beyond), Exposure::Covered { cover: Cover::Full, side }, "{side:?}");
            let behind = (T.0 - 3 * side.0, T.1 - 3 * side.1);
            assert_eq!(exposure(&map, T, behind), Exposure::Flanked, "{side:?}");
            // Level with the cover line is past it too.
            let level = (T.0 + 3 * side.1, T.1 + 3 * side.0);
            assert_eq!(exposure(&map, T, level), Exposure::Flanked, "{side:?}");
        }
    }

    #[test]
    fn a_diagonal_attacker_faces_two_sides() {
        let mut map = room();
        prop(&mut map, (4, 3), Cover::Half);
        let north = Exposure::Covered { cover: Cover::Half, side: (0, -1) };
        assert_eq!(exposure(&map, T, (7, 1)), north, "north-east");
        assert_eq!(exposure(&map, T, (1, 1)), north, "north-west");
        assert_eq!(exposure(&map, T, (6, 3)), north, "one row north is enough");
        assert_eq!(exposure(&map, T, (7, 7)), Exposure::Flanked, "south-east");
    }

    #[test]
    fn half_and_full_cover_cost_their_aim_and_the_best_wins() {
        let mut map = room();
        prop(&mut map, (4, 3), Cover::Half);
        prop(&mut map, (5, 4), Cover::Full);
        let half = exposure(&map, T, (4, 1));
        let full = exposure(&map, T, (7, 4));
        assert_eq!((half.aim_penalty(), full.aim_penalty()), (HALF_COVER_PENALTY, FULL_COVER_PENALTY));
        // From the north-east both count: full wins.
        assert_eq!(exposure(&map, T, (7, 1)), Exposure::Covered { cover: Cover::Full, side: (1, 0) });
        // A prop with no cover is no cover.
        let mut map = room();
        prop(&mut map, (4, 3), Cover::None);
        assert_eq!(exposure(&map, T, (4, 1)), Exposure::Open);
    }

    #[test]
    fn flanking_gives_no_cover_and_a_crit_bonus() {
        let mut map = room();
        prop(&mut map, (4, 3), Cover::Full);
        let flanked = exposure(&map, T, (4, 7));
        assert_eq!(flanked, Exposure::Flanked);
        assert_eq!((flanked.aim_penalty(), flanked.crit_bonus()), (0, FLANK_CRIT_BONUS));
        assert_eq!(exposure(&map, T, (4, 1)).crit_bonus(), 0);
        assert_eq!(flanked.note(), Some("flanked"));
    }

    #[test]
    fn a_closed_door_is_a_wall_and_an_open_one_nothing() {
        let mut map = room();
        let idx = map.xy_idx(4, 3);
        map.view_blocked.insert(idx);
        assert_eq!(cover_at(&map, (4, 3)), Cover::Full);
        map.view_blocked.clear();
        assert_eq!(cover_at(&map, (4, 3)), Cover::None);
    }

    /// The hover shields and the attack rule agree: from every attacker cell,
    /// the cover the attack uses is the best shield on a side facing it.
    #[test]
    fn the_shields_agree_with_the_attack_rule() {
        let mut map = room();
        wall(&mut map, (4, 3));
        prop(&mut map, (3, 4), Cover::Half);
        let shields = cover_sides(&map, T);
        assert_eq!(shields, vec![((0, -1), Cover::Full), ((-1, 0), Cover::Half)]);
        for y in 1..8 {
            for x in 1..8 {
                let facing = shields
                    .iter()
                    .filter(|((dx, dy), _)| dx * (x - T.0) + dy * (y - T.1) > 0)
                    .map(|&(_, c)| c)
                    .max();
                let used = match exposure(&map, T, (x, y)) {
                    Exposure::Covered { cover, side } => {
                        assert!(shields.contains(&(side, cover)));
                        Some(cover)
                    }
                    Exposure::Flanked => None,
                    Exposure::Open => panic!("the target has cover"),
                };
                assert_eq!(used, facing, "attacker at ({x}, {y})");
            }
        }
    }
}
