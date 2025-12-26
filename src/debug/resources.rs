use bevy::prelude::*;

#[derive(Resource)]
pub struct DebugMode {
    pub enabled: bool,
    pub show_fov_overlay: bool,
    pub show_tile_info: bool,
    pub show_inspector: bool,
    pub show_console: bool,
}

impl Default for DebugMode {
    fn default() -> Self {
        Self {
            enabled: true,
            show_fov_overlay: false,
            show_tile_info: false,
            show_inspector: false,
            show_console: false,
        }
    }
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
