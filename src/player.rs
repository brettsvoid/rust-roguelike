use std::collections::VecDeque;

use bevy::{
    input::{keyboard::KeyboardInput, ButtonState},
    prelude::*,
};

use rand::RngExt;

use crate::{
    bystander::Bystander,
    combat::{CombatStats, WantsToMelee},
    components::{BlocksTile, BlocksVisibility, Confusion, Door, HungerClock, HungerState, Item, WantsToPickupItem},
    debug::DebugMode,
    economy::town::Vendor,
    entity_render::{EntityShape, Renderable},
    game_flow::{HeroAction, Mode, TurnEnded},
    gamelog::GameLog,
    map::{is_stairs, tile_walkable, xy_idx, Map, Position},
    map_indexing,
    monsters::Monster,
    party::{follow, next_unit, ActiveUnit, PartyMember},
    pathfinding::a_star_explored,
    picking::{self, HoveredTile, TileMarks, FULL_TILE},
    rng::GameRng,
    tactics::{confused_step, move_range, Action, MoveRange},
    viewshed::Viewshed,
    RunState,
};

/// Result of attempting to move the active unit
enum MoveResult {
    /// The unit moved to the new position
    Moved { new_x: i32, new_y: i32 },
    /// The unit attacked a target
    Attacked {
        #[allow(dead_code)]
        target: Entity,
    },
    /// The unit swapped places with a bystander or another member
    Swapped { other: Entity, new_x: i32, new_y: i32 },
    /// The unit opened a door (doesn't move)
    OpenedDoor,
    /// Movement was blocked
    Blocked,
}

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WalkPath>()
            .init_resource::<MoveRange>()
            .add_systems(OnEnter(RunState::PreRun), |mut walk: ResMut<WalkPath>| *walk = WalkPath::default())
            .add_systems(
                Update,
                (
                    update_move_range.after(map_indexing::map_indexing_system),
                    handle_player_input
                        .run_if(in_state(RunState::AwaitingInput))
                        .after(picking::update_hovered_tile)
                        .after(update_move_range),
                    show_path_preview.after(picking::update_hovered_tile).after(update_move_range),
                    show_move_range.after(update_move_range),
                ),
            );
    }
}

/// The active hero's move range while the party phase waits for input. Kept
/// through a party action and the inventory screens, so the overlay does not
/// flicker; empty otherwise.
fn update_move_range(
    mut range: ResMut<MoveRange>,
    state: Res<State<RunState>>,
    mode: Res<Mode>,
    active: Res<ActiveUnit>,
    map: Res<Map>,
    party: Query<(Entity, &Position, &CombatStats), With<PartyMember>>,
) {
    let combat = matches!(*mode, Mode::Combat { .. });
    let hero = active.0.and_then(|unit| party.get(unit).ok());
    let new = match (state.get(), hero) {
        (RunState::AwaitingInput, Some((unit, pos, stats))) if combat => {
            let others: Vec<_> = party.iter().filter(|h| h.0 != unit).map(|h| (h.1.x, h.1.y)).collect();
            let open = |cell: (i32, i32)| map.revealed_tiles[map.xy_idx(cell.0, cell.1)] && !others.contains(&cell);
            move_range(&map, (pos.x, pos.y), stats.movement, mode.ap_left(unit), open)
        }
        (
            RunState::PartyAction
            | RunState::ShowInventory
            | RunState::ShowDropItem
            | RunState::ShowRemoveItem
            | RunState::ShowTargeting,
            _,
        ) if combat => return,
        _ => MoveRange::default(),
    };
    range.set_if_neq(new);
}

#[derive(Component)]
struct RangeMark;

/// The move range as tile marks: the one-AP zone and the two-AP zone in their
/// own colours, inset so the tile grid shows between them.
fn show_move_range(
    mut commands: Commands,
    range: Res<MoveRange>,
    map: Res<Map>,
    marks: Res<TileMarks>,
    shown: Query<Entity, With<RangeMark>>,
) {
    if !range.is_changed() {
        return;
    }
    for mark in &shown {
        commands.entity(mark).despawn();
    }
    let inset = Rect::from_center_half_size(Vec2::ZERO, Vec2::splat(0.45));
    for (cell, ap) in range.cells() {
        let material = if ap == 1 { &marks.move_one } else { &marks.move_two };
        commands.spawn((RangeMark, marks.mark(material, &map, cell, inset, 0.0)));
    }
}

