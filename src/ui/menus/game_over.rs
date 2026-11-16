use bevy::input::keyboard::KeyboardInput;
use bevy::input::ButtonState;
use bevy::prelude::*;

use crate::RunState;

use crate::ui::panel::{close_panels, show, spawn_panel, BodyLine, Hint, Panel, PanelSpec, Placement};

pub struct GameOverPlugin;

impl Plugin for GameOverPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(RunState::GameOver), |mut commands: Commands| {
            spawn_panel(&mut commands, Placement::Modal);
        })
        .add_systems(OnExit(RunState::GameOver), close_panels)
        .add_systems(Update, (handle_game_over_input, write_game_over).run_if(in_state(RunState::GameOver)));
    }
}

fn game_over_spec() -> PanelSpec {
    PanelSpec {
        title: "The party has fallen".into(),
        alarm: true,
        width: 420.0,
        body: vec![
            BodyLine::Text("Every hero is dead, and the save with them.".into()),
            BodyLine::Text("The town keeps its trade for the next party.".into()),
        ],
        hints: vec![Hint::new("Any key", "Main menu", KeyCode::Escape)],
        ..default()
    }
}

fn write_game_over(mut panels: Query<(&Panel, &mut PanelSpec)>) {
    show(&mut panels, game_over_spec());
}

fn handle_game_over_input(mut evr_kbd: MessageReader<KeyboardInput>, mut next_state: ResMut<NextState<RunState>>) {
    for ev in evr_kbd.read() {
        if ev.state == ButtonState::Pressed {
            next_state.set_if_different(RunState::MainMenu);
            return;
        }
    }
}
