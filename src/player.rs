use bevy::{
    ecs::system::ParamSet,
    input::{keyboard::KeyboardInput, ButtonState},
    prelude::*,
};

use crate::{
    bystander::Bystander,
    combat::{CombatStats, WantsToMelee},
    components::{BlocksTile, BlocksVisibility, Door, HungerClock, HungerState, Item, WantsToPickupItem},
    debug::DebugMode,
    gamelog::GameLog,
    map::{xy_idx, Map, Position, TileType},
    monsters::Monster,
    viewshed::Viewshed,
    RunState,
};

#[derive(Component, Debug)]
pub struct Player;

/// Result of attempting to move the player
enum MoveResult {
    /// Player moved to the new position
    Moved { new_x: i32, new_y: i32 },
    /// Player attacked a target
    Attacked {
        #[allow(dead_code)]
        target: Entity,
    },
    /// Player swapped positions with a bystander
    SwappedWithBystander { bystander: Entity, new_x: i32, new_y: i32 },
    /// Player opened a door (doesn't move)
    OpenedDoor,
    /// Movement was blocked
    Blocked,
}

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, handle_player_input.run_if(in_state(RunState::AwaitingInput)));
    }
}

fn try_move_player(
    commands: &mut Commands,
    gamelog: &mut GameLog,
    map: &Map,
    player_entity: Entity,
    player_x: i32,
    player_y: i32,
    delta_x: i32,
    delta_y: i32,
    combat_stats: &Query<&CombatStats, Without<Player>>,
    doors: &mut Query<(Entity, &mut Door, &mut Text2d), Without<Player>>,
    bystanders: &Query<Entity, (With<Bystander>, Without<Player>)>,
) -> MoveResult {
    let dest_x = player_x + delta_x;
    let dest_y = player_y + delta_y;

    // Bounds check - can't move outside map
    if dest_x < 0 || dest_x >= map.width || dest_y < 0 || dest_y >= map.height {
        return MoveResult::Blocked;
    }

    let destination_idx = xy_idx(dest_x, dest_y);

    // Check for bystanders - swap positions instead of attacking
    for potential_target in map.tile_content[destination_idx].iter() {
        if bystanders.get(*potential_target).is_ok() {
            return MoveResult::SwappedWithBystander {
                bystander: *potential_target,
                new_x: dest_x,
                new_y: dest_y,
            };
        }
    }

    // Check for attackable targets (monsters)
    for potential_target in map.tile_content[destination_idx].iter() {
        if combat_stats.get(*potential_target).is_ok() {
            commands.entity(player_entity).insert(WantsToMelee {
                target: *potential_target,
            });
            return MoveResult::Attacked { target: *potential_target };
        }
    }

    // Check for doors
    for potential_door in map.tile_content[destination_idx].iter() {
        if let Ok((door_entity, mut door, mut door_glyph)) = doors.get_mut(*potential_door) {
            if !door.open {
                // Open the door
                door.open = true;
                door_glyph.0 = "/".to_string();
                commands.entity(door_entity).remove::<BlocksTile>();
                commands.entity(door_entity).remove::<BlocksVisibility>();
                gamelog.entries.push("You open the door.".to_string());
                return MoveResult::OpenedDoor;
            }
        }
    }

    if !map.blocked_tiles[destination_idx] {
        let new_x = (player_x + delta_x).clamp(0, map.width - 1);
        let new_y = (player_y + delta_y).clamp(0, map.height - 1);
        MoveResult::Moved { new_x, new_y }
    } else {
        MoveResult::Blocked
    }
}

