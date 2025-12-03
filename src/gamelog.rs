use bevy::prelude::*;

#[derive(Resource)]
pub struct GameLog {
    pub entries: Vec<String>,
}

impl Default for GameLog {
    fn default() -> Self {
        GameLog {
            entries: vec!["Welcome to Rusty Roguelike".to_string()],
        }
    }
}
