use bevy::prelude::*;

#[derive(Resource, Default)]
pub struct DebugMode {
    pub enabled: bool,
    pub show_fov_overlay: bool,
    pub show_tile_info: bool,
    pub show_inspector: bool,
    pub show_console: bool,
}

#[derive(Resource, Default)]
pub struct DebugState {
    pub console_input: String,
    pub console_output: Vec<String>,
    pub command_history: Vec<String>,
    pub history_index: usize,
    pub reveal_map: bool,
    pub no_fog: bool,
}

#[derive(Resource, Default)]
pub struct GodMode(pub bool);
