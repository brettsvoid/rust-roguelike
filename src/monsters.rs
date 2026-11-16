use bevy::prelude::*;

use rand::RngExt;

use crate::{
    combat::{in_reach, CombatStats, Fighter, Strikes, WantsToAttack},
    components::{Confusion, Name},
    cover::{exposure, Exposure},
    effects::EffectKind,
    game_flow::Mode,
    map::{Map, Position, TileType, MAP_HEIGHT, MAP_WIDTH},
    overwatch::{walk_under_watch, Hunkered, Overwatch},
    pathfinding,
    party::PartyMember,
    presentation::{Beat, BETWEEN_UNITS_SECS},
    tactics::{confused_step, move_range, Action, AP_PER_ROUND},
    viewshed::Viewshed,
    RunState,
};

#[derive(Component, Debug)]
pub struct Monster;

pub struct MonstersPlugin;
impl Plugin for MonstersPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            update_blocked_tiles.run_if(in_state(RunState::EnemyTurn)),
        );
    }
}

fn update_blocked_tiles(mut map: ResMut<Map>, monster_query: Query<&Position, With<Monster>>) {
    // Clear blocked tiles and re-populate from walls
    let size = MAP_WIDTH * MAP_HEIGHT;
    for idx in 0..size {
        map.blocked_tiles[idx] = map.tiles[idx] == TileType::Wall;
    }

    // Block tiles with monsters
    for pos in &monster_query {
        let idx = map.xy_idx(pos.x, pos.y);
        map.blocked_tiles[idx] = true;
    }
}

/// The member a monster at `from` goes for: the nearest one it can see or is
/// next to, by Chebyshev distance. `party` is in slot order, so a tie goes to
/// the lower slot.
pub fn monster_target<T: Copy>(
    from: (i32, i32),
    party: &[(T, (i32, i32))],
    sees: impl Fn((i32, i32)) -> bool,
) -> Option<(T, (i32, i32))> {
    let distance = |(x, y): (i32, i32)| (x - from.0).abs().max((y - from.1).abs());
    party
        .iter()
        .copied()
        .filter(|&(_, pos)| distance(pos) <= 1 || sees(pos))
        .min_by_key(|&(_, pos)| distance(pos))
}

