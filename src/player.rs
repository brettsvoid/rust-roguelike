use std::collections::VecDeque;

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
    entity_render::{EntityShape, Renderable},
    gamelog::GameLog,
    map::{tile_walkable, xy_idx, Map, Position, TileType},
    monsters::Monster,
    pathfinding::a_star_explored,
    picking::{self, HoveredTile, TileMarks, FULL_TILE},
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
        app.init_resource::<WalkPath>()
            .add_systems(OnEnter(RunState::PreRun), |mut walk: ResMut<WalkPath>| *walk = WalkPath::default())
            .add_systems(
                Update,
                (
                    handle_player_input
                        .run_if(in_state(RunState::AwaitingInput))
                        .after(picking::update_hovered_tile),
                    show_path_preview.after(picking::update_hovered_tile),
                ),
            );
    }
}

/// A click-to-move walk under way.
#[derive(Resource, Default)]
pub struct WalkPath {
    /// The cells still to step onto, nearest first.
    pub cells: VecDeque<(i32, i32)>,
    /// The click was on the down stairs, so take them on arrival.
    pub descend: bool,
}

impl WalkPath {
    pub fn is_active(&self) -> bool {
        !self.cells.is_empty() || self.descend
    }
}

/// The path from `from` to a clicked cell, without `from`. `None` unless the
/// cell is explored, walkable and reachable through explored tiles.
fn walk_path(map: &Map, from: (i32, i32), to: (i32, i32)) -> Option<VecDeque<(i32, i32)>> {
    let end = map.xy_idx(to.0, to.1);
    if from == to || !map.revealed_tiles[end] || !tile_walkable(map.tiles[end]) {
        return None;
    }
    let path = a_star_explored(map, map.xy_idx(from.0, from.1), end)?;
    Some(path.into_iter().skip(1).map(|i| (i as i32 % map.width, i as i32 / map.width)).collect())
}

/// This turn's step of a walk, or `None` to stop: the walk is over, a hostile
/// is in view, or the next cell is no longer beside the player.
fn walk_step(from: (i32, i32), path: &VecDeque<(i32, i32)>, hostile_in_view: bool) -> Option<(i32, i32)> {
    let &(x, y) = path.front()?;
    let step = (x - from.0, y - from.1);
    (!hostile_in_view && step != (0, 0) && step.0.abs() <= 1 && step.1.abs() <= 1).then_some(step)
}

#[derive(Component)]
struct PathPreview;

