use bevy::prelude::*;

use rand::RngExt;

use crate::{
    combat::{CombatStats, WantsToMelee},
    components::{Confusion, Name},
    effects::{EffectKind, Effects},
    game_flow::Mode,
    gamelog::GameLog,
    map::{Map, Position, TileType, MAP_HEIGHT, MAP_WIDTH},
    pathfinding,
    party::PartyMember,
    rng::GameRng,
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
/// goes as far along its path as one AP reaches, and an attack on a hero beside
/// it ends its turn. A confused monster's moves go to a random neighbour.
#[allow(clippy::too_many_arguments)]
pub fn monster_ai(
    mut commands: Commands,
    mut map: ResMut<Map>,
    mut gamelog: ResMut<GameLog>,
    mut effects: ResMut<Effects>,
    mut rng: ResMut<GameRng>,
    mode: Res<Mode>,
    mut monster_query: Query<
        (Entity, &mut Position, &mut Viewshed, &Name, &CombatStats, Has<Confusion>),
        (With<Monster>, Without<PartyMember>),
    >,
    party_query: Query<(Entity, &PartyMember, &Position)>,
) {
    let mut party: Vec<_> = party_query.iter().collect();
    party.sort_by_key(|(_, member, _)| member.slot);
    let party: Vec<(Entity, (i32, i32))> = party.into_iter().map(|(e, _, pos)| (e, (pos.x, pos.y))).collect();
    let free = |cell: (i32, i32)| !party.iter().any(|&(_, p)| p == cell);
    let combat = matches!(*mode, Mode::Combat { .. });

    for (entity, mut pos, mut viewshed, name, stats, confused) in &mut monster_query {
        if stats.hp <= 0 {
            continue;
        }
        let mut ap = if combat { AP_PER_ROUND } else { 1 };
        while ap > 0 {
            let here = (pos.x, pos.y);
            let target = monster_target(here, &party, |tile| viewshed.visible_tiles.contains(&tile));
            let beside = |(x, y): (i32, i32)| (x - here.0).abs() <= 1 && (y - here.1).abs() <= 1;
            if let Some((target, _)) = target.filter(|&(_, at)| beside(at)) {
                commands.entity(entity).insert(WantsToMelee { target });
                break;
            }

            let next = if confused {
                gamelog.entries.push(format!("{} is confused and stumbles around.", name.name));
                effects.play(EffectKind::Confusion, here.0, here.1);
                confused_step(&map, here, free, |n| rng.0.random_range(0..n))
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
                    steps.take_while(|&cell| range.ap(cell).is_some()).last()
                } else {
                    steps.next().filter(|&(x, y)| !map.blocked_tiles[map.xy_idx(x, y)] && free((x, y)))
                }
            };
            let Some(next) = next else {
                break;
            };
            let (from_idx, to_idx) = (map.xy_idx(here.0, here.1), map.xy_idx(next.0, next.1));
            map.blocked_tiles[from_idx] = false;
            map.blocked_tiles[to_idx] = true;
            (pos.x, pos.y) = next;
            viewshed.dirty = true;
            ap = Action::Move.spend(ap).unwrap_or(0);
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
