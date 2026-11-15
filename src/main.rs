// Bevy systems naturally have many parameters and complex query types;
// these two clippy lints fight the framework, so they're off crate-wide
// (Bevy's own examples do the same).
#![allow(clippy::too_many_arguments, clippy::type_complexity)]

use bevy::prelude::*;

use map::{GRID_PX, MAP_WIDTH};

mod bystander;
mod camera;
mod combat;
mod components;
mod debug;
mod distance;
mod economy;
mod entity_render;
mod game_flow;
mod gamelog;
mod gui;
mod hunger;
mod inventory;
mod levels;
mod map;
mod map_builders;
mod map_indexing;
mod map_render;
mod mapgen_viz;
mod models;
mod monsters;
mod occlusion;
mod particle;
mod pathfinding;
mod picking;
mod player;
mod raws;
mod resources;
mod rng;
mod saveload;
mod shapes;
mod spawner;
mod traps;
mod ui;
mod viewshed;

// Re-exported so the rest of the crate can keep using `crate::RunState` etc.
pub use game_flow::{PendingMagicMap, RunState};
pub use ui::TargetingInfo;

const SCREEN_HEIGHT: usize = 50;
const RESOLUTION: Vec2 = Vec2 {
    x: MAP_WIDTH as f32 * GRID_PX.x,
    y: SCREEN_HEIGHT as f32 * GRID_PX.y,
};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Rust Roguelike".into(),
                resolution: RESOLUTION.as_uvec2().into(),
                ..default()
            }),
            ..default()
        }))
        .init_resource::<gamelog::GameLog>()
        .init_resource::<economy::Purse>()
        .init_resource::<rng::GameRng>()
        .init_resource::<particle::ParticleBuilder>()
        // Core game plugins
        .add_plugins((
            resources::ResourcesPlugin,
            game_flow::GameFlowPlugin,
            mapgen_viz::MapGenVizPlugin,
            player::PlayerPlugin,
            viewshed::ViewshedPlugin,
            map::MapPlugin,
            models::ModelsPlugin,
            monsters::MonstersPlugin,
            bystander::BystanderPlugin,
            camera::CameraPlugin,
            occlusion::OcclusionPlugin,
            economy::town::TownPlugin,
        ))
        // UI plugins
        .add_plugins((
            gui::GuiPlugin,
            picking::PickingPlugin,
            ui::HudPlugin,
            ui::TooltipPlugin,
            ui::TargetingPlugin,
            ui::GameOverPlugin,
            ui::InventoryPlugin,
            ui::LedgerPlugin,
            debug::DebugPlugin,
        ))
        .add_systems(Startup, setup)
        .add_systems(Update, saveload::save_on_quit)
        // World upkeep that runs in every state except the map visualizer
        .add_systems(
            Update,
            (
                map_indexing::map_indexing_system,
                particle::particle_spawn_system,
                particle::particle_cull_system,
                traps::reveal_hidden_system,
            )
                .run_if(not(in_state(RunState::MapGeneration))),
        )
        .run();
}

fn setup() {
    raws::load_raws();
}
