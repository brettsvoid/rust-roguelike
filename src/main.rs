use std::time::Duration;

use bevy::prelude::*;
use map::{MapPlugin, GRID_PX, MAP_HEIGHT, MAP_WIDTH};
use monsters::MonstersPlugin;
use player::PlayerPlugin;
use resources::ResourcesPlugin;
use viewshed::ViewshedPlugin;

mod combat;
mod components;
mod distance;
mod map;
mod map_indexing;
mod monsters;
mod pathfinding;
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
    #[default]
    AwaitingInput,
    PreRun,
    PlayerTurn,
    MonsterTurn,
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
        .add_systems(Update, (map_indexing::map_indexing_system, handle_exit))
        // PreRun: run systems then transition to AwaitingInput
        .add_systems(
            Update,
            transition_to_awaiting_input.run_if(in_state(RunState::PreRun)),
        )
        // PlayerTurn: run combat systems then transition to MonsterTurn
        .add_systems(
            Update,
            (
                combat::melee_combat_system,
                combat::damage_system,
                combat::delete_the_dead,
                transition_to_monster_turn,
            )
                .chain()
                .run_if(in_state(RunState::PlayerTurn)),
        )
        // MonsterTurn: run monster AI then transition to AwaitingInput
        .add_systems(
            Update,
            (
                monsters::monster_ai,
                combat::melee_combat_system,
                combat::damage_system,
                combat::delete_the_dead,
                transition_to_awaiting_input,
            )
                .chain()
                .run_if(in_state(RunState::MonsterTurn)),
        )
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

fn transition_to_awaiting_input(mut next_state: ResMut<NextState<RunState>>) {
    next_state.set(RunState::AwaitingInput);
}

fn transition_to_monster_turn(mut next_state: ResMut<NextState<RunState>>) {
    next_state.set(RunState::MonsterTurn);
}

fn run_loop(mut app: App) -> AppExit {
    //let mut exit_event_reader = app.world().resource_mut::<Events<AppExit>>().get_cursor();

    loop {
        //let run_state = app.world().resource::<State<RunState>>();
        //if run_state.get() == &RunState::Running {
        app.update();
        //}

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