/// Marks the walk a click on the hovered tile would take. Redrawn only when
/// the player or the hovered tile changes.
fn show_path_preview(
    mut commands: Commands,
    state: Res<State<RunState>>,
    hovered: Res<HoveredTile>,
    map: Res<Map>,
    marks: Res<TileMarks>,
    player: Query<&Position, With<Player>>,
    previews: Query<Entity, With<PathPreview>>,
    mut shown: Local<Option<((i32, i32), (i32, i32))>>,
) {
    // Kept up through the turn states so a walk does not flicker.
    let in_turn = matches!(
        state.get(),
        RunState::AwaitingInput | RunState::PlayerTurn | RunState::MonsterTurn
    );
    let wanted = player
        .single()
        .ok()
        .map(|p| (p.x, p.y))
        .zip(hovered.tile)
        .filter(|_| in_turn);
    if wanted == *shown {
        return;
    }
    *shown = wanted;
    for preview in &previews {
        commands.entity(preview).despawn();
    }
    let Some((from, to)) = wanted else {
        return;
    };
    for cell in walk_path(&map, from, to).unwrap_or_default() {
        commands.spawn((PathPreview, marks.mark(&marks.path, &map, cell, FULL_TILE, 0.0)));
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
    doors: &mut Query<(Entity, &mut Door, &mut Renderable), Without<Player>>,
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
                door_glyph.glyph = "/".to_string();
                commands.entity(door_entity).insert(EntityShape::OpenDoor);
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

pub(crate) fn handle_player_input(
    mut commands: Commands,
    mut evr_kbd: MessageReader<KeyboardInput>,
    mouse: Res<ButtonInput<MouseButton>>,
    hovered: Res<HoveredTile>,
    mut walk: ResMut<WalkPath>,
    map: Res<Map>,
    debug_mode: Res<DebugMode>,
    mut gamelog: ResMut<GameLog>,
    mut player_query: Query<(Entity, &Viewshed, &mut CombatStats, &HungerClock), With<Player>>,
    mut next_state: ResMut<NextState<RunState>>,
    other_combat_stats: Query<&CombatStats, Without<Player>>,
    items: Query<(Entity, &Position), (With<Item>, Without<Player>, Without<Bystander>)>,
    monsters: Query<&Position, (With<Monster>, Without<Player>, Without<Bystander>)>,
    mut doors: Query<(Entity, &mut Door, &mut Renderable), Without<Player>>,
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

        // Any key stops a walk.
        *walk = WalkPath::default();

        match &ev.key_code {
            // Skip turn / wait
            KeyCode::Space => {
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

            // Drop item (D pans the camera)
            KeyCode::KeyX => {
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

    // Left click: on the player picks up or descends, anywhere else walks there.
    // A walk carries on one step per turn while it has steps left.
    let mut step = None;
    if !player_acted && mouse.just_pressed(MouseButton::Left) {
        if let Some(target) = hovered.tile {
            *walk = WalkPath::default();
            if target == (player_x, player_y) {
                if items.iter().any(|(_, pos)| (pos.x, pos.y) == target) {
                    player_acted = get_item(&mut commands, &mut gamelog, player_entity, player_x, player_y, &items);
                } else if map.tiles[xy_idx(player_x, player_y)] == TileType::DownStairs {
                    next_state.set_if_different(RunState::NextLevel);
                }
            } else if let Some(path) = walk_path(&map, (player_x, player_y), target) {
                step = walk_step((player_x, player_y), &path, false);
                walk.cells = path;
                walk.descend = map.tiles[xy_idx(target.0, target.1)] == TileType::DownStairs;
            }
        }
    } else if !player_acted && walk.cells.is_empty() && walk.descend {
        *walk = WalkPath::default();
        if map.tiles[xy_idx(player_x, player_y)] == TileType::DownStairs {
            next_state.set_if_different(RunState::NextLevel);
        }
    } else if !player_acted && walk.is_active() {
        let hostile_in_view = monsters
            .iter()
            .any(|pos| viewshed.visible_tiles.contains(&(pos.x, pos.y)));
        step = walk_step((player_x, player_y), &walk.cells, hostile_in_view);
        if step.is_none() {
            *walk = WalkPath::default();
        }
    }
    if let Some((dx, dy)) = step {
        let result = try_move_player(
            &mut commands, &mut gamelog, &map, player_entity, player_x, player_y,
            dx, dy, &other_combat_stats, &mut doors, &bystanders,
        );
        match result {
            MoveResult::Moved { .. } | MoveResult::SwappedWithBystander { .. } => {
                walk.cells.pop_front();
            }
            // Walk through the door next turn.
            MoveResult::OpenedDoor => {}
            MoveResult::Attacked { .. } | MoveResult::Blocked => *walk = WalkPath::default(),
        }
        player_acted = !matches!(result, MoveResult::Blocked);
        move_result = Some(result);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn path(cells: &[(i32, i32)]) -> VecDeque<(i32, i32)> {
        cells.iter().copied().collect()
    }

    #[test]
    fn walk_steps_towards_the_next_cell() {
        assert_eq!(walk_step((5, 5), &path(&[(6, 5), (7, 5)]), false), Some((1, 0)));
        assert_eq!(walk_step((5, 5), &path(&[(4, 6)]), false), Some((-1, 1)));
    }

    #[test]
    fn walk_stops_when_over_or_a_hostile_is_in_view() {
        assert_eq!(walk_step((5, 5), &path(&[]), false), None);
        assert_eq!(walk_step((5, 5), &path(&[(6, 5)]), true), None);
    }

    #[test]
    fn walk_stops_when_the_next_cell_is_not_beside_the_player() {
        assert_eq!(walk_step((5, 5), &path(&[(7, 5)]), false), None);
        assert_eq!(walk_step((5, 5), &path(&[(5, 5)]), false), None);
    }

    #[test]
    fn walk_path_needs_an_explored_walkable_cell() {
        let mut map = Map::new(7, 5, 2);
        for x in 1..6 {
            let idx = map.xy_idx(x, 2);
            map.tiles[idx] = TileType::Floor;
        }
        map.revealed_tiles.fill(true);
        assert_eq!(walk_path(&map, (1, 2), (4, 2)), Some(path(&[(2, 2), (3, 2), (4, 2)])));
        assert_eq!(walk_path(&map, (1, 2), (1, 2)), None);
        assert_eq!(walk_path(&map, (1, 2), (4, 1)), None);
        let idx = map.xy_idx(4, 2);
        map.revealed_tiles[idx] = false;
        assert_eq!(walk_path(&map, (1, 2), (4, 2)), None);
    }
}
