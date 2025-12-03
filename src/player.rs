use bevy::{
    color::palettes,
    input::{keyboard::KeyboardInput, ButtonState},
    prelude::*,
};

use crate::{
    combat::{CombatStats, WantsToMelee},
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
        CombatStats {
            max_hp: 30,
            hp: 30,
            defense: 2,
            power: 5,
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

fn try_move_player(
    commands: &mut Commands,
    map: &Map,
    player_entity: Entity,
    pos: &mut Position,
    delta_x: i32,
    delta_y: i32,
    combat_stats: &Query<&CombatStats>,
) {
    let destination_idx = xy_idx(pos.x + delta_x, pos.y + delta_y);

    // Check for attackable targets
    for potential_target in map.tile_content[destination_idx].iter() {
        if combat_stats.get(*potential_target).is_ok() {
            commands.entity(player_entity).insert(WantsToMelee {
                target: *potential_target,
            });
            return; // So we don't move after attacking
        }
    }

    if !map.blocked_tiles[destination_idx] {
        pos.x = (pos.x + delta_x).clamp(0, map.width - 1);
        pos.y = (pos.y + delta_y).clamp(0, map.height - 1);
    }
}

fn handle_player_input(
    mut commands: Commands,
    mut evr_kbd: EventReader<KeyboardInput>,
    map: Res<Map>,
    mut query: Single<(Entity, &mut Position), With<Player>>,
    mut next_state: ResMut<NextState<RunState>>,
    combat_stats: Query<&CombatStats>,
) {
    let (player_entity, ref mut pos) = *query;
    let mut player_moved = false;

    for ev in evr_kbd.read() {
        // We don't care about key releases or key repeats, only initial key presses
        if ev.state == ButtonState::Released || ev.repeat {
            continue;
        }

        match &ev.key_code {
            KeyCode::ArrowLeft | KeyCode::KeyH | KeyCode::Numpad4 => {
                try_move_player(
                    &mut commands,
                    &map,
                    player_entity,
                    pos,
                    -1,
                    0,
                    &combat_stats,
                );
                player_moved = true;
            }
            KeyCode::ArrowRight | KeyCode::KeyL | KeyCode::Numpad6 => {
                try_move_player(&mut commands, &map, player_entity, pos, 1, 0, &combat_stats);
                player_moved = true;
            }
            KeyCode::ArrowUp | KeyCode::KeyK | KeyCode::Numpad8 => {
                try_move_player(
                    &mut commands,
                    &map,
                    player_entity,
                    pos,
                    0,
                    -1,
                    &combat_stats,
                );
                player_moved = true;
            }
            KeyCode::ArrowDown | KeyCode::KeyJ | KeyCode::Numpad2 => {
                try_move_player(&mut commands, &map, player_entity, pos, 0, 1, &combat_stats);
                player_moved = true;
            }

            // Diagonals
            KeyCode::KeyY | KeyCode::Numpad7 => {
                try_move_player(
                    &mut commands,
                    &map,
                    player_entity,
                    pos,
                    -1,
                    -1,
                    &combat_stats,
                );
                player_moved = true;
            }
            KeyCode::KeyU | KeyCode::Numpad9 => {
                try_move_player(
                    &mut commands,
                    &map,
                    player_entity,
                    pos,
                    1,
                    -1,
                    &combat_stats,
                );
                player_moved = true;
            }
            KeyCode::KeyM | KeyCode::Numpad3 => {
                try_move_player(&mut commands, &map, player_entity, pos, 1, 1, &combat_stats);
                player_moved = true;
            }
            KeyCode::KeyN | KeyCode::Numpad1 => {
                try_move_player(
                    &mut commands,
                    &map,
                    player_entity,
                    pos,
                    -1,
                    1,
                    &combat_stats,
                );
                player_moved = true;
            }
            _ => {}
        }
    }

    if player_moved {
        next_state.set(RunState::PlayerTurn);
    }
}
