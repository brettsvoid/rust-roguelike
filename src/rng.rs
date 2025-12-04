use bevy::prelude::*;
use rand::{rngs::StdRng, SeedableRng};

#[derive(Resource)]
pub struct GameRng(pub StdRng);

impl Default for GameRng {
    fn default() -> Self {
        GameRng(StdRng::from_entropy())
    }
}

impl GameRng {
    pub fn seeded(seed: u64) -> Self {
        GameRng(StdRng::seed_from_u64(seed))
    }
}
