use std::time::Duration;

use bevy::prelude::*;
use map::{MapPlugin, GRID_PX, MAP_HEIGHT, MAP_WIDTH};
use monsters::MonstersPlugin;
use player::PlayerPlugin;
use resources::ResourcesPlugin;
use viewshed::ViewshedPlugin;

mod map;
mod monsters;
mod player;
mod resources;
mod shapes;
mod viewshed;

const RESOLUTION: Vec2 = Vec2 {
    x: MAP_WIDTH as f32 * GRID_PX.x,
    y: MAP_HEIGHT as f32 * GRID_PX.y,
};

#[derive(States, Clone, Copy, Default, Eq, PartialEq, Debug, Hash)]
pub enum RunState {
    Paused,
    #[default]
    Running,
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Rust Roguelike".into(),
                resolution: RESOLUTION.into(),
                ..default()
            }),
            ..default()
        }))
        .init_state::<RunState>()
        .add_event::<AppExit>()
        .add_plugins((
            ResourcesPlugin,
            PlayerPlugin,
            ViewshedPlugin,
            MapPlugin,
            MonstersPlugin,
        ))
        .add_systems(Startup, setup)
        .add_systems(Update, (update_run_state, handle_exit))
        //.set_runner(run_loop)
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
}

fn handle_exit(keyboard: Res<ButtonInput<KeyCode>>, mut exit: EventWriter<AppExit>) {
    if keyboard.just_released(KeyCode::KeyQ) {
        exit.send(AppExit::Success);
    }
}

fn update_run_state(
    current_state: Res<State<RunState>>,
    mut next_state: ResMut<NextState<RunState>>,
) {
    if current_state.get() == &RunState::Running {
        next_state.set(RunState::Paused);
    }
}

fn run_loop(mut app: App) -> AppExit {
    //let mut exit_event_reader = app.world().resource_mut::<Events<AppExit>>().get_cursor();

    loop {
        let run_state = app.world().resource::<State<RunState>>();
        if run_state.get() == &RunState::Running {
            app.update();
        }

        // Check if we got an exit event, etc...
        if let Some(exit) = app.should_exit() {
            return exit;
        }

        // Check if we should exit
        {
            //let exit_events = app.world().resource::<Events<AppExit>>();
            //for exit in exit_event_reader.read(exit_events) {
            //    return exit.clone();
            //}

            //if !exit_events.is_empty() {
            //    // Return the first exit event found.
            //    if let Some(exit) = exit_events.iter().next() {
            //        return exit.clone();
            //    }
            //}
        }

        // Small sleep to avoid busy-looping (adjust as needed)
        std::thread::sleep(Duration::from_millis(16));
    }
}
