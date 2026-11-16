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
mod cover;
mod debug;
mod distance;
mod economy;
mod effects;
mod entity_render;
mod game_flow;
mod gamelog;
mod gui;
mod hunger;
mod inventory;
mod levels;
mod lighting;
mod map;
mod map_builders;
mod map_indexing;
mod map_render;
mod mapgen_viz;
mod models;
mod monsters;
mod overwatch;
mod occlusion;
mod party;
mod pathfinding;
mod picking;
mod player;
mod raws;
mod resources;
mod rng;
mod saveload;
mod shapes;
mod spawner;
mod tactics;
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
    // The screenshot harness compiles shaders before drawing, so a frame
    // count always means the same picture however slow the compiler is.
    let screenshot = std::env::args().any(|a| a == "--screenshot");
    // FRAME_TIME_LOG=1 logs frame times with vsync off, for performance checks.
    let frame_time_log = std::env::var_os("FRAME_TIME_LOG").is_some();
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Rust Roguelike".into(),
                    resolution: RESOLUTION.as_uvec2().into(),
                    present_mode: if frame_time_log {
                        bevy::window::PresentMode::AutoNoVsync
                    } else {
                        default()
                    },
                    ..default()
                }),
                ..default()
            })
            .set(bevy::render::RenderPlugin {
                synchronous_pipeline_compilation: screenshot,
                ..default()
            }),
    )
        .init_resource::<gamelog::GameLog>()
        .init_resource::<economy::Purse>()
        .init_resource::<rng::GameRng>()
        // Core game plugins
        .add_plugins((
            resources::ResourcesPlugin,
            game_flow::GameFlowPlugin,
            mapgen_viz::MapGenVizPlugin,
            party::PartyPlugin,
            player::PlayerPlugin,
            viewshed::ViewshedPlugin,
            map::MapPlugin,
            models::ModelsPlugin,
            monsters::MonstersPlugin,
            bystander::BystanderPlugin,
            camera::CameraPlugin,
            occlusion::OcclusionPlugin,
            economy::town::TownPlugin,
            lighting::LightingPlugin,
            effects::EffectsPlugin,
        ))
        // UI plugins
        .add_plugins((
            gui::GuiPlugin,
            picking::PickingPlugin,
            ui::HudPlugin,
            ui::TooltipPlugin,
            ui::BannerPlugin,
            ui::HealthBarsPlugin,
            ui::TargetingPlugin,
            ui::GameOverPlugin,
            ui::InventoryPlugin,
            ui::LedgerPlugin,
            ui::ShopPlugin,
            debug::DebugPlugin,
        ))
        .add_systems(Startup, setup)
        .add_systems(Update, saveload::save_on_quit)
        // World upkeep that runs in every state except the map visualizer
        .add_systems(
            Update,
            (
                map_indexing::map_indexing_system,
                traps::reveal_hidden_system,
            )
                .run_if(not(in_state(RunState::MapGeneration))),
        );
    if frame_time_log {
        app.add_plugins((
            bevy::diagnostic::FrameTimeDiagnosticsPlugin::default(),
            bevy::diagnostic::LogDiagnosticsPlugin::default(),
        ));
    }
    app.run();
}

fn setup() {
    raws::load_raws();
}