fn get_item(
    commands: &mut Commands,
    gamelog: &mut GameLog,
    player_entity: Entity,
    player_x: i32,
    player_y: i32,
    items: &Query<(Entity, &Position), (With<Item>, Without<Player>, Without<Bystander>)>,
) -> bool {
    for (item_entity, item_pos) in items {
        if item_pos.x == player_x && item_pos.y == player_y {
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
    mut evr_kbd: MessageReader<KeyboardInput>,
    map: Res<Map>,
    debug_mode: Res<DebugMode>,
    mut gamelog: ResMut<GameLog>,
    mut player_query: Query<(Entity, &Viewshed, &mut CombatStats, &HungerClock), With<Player>>,
    mut next_state: ResMut<NextState<RunState>>,
    other_combat_stats: Query<&CombatStats, Without<Player>>,
    items: Query<(Entity, &Position), (With<Item>, Without<Player>, Without<Bystander>)>,
    monsters: Query<&Position, (With<Monster>, Without<Player>, Without<Bystander>)>,
    mut doors: Query<(Entity, &mut Door, &mut Text2d), Without<Player>>,
    bystanders: Query<Entity, (With<Bystander>, Without<Player>)>,
    // ParamSet to handle conflicting Position queries
    mut position_set: ParamSet<(
        Query<&mut Position, With<Player>>,
        Query<&mut Position, (With<Bystander>, Without<Player>)>,
    )>,
) {
    // Don't process player input if debug console is open
    if debug_mode.show_console {
        return;
    }

    let Ok((player_entity, viewshed, mut player_stats, hunger)) = player_query.single_mut() else {
        return;
    };

    // Get player position
    let (player_x, player_y) = {
        let player_pos_query = position_set.p0();
        let Ok(pos) = player_pos_query.single() else {
            return;
        };
        (pos.x, pos.y)
    };

    let mut player_acted = false;
    let mut move_result: Option<MoveResult> = None;

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
                move_result = Some(try_move_player(
                    &mut commands, &mut gamelog, &map, player_entity, player_x, player_y,
                    -1, 0, &other_combat_stats, &mut doors, &bystanders,
                ));
                player_acted = true;
            }
            KeyCode::ArrowRight | KeyCode::KeyL | KeyCode::Numpad6 => {
                move_result = Some(try_move_player(
                    &mut commands, &mut gamelog, &map, player_entity, player_x, player_y,
                    1, 0, &other_combat_stats, &mut doors, &bystanders,
                ));
                player_acted = true;
            }
            KeyCode::ArrowUp | KeyCode::KeyK | KeyCode::Numpad8 => {
                move_result = Some(try_move_player(
                    &mut commands, &mut gamelog, &map, player_entity, player_x, player_y,
                    0, -1, &other_combat_stats, &mut doors, &bystanders,
                ));
                player_acted = true;
            }
            KeyCode::ArrowDown | KeyCode::KeyJ | KeyCode::Numpad2 => {
                move_result = Some(try_move_player(
                    &mut commands, &mut gamelog, &map, player_entity, player_x, player_y,
                    0, 1, &other_combat_stats, &mut doors, &bystanders,
                ));
                player_acted = true;
            }

            // Diagonals
            KeyCode::KeyY | KeyCode::Numpad7 => {
                move_result = Some(try_move_player(
                    &mut commands, &mut gamelog, &map, player_entity, player_x, player_y,
                    -1, -1, &other_combat_stats, &mut doors, &bystanders,
                ));
                player_acted = true;
            }
            KeyCode::KeyU | KeyCode::Numpad9 => {
                move_result = Some(try_move_player(
                    &mut commands, &mut gamelog, &map, player_entity, player_x, player_y,
                    1, -1, &other_combat_stats, &mut doors, &bystanders,
                ));
                player_acted = true;
            }
            KeyCode::KeyM | KeyCode::Numpad3 => {
                move_result = Some(try_move_player(
                    &mut commands, &mut gamelog, &map, player_entity, player_x, player_y,
                    1, 1, &other_combat_stats, &mut doors, &bystanders,
                ));
                player_acted = true;
            }
            KeyCode::KeyN | KeyCode::Numpad1 => {
                move_result = Some(try_move_player(
                    &mut commands, &mut gamelog, &map, player_entity, player_x, player_y,
                    -1, 1, &other_combat_stats, &mut doors, &bystanders,
                ));
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
                // Can't heal while starving
                if hunger.state == HungerState::Starving {
                    can_heal = false;
                }
                // Heal 1 HP if no monsters visible and not starving
                if can_heal && player_stats.hp < player_stats.max_hp {
                    player_stats.hp = (player_stats.hp + 1).min(player_stats.max_hp);
                    gamelog.entries.push("You rest and recover 1 HP.".to_string());
                }
                player_acted = true;
            }

            // Pickup
            KeyCode::KeyG => {
                if get_item(&mut commands, &mut gamelog, player_entity, player_x, player_y, &items) {
                    player_acted = true;
                }
            }

            // Inventory
            KeyCode::KeyI => {
                next_state.set_if_different(RunState::ShowInventory);
            }

            // Drop item
            KeyCode::KeyD => {
                next_state.set_if_different(RunState::ShowDropItem);
            }

            // Remove equipment
            KeyCode::KeyR => {
                next_state.set_if_different(RunState::ShowRemoveItem);
            }

            // Town ledger; depth 1 is the town
            KeyCode::KeyT => {
                if map.depth == 1 {
                    next_state.set_if_different(RunState::ShowLedger);
                } else {
                    gamelog.entries.push("The town ledger is kept in town.".to_string());
                }
            }

            // Go down stairs
            KeyCode::Period => {
                let idx = xy_idx(player_x, player_y);
                if map.tiles[idx] == TileType::DownStairs {
                    next_state.set_if_different(RunState::NextLevel);
                } else {
                    gamelog.entries.push("There are no stairs here.".to_string());
                }
            }
            _ => {}
        }
    }

    // Process movement results and update positions
    if let Some(result) = move_result {
        match result {
            MoveResult::Moved { new_x, new_y } => {
                let mut player_pos_query = position_set.p0();
                if let Ok(mut pos) = player_pos_query.single_mut() {
                    pos.x = new_x;
                    pos.y = new_y;
                }
            }
            MoveResult::SwappedWithBystander { bystander, new_x, new_y } => {
                // First update bystander position to player's old position
                {
                    let mut bystander_pos_query = position_set.p1();
                    if let Ok(mut bystander_pos) = bystander_pos_query.get_mut(bystander) {
                        bystander_pos.x = player_x;
                        bystander_pos.y = player_y;
                    }
                }
                // Then update player position
                {
                    let mut player_pos_query = position_set.p0();
                    if let Ok(mut pos) = player_pos_query.single_mut() {
                        pos.x = new_x;
                        pos.y = new_y;
                    }
                }
            }
            MoveResult::Attacked { .. } | MoveResult::OpenedDoor | MoveResult::Blocked => {
                // No position change needed
            }
        }
    }

    if player_acted {
        next_state.set_if_different(RunState::PlayerTurn);
    }
}
