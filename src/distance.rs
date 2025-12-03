use bevy::prelude::*;

/// Distance calculation algorithms
pub enum DistanceAlg {
    /// Straight-line distance: sqrt((x2-x1)² + (y2-y1)²)
    /// Best for: circular ranges, spell AOE.
    Pythagoras,
    /// No diagonal movement: |x2-x1| + |y2-y1|
    /// Best for: 4-directional movement only.
    Manhattan,
    /// Diagonal costs same as cardinal: max(|x2-x1|, |y2-y1|)
    /// Best for: 8-directional roguelike movement.
    Chebyshev,
}

impl DistanceAlg {
    pub fn distance2d(&self, p1: Vec2, p2: Vec2) -> f32 {
        let delta = (p2 - p1).abs();

        match self {
            DistanceAlg::Pythagoras => delta.length(),
            DistanceAlg::Manhattan => delta.x + delta.y,
            DistanceAlg::Chebyshev => delta.x.max(delta.y),
        }
    }
}
