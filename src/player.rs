use bevy::{
    input::{keyboard::KeyboardInput, ButtonState},
    prelude::*,
};

use crate::{
    combat::{CombatStats, WantsToMelee},
    components::{Item, WantsToPickupItem},
    debug::DebugMode,
    gamelog::GameLog,
    map::{xy_idx, Map, Position, TileType, FONT_SIZE},
    monsters::Monster,
    resources::UiFont,
    spawner,
    viewshed::Viewshed,
    RunState,
};

#[derive(Component, Debug)]
pub struct Player;

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_player)
            .add_systems(Update, handle_player_input.run_if(in_state(RunState::AwaitingInput)));
    }
}

fn spawn_player(mut commands: Commands, font: Res<UiFont>, map: Res<Map>) {
    let text_font = TextFont {
        font: font.0.clone(),
        font_size: FONT_SIZE,
        ..default()
    };

    let (player_x, player_y) = map.rooms[0].center();
    spawner::spawn_player(&mut commands, &text_font, player_x, player_y);
}

fn try_move_player(
    commands: &mut Commands,
    map: &Map,
    player_entity: Entity,
    pos: &mut Position,
    delta_x: i32,
    delta_y: i32,
    combat_stats: &Query<&CombatStats, Without<Player>>,
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

fn get_item(
    commands: &mut Commands,
    gamelog: &mut GameLog,
    player_entity: Entity,
    player_pos: &Position,
    items: &Query<(Entity, &Position), (With<Item>, Without<Player>)>,
) -> bool {
    for (item_entity, item_pos) in items {
        if item_pos.x == player_pos.x && item_pos.y == player_pos.y {
            commands.entity(player_entity).insert(WantsToPickupItem {
                collected_by: player_entity,
                item: item_entity,
            });
            return true;
        }
    }
    gamelog.entries.push("There is nothing here to pick up.".to_string());
    false
}

fn handle_player_input(
    mut commands: Commands,
    mut evr_kbd: EventReader<KeyboardInput>,
    map: Res<Map>,
    debug_mode: Res<DebugMode>,
    mut gamelog: ResMut<GameLog>,
    mut query: Single<(Entity, &mut Position, &Viewshed, &mut CombatStats), With<Player>>,
    mut next_state: ResMut<NextState<RunState>>,
    other_combat_stats: Query<&CombatStats, Without<Player>>,
    items: Query<(Entity, &Position), (With<Item>, Without<Player>)>,
    monsters: Query<&Position, (With<Monster>, Without<Player>)>,
) {
    // Don't process player input if debug console is open
    if debug_mode.show_console {
        return;
    }

    let (player_entity, ref mut pos, viewshed, ref mut player_stats) = *query;
    let mut player_acted = false;

    for ev in evr_kbd.read() {
        // We don't care about key releases, only key presses (including repeats)
        if ev.state == ButtonState::Released {
            continue;
        }

        // Only process one input per frame to allow turn cycle to complete
        if player_acted {
            break;
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
                    &other_combat_stats,
                );
                player_acted = true;
            }
            KeyCode::ArrowRight | KeyCode::KeyL | KeyCode::Numpad6 => {
                try_move_player(&mut commands, &map, player_entity, pos, 1, 0, &other_combat_stats);
                player_acted = true;
            }
            KeyCode::ArrowUp | KeyCode::KeyK | KeyCode::Numpad8 => {
                try_move_player(
                    &mut commands,
                    &map,
                    player_entity,
                    pos,
                    0,
                    -1,
                    &other_combat_stats,
                );
                player_acted = true;
            }
            KeyCode::ArrowDown | KeyCode::KeyJ | KeyCode::Numpad2 => {
                try_move_player(&mut commands, &map, player_entity, pos, 0, 1, &other_combat_stats);
                player_acted = true;
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
                    &other_combat_stats,
                );
                player_acted = true;
            }
            KeyCode::KeyU | KeyCode::Numpad9 => {
                try_move_player(
                    &mut commands,
                    &map,
                    player_entity,
                    pos,
                    1,
                    -1,
                    &other_combat_stats,
                );
                player_acted = true;
            }
            KeyCode::KeyM | KeyCode::Numpad3 => {
                try_move_player(&mut commands, &map, player_entity, pos, 1, 1, &other_combat_stats);
                player_acted = true;
            }
            KeyCode::KeyN | KeyCode::Numpad1 => {
                try_move_player(
                    &mut commands,
                    &map,
                    player_entity,
                    pos,
                    -1,
                    1,
                    &other_combat_stats,
                );
                player_acted = true;
            }

            // Skip turn / wait
            KeyCode::Space | KeyCode::Numpad5 => {
                // Check if any monsters are visible
                let mut can_heal = true;
                for monster_pos in monsters.iter() {
                    if viewshed.visible_tiles.contains(&(monster_pos.x, monster_pos.y)) {
                        can_heal = false;
                        break;
                    }
                }
                // Heal 1 HP if no monsters visible
                if can_heal && player_stats.hp < player_stats.max_hp {
                    player_stats.hp = (player_stats.hp + 1).min(player_stats.max_hp);
                    gamelog.entries.push("You rest and recover 1 HP.".to_string());
                }
                player_acted = true;
            }

            // Pickup
            KeyCode::KeyG => {
                if get_item(&mut commands, &mut gamelog, player_entity, pos, &items) {
                    player_acted = true;
                }
            }

            // Inventory
            KeyCode::KeyI => {
                next_state.set(RunState::ShowInventory);
            }

            // Drop item
            KeyCode::KeyD => {
                next_state.set(RunState::ShowDropItem);
            }

            // Remove equipment
            KeyCode::KeyR => {
                next_state.set(RunState::ShowRemoveItem);
            }

            // Go down stairs
            KeyCode::Period => {
                let idx = xy_idx(pos.x, pos.y);
                if map.tiles[idx] == TileType::DownStairs {
                    next_state.set(RunState::NextLevel);
                } else {
                    gamelog.entries.push("There are no stairs here.".to_string());
                }
            }
            _ => {}
        }
    }

    if player_acted {
        next_state.set(RunState::PlayerTurn);
    }
}
