//! Wall torch meta builder.
//!
//! Hangs torches on walls so dungeons get pools of light. A torch only goes
//! on a wall with open ground on exactly one side, so it lights one room and
//! never sits inside a thin divider, and no two torches are closer than
//! `SPACING` tiles. Run it last: it only reads the finished layout.

use rand::seq::SliceRandom;

use crate::map::TileType;
use crate::rng::GameRng;

use super::{BuilderMap, MetaMapBuilder};

/// Minimum distance between two torches, in tiles.
const SPACING: i32 = 9;

pub struct WallTorches;

impl WallTorches {
    pub fn new() -> Box<Self> {
        Box::new(Self)
    }
}

impl MetaMapBuilder for WallTorches {
    fn build_map(&mut self, rng: &mut GameRng, build_data: &mut BuilderMap) {
        let map = &build_data.map;
        let is_wall = |x: i32, y: i32| map.tiles[map.xy_idx(x, y)] == TileType::Wall;
        let mut spots: Vec<(i32, i32)> = (1..map.height - 1)
            .flat_map(|y| (1..map.width - 1).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                is_wall(x, y)
                    && [(0, 1), (1, 0), (0, -1), (-1, 0)]
                        .iter()
                        .filter(|&&(dx, dy)| !is_wall(x + dx, y + dy))
                        .count()
                        == 1
            })
            .collect();
        spots.shuffle(&mut rng.0);

        let mut placed: Vec<(i32, i32)> = Vec::new();
        for (x, y) in spots {
            if placed.iter().all(|&(px, py)| (px - x).abs().max((py - y).abs()) >= SPACING) {
                placed.push((x, y));
            }
        }
        for (x, y) in placed {
            let idx = build_data.map.xy_idx(x, y);
            build_data.spawn_list.push((idx, "Wall Torch".to_string()));
        }
        build_data.take_snapshot();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::Map;

    #[test]
    fn torches_hang_on_one_sided_walls_spaced_apart() {
        // A 20 by 5 map: one corridor along y = 2 with walls above and below.
        let mut build_data = BuilderMap::new(2);
        build_data.map = Map::new(20, 5, 2);
        for x in 1..19 {
            let idx = build_data.map.xy_idx(x, 2);
            build_data.map.tiles[idx] = TileType::Floor;
        }
        WallTorches.build_map(&mut GameRng::seeded(1), &mut build_data);

        let spots: Vec<(i32, i32)> = build_data
            .spawn_list
            .iter()
            .map(|(idx, name)| {
                assert_eq!(name, "Wall Torch");
                (*idx as i32 % 20, *idx as i32 / 20)
            })
            .collect();
        assert!(!spots.is_empty());
        for &(x, y) in &spots {
            assert!(y == 1 || y == 3, "torch at ({x}, {y}) is not on a corridor wall");
            for &(ox, oy) in &spots {
                if (ox, oy) != (x, y) {
                    assert!((ox - x).abs().max((oy - y).abs()) >= SPACING);
                }
            }
        }
    }
}