/// Each living monster's turn. Exploring, it takes one step or attacks. In
/// combat it spends `AP_PER_ROUND` action points by the heroes' rules: a move
/// goes as far along its path as one AP reaches, and an attack on a hero in
/// reach of its weapon ends its turn. A confused monster's moves go to a
/// random neighbour. Every move walks under the heroes' overwatch
/// (`overwatch::walk_under_watch`). Until C07's tactics, two stand-in rules in
/// combat: a monster at a third of its hit points or less in cover from the
/// hero it targets hunkers down instead of moving, and one that sees a hero
/// but can neither attack nor move closer with AP left goes on overwatch.
pub fn monster_ai(
    mut strikes: Strikes,
    mut map: ResMut<Map>,
    mode: Res<Mode>,
    mut monster_query: Query<
        (Entity, &mut Position, &mut Viewshed, &Name, &CombatStats, Has<Confusion>),
        (With<Monster>, Without<PartyMember>),
    >,
    party_query: Query<(Entity, &PartyMember, &Position)>,
    on_watch: Query<(Entity, &Name, &CombatStats, &Position), (With<Overwatch>, With<PartyMember>, Without<Monster>)>,
) {
    let mut party: Vec<_> = party_query.iter().collect();
    party.sort_by_key(|(_, member, _)| member.slot);
    let party: Vec<(Entity, (i32, i32))> = party.into_iter().map(|(e, _, pos)| (e, (pos.x, pos.y))).collect();
    let free = |cell: (i32, i32)| !party.iter().any(|&(_, p)| p == cell);
    let combat = matches!(*mode, Mode::Combat { .. });
    let mut watchers: Vec<_> = on_watch
        .iter()
        .map(|(entity, name, stats, pos)| Fighter { entity, name: &name.name, stats, at: (pos.x, pos.y) })
        .collect();

    for (entity, mut pos, mut viewshed, name, stats, confused) in &mut monster_query {
        if stats.hp <= 0 {
            continue;
        }
        let mut ap = if combat { AP_PER_ROUND } else { 1 };
        // In combat a monster with a hero to go for acts in view: the camera
        // glides there if its tile is off-screen (`presentation::play_beats`).
        let acts = combat && monster_target((pos.x, pos.y), &party, |tile| viewshed.visible_tiles.contains(&tile)).is_some();
        if acts {
            strikes.presentation.push(Beat::CameraFocus { at: (pos.x, pos.y) });
        }
        while ap > 0 {
            let here = (pos.x, pos.y);
            let target = monster_target(here, &party, |tile| viewshed.visible_tiles.contains(&tile));
            let weapon = strikes.arms.weapon(entity);
            if let Some((target, _)) = target.filter(|&(_, at)| in_reach(&map, &weapon, here, at).is_ok()) {
                strikes.commands.entity(entity).insert(WantsToAttack { target });
                break;
            }
            let tactical = combat && !confused;
            let wounded = stats.hp * 3 <= stats.max_hp;
            if tactical && wounded && target.is_some_and(|(_, at)| matches!(exposure(&map, here, at), Exposure::Covered { .. })) {
                strikes.commands.entity(entity).insert(Hunkered);
                strikes.log.entries.push(format!("{} hunkers down.", name.name));
                break;
            }

            let path: Vec<(i32, i32)> = if confused {
                strikes.log.entries.push(format!("{} is confused and stumbles around.", name.name));
                strikes.effects.play(EffectKind::Confusion, here.0, here.1);
                let rng = &mut strikes.rng;
                confused_step(&map, here, free, |n| rng.0.random_range(0..n)).into_iter().collect()
            } else {
                let Some((_, (tx, ty))) = target else {
                    break;
                };
                // Path ignoring other entities so monsters keep chasing.
                let path = pathfinding::a_star_ignoring_entities(&map, map.xy_idx(here.0, here.1), map.xy_idx(tx, ty))
                    .unwrap_or_default();
                let mut steps = path.into_iter().skip(1).map(|i| (i as i32 % map.width, i as i32 / map.width));
                if combat {
                    let range = move_range(&map, here, stats.movement, 1, free);
                    steps.take_while(|&cell| range.ap(cell).is_some()).collect()
                } else {
                    steps.next().filter(|&(x, y)| !map.blocked_tiles[map.xy_idx(x, y)] && free((x, y))).into_iter().collect()
                }
            };
            if path.is_empty() {
                if tactical && target.is_some() {
                    strikes.commands.entity(entity).insert(Overwatch);
                    strikes.log.entries.push(format!("{} goes on overwatch.", name.name));
                }
                break;
            }
            let mover = Fighter { entity, name: &name.name, stats, at: here };
            let (end, alive) = walk_under_watch(&mut strikes, &map, &mover, &path, &mut watchers);
            let (from_idx, to_idx) = (map.xy_idx(here.0, here.1), map.xy_idx(end.0, end.1));
            map.blocked_tiles[from_idx] = false;
            map.blocked_tiles[to_idx] = true;
            (pos.x, pos.y) = end;
            viewshed.dirty = true;
            ap = Action::Move.spend(ap).unwrap_or(0);
            if !alive {
                break;
            }
        }
        if acts {
            strikes.presentation.push(Beat::Wait { secs: BETWEEN_UNITS_SECS });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_monster_goes_for_the_nearest_member_it_can_see() {
        let party = [('a', (10, 0)), ('b', (4, 0)), ('c', (2, 0))];
        let sees_all = |_| true;
        assert_eq!(monster_target((0, 0), &party, sees_all), Some(('c', (2, 0))));
        // The nearest is out of sight, so the next nearest it can see.
        let not_c = |tile| tile != (2, 0);
        assert_eq!(monster_target((0, 0), &party, not_c), Some(('b', (4, 0))));
        assert_eq!(monster_target((0, 0), &party, |_| false), None);
    }

    #[test]
    fn a_member_next_to_a_monster_is_a_target_even_unseen() {
        let party = [('a', (1, 1)), ('b', (0, 3))];
        assert_eq!(monster_target((0, 0), &party, |_| false), Some(('a', (1, 1))));
    }

    #[test]
    fn a_tie_goes_to_the_lower_slot() {
        let party = [('a', (0, 3)), ('b', (3, 3))];
        assert_eq!(monster_target((0, 0), &party, |_| true), Some(('a', (0, 3))));
    }
}
