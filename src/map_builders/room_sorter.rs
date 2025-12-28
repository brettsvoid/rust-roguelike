use crate::map::{MAP_HEIGHT, MAP_WIDTH};
use crate::rng::GameRng;

use super::{BuilderMap, MetaMapBuilder};

// ============================================================================
// RoomSort - Sorting strategies for room ordering
// ============================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoomSort {
    /// Sort by leftmost x coordinate (x1)
    Leftmost,
    /// Sort by rightmost x coordinate (x2, descending)
    Rightmost,
    /// Sort by topmost y coordinate (y1)
    Topmost,
    /// Sort by bottommost y coordinate (y2, descending)
    Bottommost,
    /// Sort by distance from map center (closest first)
    Central,
}

// ============================================================================
// RoomSorter - MetaMapBuilder that sorts rooms in build_data
// ============================================================================

pub struct RoomSorter {
    sort_by: RoomSort,
}

impl RoomSorter {
    pub fn new(sort_by: RoomSort) -> Box<Self> {
        Box::new(Self { sort_by })
    }
}

impl MetaMapBuilder for RoomSorter {
    fn build_map(&mut self, _rng: &mut GameRng, build_data: &mut BuilderMap) {
        if let Some(ref mut rooms) = build_data.rooms {
            match self.sort_by {
                RoomSort::Leftmost => {
                    rooms.sort_by(|a, b| a.x1.cmp(&b.x1));
                }
                RoomSort::Rightmost => {
                    rooms.sort_by(|a, b| b.x2.cmp(&a.x2));
                }
                RoomSort::Topmost => {
                    rooms.sort_by(|a, b| a.y1.cmp(&b.y1));
                }
                RoomSort::Bottommost => {
                    rooms.sort_by(|a, b| b.y2.cmp(&a.y2));
                }
                RoomSort::Central => {
                    let center_x = MAP_WIDTH as i32 / 2;
                    let center_y = MAP_HEIGHT as i32 / 2;
                    rooms.sort_by(|a, b| {
                        let (ax, ay) = a.center();
                        let (bx, by) = b.center();
                        let dist_a = (ax - center_x).abs() + (ay - center_y).abs();
                        let dist_b = (bx - center_x).abs() + (by - center_y).abs();
                        dist_a.cmp(&dist_b)
                    });
                }
            }
        }
    }
}
