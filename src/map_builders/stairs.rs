//! Stairs meta builder, run last on every dungeon level. Up stairs go where
//! the party arrives. On the deepest level there are no down stairs: the
//! Heart of the Abyss lies where they would have been (the far end of the
//! level), with a Demon Lord beside it.

use crate::levels::HEART_OF_THE_ABYSS;
use crate::map::TileType;
use crate::rng::GameRng;

use super::{BuilderMap, MetaMapBuilder};

/// The Heart's guard, a raws mob.
pub const HEART_GUARD: &str = "Demon Lord";

pub struct LevelStairs {
    /// The deepest level: no down stairs.
    bottom: bool,
    /// Lay the Heart and its guard (not when the party already carries it).
    heart: bool,
}

impl LevelStairs {
    pub fn new(bottom: bool, heart: bool) -> Box<Self> {
        Box::new(Self { bottom, heart })
    }
}

impl MetaMapBuilder for LevelStairs {
    fn build_map(&mut self, _rng: &mut GameRng, build_data: &mut BuilderMap) {
        let map = &mut build_data.map;
        let start = super::starting_position(build_data.starting_position);
        let start_idx = map.xy_idx(start.0, start.1);
        map.tiles[start_idx] = TileType::UpStairs;

        if self.bottom {
            let down = map.tiles.iter().position(|&t| t == TileType::DownStairs);
            for tile in map.tiles.iter_mut().filter(|t| **t == TileType::DownStairs) {
                *tile = TileType::Floor;
            }
            if let (true, Some(heart)) = (self.heart, down) {
                let (x, y) = ((heart % map.width as usize) as i32, (heart / map.width as usize) as i32);
                let guard = [(0, -1), (-1, 0), (1, 0), (0, 1), (-1, -1), (1, -1), (-1, 1), (1, 1)]
                    .into_iter()
                    .map(|(dx, dy)| map.xy_idx(x + dx, y + dy))
                    .find(|&idx| idx != start_idx && map.tiles[idx] == TileType::Floor);
                build_data.spawn_list.retain(|&(idx, _)| idx != heart && Some(idx) != guard);
                build_data.spawn_list.push((heart, HEART_OF_THE_ABYSS.to_string()));
                if let Some(guard) = guard {
                    build_data.spawn_list.push((guard, HEART_GUARD.to_string()));
                }
            }
        }
        build_data.take_snapshot();
    }
}