/// A click-to-move walk under way.
#[derive(Resource, Default)]
pub struct WalkPath {
    /// The cells still to step onto, nearest first.
    pub cells: VecDeque<(i32, i32)>,
    /// The click was on stairs, so take them on arrival.
    pub descend: bool,
    /// The click was on a vendor, so open their shop on arrival (`ui::menus::shop`).
    pub trade_with: Option<Vendor>,
}

impl WalkPath {
    pub fn is_active(&self) -> bool {
        !self.cells.is_empty() || self.descend || self.trade_with.is_some()
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
/// is in view, or the next cell is no longer beside the walking unit.
fn walk_step(from: (i32, i32), path: &VecDeque<(i32, i32)>, hostile_in_view: bool) -> Option<(i32, i32)> {
    let &(x, y) = path.front()?;
    let step = (x - from.0, y - from.1);
    (!hostile_in_view && step != (0, 0) && step.0.abs() <= 1 && step.1.abs() <= 1).then_some(step)
}

#[derive(Component)]
struct PathPreview;

/// Marks the walk a click on the hovered tile would take; in combat, the
/// path of the move within range. Redrawn only when the active unit, the
/// hovered tile or the move range changes.
#[allow(clippy::too_many_arguments)]
fn show_path_preview(
    mut commands: Commands,
    state: Res<State<RunState>>,
    hovered: Res<HoveredTile>,
    map: Res<Map>,
    marks: Res<TileMarks>,
    mode: Res<Mode>,
    range: Res<MoveRange>,
    active: Res<ActiveUnit>,
    party: Query<&Position, With<PartyMember>>,
    previews: Query<Entity, With<PathPreview>>,
    mut shown: Local<Option<((i32, i32), (i32, i32), bool)>>,
) {
    // Kept up through the turn states so a walk does not flicker.
    let in_turn = matches!(
        state.get(),
        RunState::AwaitingInput | RunState::PartyAction | RunState::EnemyTurn
    );
    let wanted = active
        .0
        .and_then(|unit| party.get(unit).ok())
        .map(|p| (p.x, p.y))
        .zip(hovered.tile)
        .filter(|_| in_turn)
        // The field of view may reveal the hovered tile after this runs, so
        // redraw when it does or the preview depends on system order.
        .map(|(from, to)| (from, to, map.revealed_tiles[map.xy_idx(to.0, to.1)]));
    if wanted == *shown && !range.is_changed() {
        return;
    }
    *shown = wanted;
    for preview in &previews {
        commands.entity(preview).despawn();
    }
    let Some((from, to, _)) = wanted else {
        return;
    };
    let cells = match *mode {
        Mode::Combat { .. } => range.path(to),
        Mode::Explore => walk_path(&map, from, to).map(Vec::from).unwrap_or_default(),
    };
    for cell in cells {
        // Just above the move range marks.
        commands.spawn((PathPreview, marks.mark(&marks.path, &map, cell, FULL_TILE, 0.005)));
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
    combat_stats: &Query<&CombatStats, Without<PartyMember>>,
    doors: &mut Query<(Entity, &mut Door, &mut Renderable), Without<PartyMember>>,
    swappable: &Query<&mut Position, Or<(With<Bystander>, With<PartyMember>)>>,
) -> MoveResult {
    let dest_x = player_x + delta_x;
    let dest_y = player_y + delta_y;

    // Bounds check - can't move outside map
    if dest_x < 0 || dest_x >= map.width || dest_y < 0 || dest_y >= map.height {
        return MoveResult::Blocked;
    }

    let destination_idx = xy_idx(dest_x, dest_y);

    // Bystanders and other members swap places instead of being attacked
    for potential_target in map.tile_content[destination_idx].iter() {
        if swappable.contains(*potential_target) {
            return MoveResult::Swapped {
                other: *potential_target,
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
    items: &Query<(Entity, &Position), (With<Item>, Without<PartyMember>, Without<Bystander>)>,
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
    mut gamelog: ResMut<GameLog>,
    mut active: ResMut<ActiveUnit>,
    mut party: Query<(Entity, &PartyMember, &mut CombatStats, &HungerClock, &Viewshed)>,
    mut next_state: ResMut<NextState<RunState>>,
    other_combat_stats: Query<&CombatStats, Without<PartyMember>>,
    items: Query<(Entity, &Position), (With<Item>, Without<PartyMember>, Without<Bystander>)>,
    monsters: Query<&Position, (With<Monster>, Without<PartyMember>, Without<Bystander>)>,
    mut doors: Query<(Entity, &mut Door, &mut Renderable), Without<PartyMember>>,
    mut positions: Query<&mut Position, Or<(With<Bystander>, With<PartyMember>)>>,
    (debug_mode, mode, mut turn_ended, mut hero_action, range, confused, mut rng): (
        Res<DebugMode>,
        Res<Mode>,
        ResMut<TurnEnded>,
        ResMut<HeroAction>,
        Res<MoveRange>,
        Query<(), With<Confusion>>,
        ResMut<GameRng>,
    ),
) {
    // Don't process player input if debug console is open
    if debug_mode.show_console {
        return;
    }

    let Some(player_entity) = active.0 else {
        return;
    };
    let Ok(&Position { x: player_x, y: player_y }) = positions.get(player_entity) else {
        return;
    };
    // Who can be selected: the living, and in combat only those yet to act.
    let mut roster: Vec<(usize, Entity, bool)> = party
        .iter()
        .map(|(entity, member, stats, ..)| (member.slot, entity, stats.hp > 0 && mode.can_act(entity)))
        .collect();
    roster.sort_by_key(|m| m.0);
    // Seen by any member: `Map::visible_tiles` would count every tile under no-fog.
    let hostile_in_view = monsters
        .iter()
        .any(|pos| party.iter().any(|(.., viewshed)| viewshed.visible_tiles.contains(&(pos.x, pos.y))));
    let Ok((_, _, mut player_stats, hunger, _)) = party.get_mut(player_entity) else {
        return;
    };
    let mut select = None;

    let mut player_acted = false;
    let mut action = None;
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
                // No healing with a hostile in view or while starving
                let can_heal = !hostile_in_view && hunger.state != HungerState::Starving;
                // Heal 1 HP if no monsters visible and not starving
                if can_heal && player_stats.hp < player_stats.max_hp {
                    player_stats.hp = (player_stats.hp + 1).min(player_stats.max_hp);
                    gamelog.entries.push("You rest and recover 1 HP.".to_string());
                }
                player_acted = true;
                action = Some(Action::Wait);
            }

            // Pickup
            KeyCode::KeyG => {
                if get_item(&mut commands, &mut gamelog, player_entity, player_x, player_y, &items) {
                    player_acted = true;
                    action = Some(Action::UseItem);
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

            // Town ledger
            KeyCode::KeyT => {
                if map.depth == crate::levels::TOWN_DEPTH {
                    next_state.set_if_different(RunState::ShowLedger);
                } else {
                    gamelog.entries.push("The town ledger is kept in town.".to_string());
                }
            }

            // Select the next member who can act
            KeyCode::Tab => {
                select = next_unit(&roster, Some(player_entity));
            }

            // End the party phase; outside combat nothing happens
            KeyCode::KeyE => {
                turn_ended.0 = true;
                player_acted = true;
            }

            // Take the stairs
            KeyCode::Period => {
                let idx = xy_idx(player_x, player_y);
                if is_stairs(map.tiles[idx]) {
                    next_state.set_if_different(RunState::NextLevel);
                } else {
                    gamelog.entries.push("There are no stairs here.".to_string());
                }
            }
            _ => {}
        }
    }

    // Left click: on another member selects them, on the active unit picks up
    // or descends, anywhere else walks there. A walk carries on one step per
    // turn while it has steps left.
    let clicked = hovered.tile.filter(|_| !player_acted && mouse.just_pressed(MouseButton::Left));
    let clicked_member = clicked.and_then(|target| {
        roster.iter().find(|&&(_, entity, _)| {
            entity != player_entity && positions.get(entity).is_ok_and(|p| (p.x, p.y) == target)
        })
    });
    let mut step = None;
    if let Some(&(_, member, selectable)) = clicked_member {
        select = selectable.then_some(member);
    } else if !player_acted && mouse.just_pressed(MouseButton::Left) {
        if let Some(target) = hovered.tile {
            *walk = WalkPath::default();
            if target == (player_x, player_y) {
                if items.iter().any(|(_, pos)| (pos.x, pos.y) == target) {
                    player_acted = get_item(&mut commands, &mut gamelog, player_entity, player_x, player_y, &items);
                    action = Some(Action::UseItem);
                } else if is_stairs(map.tiles[xy_idx(player_x, player_y)]) {
                    next_state.set_if_different(RunState::NextLevel);
                }
            } else if matches!(*mode, Mode::Combat { .. }) {
                // One action: a move within range, or an attack or a door beside the hero.
                let beside = (target.0 - player_x).abs() <= 1 && (target.1 - player_y).abs() <= 1;
                if let Some(ap) = range.ap(target) {
                    let to = if confused.contains(player_entity) {
                        gamelog.entries.push("You are confused and stumble.".to_string());
                        let reachable = |cell| range.ap(cell).is_some();
                        confused_step(&map, (player_x, player_y), reachable, |n| rng.0.random_range(0..n))
                    } else {
                        Some(target)
                    };
                    if let Some((new_x, new_y)) = to {
                        move_result = Some(MoveResult::Moved { new_x, new_y });
                    }
                    player_acted = true;
                    action = Some(Action::moving(ap));
                } else if beside {
                    step = Some((target.0 - player_x, target.1 - player_y));
                } else {
                    gamelog.entries.push("Out of range.".to_string());
                }
            } else if let Some(path) = walk_path(&map, (player_x, player_y), target) {
                step = walk_step((player_x, player_y), &path, false);
                walk.cells = path;
                walk.descend = is_stairs(map.tiles[xy_idx(target.0, target.1)]);
            }
        }
    } else if !player_acted && walk.cells.is_empty() && walk.descend {
        *walk = WalkPath::default();
        if is_stairs(map.tiles[xy_idx(player_x, player_y)]) {
            next_state.set_if_different(RunState::NextLevel);
        }
    } else if !player_acted && walk.is_active() {
        step = walk_step((player_x, player_y), &walk.cells, hostile_in_view);
        if step.is_none() {
            *walk = WalkPath::default();
        }
    }
    if let Some((dx, dy)) = step {
        let result = try_move_player(
            &mut commands, &mut gamelog, &map, player_entity, player_x, player_y,
            dx, dy, &other_combat_stats, &mut doors, &positions,
        );
        match result {
            MoveResult::Moved { .. } | MoveResult::Swapped { .. } => {
                walk.cells.pop_front();
            }
            // Walk through the door next turn.
            MoveResult::OpenedDoor => {}
            MoveResult::Attacked { .. } | MoveResult::Blocked => *walk = WalkPath::default(),
        }
        player_acted = !matches!(result, MoveResult::Blocked);
        action = Some(if matches!(result, MoveResult::Attacked { .. }) { Action::Attack } else { Action::Move });
        move_result = Some(result);
    }

    // Process movement results and update positions
    match move_result {
        Some(MoveResult::Moved { new_x, new_y }) => {
            if let Ok(mut pos) = positions.get_mut(player_entity) {
                (pos.x, pos.y) = (new_x, new_y);
            }
        }
        Some(MoveResult::Swapped { other, new_x, new_y }) => {
            if let Ok(mut pos) = positions.get_mut(other) {
                (pos.x, pos.y) = (player_x, player_y);
            }
            if let Ok(mut pos) = positions.get_mut(player_entity) {
                (pos.x, pos.y) = (new_x, new_y);
            }
        }
        _ => {}
    }

    // Exploring, the rest of the party follows the leader's step.
    let stepped = matches!(move_result, Some(MoveResult::Moved { .. } | MoveResult::Swapped { .. }));
    if *mode == Mode::Explore && stepped {
        let leader_to = positions.get(player_entity).map(|p| (p.x, p.y)).unwrap_or((player_x, player_y));
        let followers: Vec<(Entity, (i32, i32))> = roster
            .iter()
            .filter(|m| m.2 && m.1 != player_entity)
            .filter_map(|m| positions.get(m.1).ok().map(|p| (m.1, (p.x, p.y))))
            .collect();
        let cells: Vec<_> = followers.iter().map(|f| f.1).collect();
        let step_towards = |from: (i32, i32), to: (i32, i32)| {
            let path = a_star_explored(&map, map.xy_idx(from.0, from.1), map.xy_idx(to.0, to.1))?;
            let next = *path.get(1)?;
            (!map.blocked_tiles[next]).then_some((next as i32 % map.width, next as i32 / map.width))
        };
        let moved = follow((player_x, player_y), leader_to, &cells, step_towards);
        for ((follower, _), cell) in followers.iter().zip(moved) {
            if let Ok(mut pos) = positions.get_mut(*follower) {
                if (pos.x, pos.y) != cell {
                    (pos.x, pos.y) = cell;
                }
            }
        }
    }

    // Selecting a unit is free: no turn passes, and its walk starts afresh.
    if let Some(unit) = select.filter(|&unit| unit != player_entity) {
        *walk = WalkPath::default();
        active.0 = Some(unit);
    }

    if player_acted {
        hero_action.0 = action;
        next_state.set_if_different(RunState::PartyAction);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::TileType;

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
