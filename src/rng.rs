use bevy::prelude::*;
use rand::{rngs::StdRng, Rng, SeedableRng};

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

pub struct RandomEntry {
    name: String,
    weight: i32,
}

pub struct RandomTable {
    entries: Vec<RandomEntry>,
    total_weight: i32,
}

impl RandomTable {
    pub fn new() -> Self {
        RandomTable {
            entries: Vec::new(),
            total_weight: 0,
        }
    }

    pub fn add(mut self, name: &str, weight: i32) -> Self {
        if weight > 0 {
            self.total_weight += weight;
            self.entries.push(RandomEntry {
                name: name.to_string(),
                weight,
            });
        }
        self
    }

    pub fn roll(&self, rng: &mut GameRng) -> Option<String> {
        if self.total_weight == 0 {
            return None;
        }

        let mut roll = rng.0.gen_range(1..=self.total_weight);
        for entry in &self.entries {
            if roll <= entry.weight {
                return Some(entry.name.clone());
            }
            roll -= entry.weight;
        }

        None
    }
}
