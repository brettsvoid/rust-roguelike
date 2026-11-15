use bevy::prelude::*;

use crate::{
    combat::{CombatStats, WantsToMelee},
    components::{Confusion, Name},
    gamelog::GameLog,
    map::{Map, Position, TileType, MAP_HEIGHT, MAP_WIDTH},
    effects::{EffectKind, Effects},
    pathfinding,
    party::PartyMember,
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

pub fn monster_ai(
    mut commands: Commands,
    mut map: ResMut<Map>,
    mut gamelog: ResMut<GameLog>,
    mut effects: ResMut<Effects>,
    mut monster_query: Query<
        (Entity, &mut Position, &mut Viewshed, &Name, &CombatStats, Option<&mut Confusion>),
        (With<Monster>, Without<PartyMember>),
    >,
    party_query: Query<(Entity, &PartyMember, &Position)>,
) {
    let mut party: Vec<_> = party_query.iter().collect();
    party.sort_by_key(|(_, member, _)| member.slot);
    let party: Vec<(Entity, (i32, i32))> = party.into_iter().map(|(e, _, pos)| (e, (pos.x, pos.y))).collect();

    for (entity, mut pos, mut viewshed, name, stats, confusion) in &mut monster_query {
        if stats.hp <= 0 {
            continue;
        }

        // Handle confusion
        if let Some(mut confused) = confusion {
            confused.turns -= 1;
            if confused.turns < 1 {
                commands.entity(entity).remove::<Confusion>();
                gamelog.entries.push(format!("{} is no longer confused.", name.name));
            } else {
                gamelog.entries.push(format!("{} is confused and stumbles around.", name.name));

                effects.play(EffectKind::Confusion, pos.x, pos.y);
            }
            continue; // Skip normal AI while confused
        }

        let Some((target, (tx, ty))) =
            monster_target((pos.x, pos.y), &party, |tile| viewshed.visible_tiles.contains(&tile))
        else {
            continue;
        };
        if (tx - pos.x).abs() <= 1 && (ty - pos.y).abs() <= 1 {
            commands.entity(entity).insert(WantsToMelee { target });
            continue;
        }

        let monster_idx = map.xy_idx(pos.x, pos.y);
        let target_idx = map.xy_idx(tx, ty);
        // Find path to the target (ignoring other entities so monsters keep chasing)
        if let Some(path) = pathfinding::a_star_ignoring_entities(&map, monster_idx, target_idx) {
            // Move one step toward the target (path[0] is current position)
            if path.len() > 1 {
                let next_idx = path[1];
                let next = ((next_idx % map.width as usize) as i32, (next_idx / map.width as usize) as i32);
                // Only move if destination is not blocked, by the map or another member
                if !map.blocked_tiles[next_idx] && !party.iter().any(|&(_, p)| p == next) {
                    // Unblock old position, block new position
                    map.blocked_tiles[monster_idx] = false;
                    map.blocked_tiles[next_idx] = true;
                    (pos.x, pos.y) = next;
                    viewshed.dirty = true;
                }
            }
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
