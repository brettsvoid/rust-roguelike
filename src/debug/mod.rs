mod commands;
mod console;
mod overlays;
mod resources;

use bevy::prelude::*;
use crate::RunState;

pub use resources::{DebugMode, DebugState, GodMode};

pub struct DebugPlugin;

impl Plugin for DebugPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DebugMode>()
            .init_resource::<DebugState>()
            .init_resource::<GodMode>()
            // Master toggle always runs (except during map generation)
            .add_systems(
                Update,
                overlays::toggle_debug_mode.run_if(not(in_state(RunState::MapGeneration))),
            )
            // Debug systems only run when debug is enabled and not in map generation
            .add_systems(
                Update,
                (
                    overlays::toggle_debug_overlays,
                    overlays::update_fov_overlay,
                    overlays::update_tile_info_overlay,
                    overlays::update_state_inspector,
                    overlays::process_reveal_map,
                    console::update_console,
                    console::handle_console_input,
                )
                    .run_if(debug_enabled.and(not(in_state(RunState::MapGeneration)))),
            );
    }
}

fn debug_enabled(debug: Res<DebugMode>) -> bool {
    debug.enabled
}
