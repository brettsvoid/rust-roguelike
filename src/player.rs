use bevy::{
    color::palettes,
    input::{keyboard::KeyboardInput, ButtonState},
    prelude::*,
};

use crate::{
    components::Name,
    map::{xy_idx, Map, Position, TileType, FONT_SIZE},
    resources::UiFont,
    viewshed::Viewshed,
    RunState,
};

#[derive(Component, Debug)]
pub struct Player;

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_player)
            .add_systems(Update, handle_player_input);
    }
}

fn spawn_player(mut commands: Commands, font: Res<UiFont>, map: Res<Map>) {
    let text_font = TextFont {
        font: font.0.clone(),
        font_size: FONT_SIZE,
        ..default()
    };

    let (player_x, player_y) = map.rooms[0].center();
    commands.spawn((
        Player,
        Name {
            name: "Player".to_string(),
        },
        Position {
            x: player_x,
            y: player_y,
            z: 1,
        },
        Viewshed {
            range: 8,
            ..default()
        },
        Text2d::new("☺"),
        text_font.clone(),
        TextColor(palettes::basic::YELLOW.into()),
        BackgroundColor(palettes::basic::BLACK.into()),
    ));
}

fn try_move_player(map: &Map, pos: &mut Position, delta_x: i32, delta_y: i32) {
    let destination_idx = xy_idx(pos.x + delta_x, pos.y + delta_y);

    if map.tiles[destination_idx] != TileType::Wall {
        pos.x = (pos.x + delta_x).clamp(0, map.width - 1);
        pos.y = (pos.y + delta_y).clamp(0, map.height - 1);
    }
}

fn handle_player_input(
    mut evr_kbd: EventReader<KeyboardInput>,
    map: Res<Map>,
    mut query: Single<&mut Position, With<Player>>,
    mut next_state: ResMut<NextState<RunState>>,
) {
    let pos = &mut query;
    let mut player_moved = false;

    for ev in evr_kbd.read() {
        // We don't care about key releases, only key presses
        if ev.state == ButtonState::Released {
            continue;
        }

        match &ev.key_code {
            KeyCode::ArrowLeft | KeyCode::KeyH | KeyCode::Numpad4 => {
                try_move_player(&map, pos, -1, 0);
                player_moved = true;
            }
            KeyCode::ArrowRight | KeyCode::KeyL | KeyCode::Numpad6 => {
                try_move_player(&map, pos, 1, 0);
                player_moved = true;
            }
            KeyCode::ArrowUp | KeyCode::KeyK | KeyCode::Numpad8 => {
                try_move_player(&map, pos, 0, -1);
                player_moved = true;
            }
            KeyCode::ArrowDown | KeyCode::KeyJ | KeyCode::Numpad2 => {
                try_move_player(&map, pos, 0, 1);
                player_moved = true;
            }
            _ => {}
        }
    }

    if player_moved {
        next_state.set(RunState::Running);
    }
}
