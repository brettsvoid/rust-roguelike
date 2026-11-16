use bevy::prelude::*;

use rand::RngExt;

use crate::{
    combat::{chebyshev, in_reach, CombatStats, Fighter, Strikes},
    components::{Confusion, Name},
    effects::EffectKind,
    gamelog::GameLog,
    enemy_ai::{plan, spread_alert, Alertness, Archetype, Foe, Lookout, Me, Plan, ALERT_RADIUS},
    game_flow::Mode,
    map::{Map, Position, TileType, MAP_HEIGHT, MAP_WIDTH},
    overwatch::{walk_under_watch, Hunkered, Overwatch},
    pathfinding,
    party::PartyMember,
    presentation::{Beat, BETWEEN_UNITS_SECS},
    tactics::{confused_step, move_range, AP_PER_ROUND},
    viewshed::Viewshed,
    RunState,
};

/// A hostile. Starts idle, fighting as a brute unless the raws say otherwise.
#[derive(Component, Debug)]
#[require(Alertness, Archetype)]
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

/// The steps from `from` towards `to` a monster takes on one AP: as far
/// along the path as its `movement` reaches in combat, one step exploring.
fn steps_towards(
    map: &Map,
    from: (i32, i32),
    to: (i32, i32),
    combat: bool,
    movement: i32,
    free: impl Fn((i32, i32)) -> bool,
) -> Vec<(i32, i32)> {
    // Path ignoring other entities so monsters keep chasing.
    let path = pathfinding::a_star_ignoring_entities(map, map.xy_idx(from.0, from.1), map.xy_idx(to.0, to.1))
        .unwrap_or_default();
    let mut steps = path.into_iter().skip(1).map(|i| (i as i32 % map.width, i as i32 / map.width));
    if combat {
        let range = move_range(map, from, movement, 1, &free);
        steps.take_while(|&cell| range.ap(cell).is_some()).collect()
    } else {
        steps.next().filter(|&(x, y)| !map.blocked_tiles[map.xy_idx(x, y)] && free((x, y))).into_iter().collect()
    }
}

type Monsters<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static mut Position,
        &'static mut Viewshed,
        &'static Name,
        &'static CombatStats,
        &'static Archetype,
        &'static mut Alertness,
        Has<Confusion>,
    ),
    (With<Monster>, Without<PartyMember>),
>;

type HeroData = (
    Entity,
    &'static PartyMember,
    &'static Name,
    &'static CombatStats,
    &'static Position,
    Has<Overwatch>,
    Has<Hunkered>,
);
type Heroes<'w, 's> = Query<'w, 's, HeroData, Without<Monster>>;
type HeroItem<'a> = bevy::ecs::query::QueryItem<'a, 'a, HeroData>;

/// The look round: each living monster's alertness (`enemy_ai::spread_alert`
/// over `party`, names and cells), written back and logged as it changes.
/// Returns each living monster, where it stands and its alertness now.
fn look_round(
    monsters: &mut Monsters,
    party: &[(&str, (i32, i32))],
    log: &mut GameLog,
    new_turn: bool,
) -> Vec<(Entity, (i32, i32), Alertness)> {
    let cells: Vec<_> = party.iter().map(|&(_, at)| at).collect();
    let (living, lookouts): (Vec<Entity>, Vec<Lookout>) = monsters
        .iter()
        .filter(|m| m.4.hp > 0)
        .map(|(entity, pos, view, .., alert, _)| {
            let at = (pos.x, pos.y);
            let sees = monster_target(at, party, |tile| view.visible_tiles.contains(&tile)).map(|(_, p)| p);
            (entity, Lookout { at, alert: *alert, sees })
        })
        .unzip();
    let alerts = spread_alert(&lookouts, &cells, new_turn);
    for ((&entity, look), &now) in living.iter().zip(&lookouts).zip(&alerts) {
        let Ok((_, _, _, name, _, _, mut alert, _)) = monsters.get_mut(entity) else {
            continue;
        };
        let hero = look.sees.and_then(|at| party.iter().find(|&&(_, p)| p == at)).map(|&(hero, _)| hero);
        match (look.alert.alerted(), now.alerted(), hero) {
            (false, true, Some(hero)) => log.entries.push(format!("{} spots {hero}!", name.name)),
            (false, true, None) => log.entries.push(format!("{} is alerted.", name.name)),
            (true, false, _) => log.entries.push(format!("{} loses the party's trail.", name.name)),
            _ => {}
        }
        *alert = now;
    }
    living.into_iter().zip(lookouts).zip(alerts).map(|((entity, look), alert)| (entity, look.at, alert)).collect()
}

/// The living heroes in slot order.
fn living_party<'a>(heroes: &'a Heroes) -> Vec<HeroItem<'a>> {
    let mut party: Vec<_> = heroes.iter().filter(|h| h.3.hp > 0).collect();
    party.sort_by_key(|h| h.1.slot);
    party
}

/// After each party action, monsters that now see a hero are alerted, and
/// their group with them, so a fight goes on even if the party steps out of
/// sight before the monsters' first turn.
pub fn spot_party(mut monsters: Monsters, heroes: Heroes, mut log: ResMut<GameLog>) {
    let party: Vec<_> = living_party(&heroes).iter().map(|h| (h.2.name.as_str(), (h.4.x, h.4.y))).collect();
    look_round(&mut monsters, &party, &mut log, false);
}

/// The monsters' turn. First the look round (`look_round`), counting a turn
/// of the hunt. Then each alerted one acts; an idle one waits. Exploring, it
/// attacks a hero in reach or takes a step towards the nearest it sees, or
/// towards where it last saw one. In combat it spends `AP_PER_ROUND` action
/// points on `enemy_ai::plan`: moves to the best tile, attacks, hunkers or
/// goes on overwatch (the last three end its turn); one that sees no hero
/// hunts, a move towards where it last saw one per AP, unless it is a coward
/// that would flee. A confused monster's moves go to a random neighbour.
/// Every move walks under the heroes' overwatch
/// (`overwatch::walk_under_watch`) and every attack resolves through
/// `Strikes::strike`.
pub fn monster_ai(mut strikes: Strikes, mut map: ResMut<Map>, mode: Res<Mode>, mut monsters: Monsters, heroes: Heroes) {
    let party = living_party(&heroes);
    let cells: Vec<(usize, (i32, i32))> = party.iter().enumerate().map(|(i, h)| (i, (h.4.x, h.4.y))).collect();
    // Hit points after this turn's blows so far, so nobody hits a fallen hero.
    let mut hero_hp: Vec<i32> = party.iter().map(|h| h.3.hp).collect();
    let free = |cell: (i32, i32)| !cells.iter().any(|&(_, p)| p == cell);
    let combat = matches!(*mode, Mode::Combat { .. });
    let mut watchers: Vec<_> = party
        .iter()
        .filter(|h| h.5)
        .map(|h| Fighter { entity: h.0, name: &h.2.name, stats: h.3, at: (h.4.x, h.4.y) })
        .collect();

    let named: Vec<_> = party.iter().map(|h| (h.2.name.as_str(), (h.4.x, h.4.y))).collect();
    let looked = look_round(&mut monsters, &named, &mut strikes.log, true);
    for &(entity, at, alertness) in &looked {
        if !alertness.alerted() {
            continue;
        }
        let allies_near = looked
            .iter()
            .filter(|&&(other, there, a)| other != entity && a.alerted() && chebyshev(at, there) <= ALERT_RADIUS)
            .count();
        let Ok((_, mut pos, mut viewshed, name, stats, &archetype, alert, confused)) = monsters.get_mut(entity) else {
            continue;
        };
        let weapon = strikes.arms.weapon(entity);
        // The heroes it knows of: in its sight or beside it, for the whole turn.
        let start = (pos.x, pos.y);
        let known: Vec<usize> = cells
            .iter()
            .filter(|&&(_, at)| chebyshev(start, at) <= 1 || viewshed.visible_tiles.contains(&at))
            .map(|&(h, _)| h)
            .collect();
        // In combat a monster with a hero in view acts in view: the camera
        // glides there if its tile is off-screen (`presentation::play_beats`).
        let acts = combat && !known.is_empty();
        if acts {
            strikes.presentation.push(Beat::CameraFocus { at: start });
        }
        let mut ap = if combat { AP_PER_ROUND } else { 1 };
        while ap > 0 {
            let here = (pos.x, pos.y);
            let targets: Vec<usize> = known.iter().copied().filter(|&h| hero_hp[h] > 0).collect();
            let attacker = Fighter { entity, name: &name.name, stats, at: here };
            let me = |ap| Me { at: here, archetype, weapon, aim: stats.aim, hp: stats.hp, max_hp: stats.max_hp, allies_near, ap };
            let mut strike = |strikes: &mut Strikes, h: usize| {
                let stats = CombatStats { hp: hero_hp[h], ..party[h].3.clone() };
                let target = Fighter { entity: party[h].0, name: &party[h].2.name, stats: &stats, at: cells[h].1 };
                hero_hp[h] -= strikes.strike(&map, &attacker, &target, false).unwrap_or(0);
            };
            let (path, cost) = if confused {
                strikes.log.entries.push(format!("{} is confused and stumbles around.", name.name));
                strikes.effects.play(EffectKind::Confusion, here.0, here.1);
                let rng = &mut strikes.rng;
                (confused_step(&map, here, free, |n| rng.0.random_range(0..n)).into_iter().collect(), 1)
            } else if targets.is_empty() {
                // Hunting: towards where it last saw a hero; a coward would rather not.
                let Alertness::Alerted { last_seen: Some(goal), .. } = *alert else {
                    break;
                };
                if me(ap).fleeing() {
                    break;
                }
                (steps_towards(&map, here, goal, combat, stats.movement, free), 1)
            } else if !combat {
                if let Some(&h) = targets.iter().find(|&&h| in_reach(&map, &weapon, here, cells[h].1).is_ok()) {
                    strike(&mut strikes, h);
                    break;
                }
                let nearest = targets.iter().map(|&h| cells[h].1).min_by_key(|&at| chebyshev(here, at));
                (nearest.map_or_else(Vec::new, |goal| steps_towards(&map, here, goal, false, stats.movement, free)), 1)
            } else {
                let foes: Vec<Foe> = targets
                    .iter()
                    .map(|&h| Foe {
                        at: cells[h].1,
                        defence: party[h].3.defence,
                        hunkered: party[h].6,
                        watch: watchers.iter().any(|w| w.entity == party[h].0).then(|| strikes.arms.weapon(party[h].0).range),
                    })
                    .collect();
                let range = move_range(&map, here, stats.movement, ap, free);
                match plan(&map, &me(ap), &foes, &range) {
                    Plan::Move(cell) => (range.path(cell), range.ap(cell).unwrap_or(ap)),
                    Plan::Attack(foe) => {
                        strike(&mut strikes, targets[foe]);
                        break;
                    }
                    Plan::Hunker => {
                        strikes.commands.entity(entity).insert(Hunkered);
                        strikes.log.entries.push(format!("{} hunkers down.", name.name));
                        break;
                    }
                    Plan::Overwatch => {
                        strikes.commands.entity(entity).insert(Overwatch);
                        strikes.log.entries.push(format!("{} goes on overwatch.", name.name));
                        break;
                    }
                    Plan::Wait => break,
                }
            };
            if path.is_empty() {
                break;
            }
            let (end, alive) = walk_under_watch(&mut strikes, &map, &attacker, &path, &mut watchers);
            let (from_idx, to_idx) = (map.xy_idx(here.0, here.1), map.xy_idx(end.0, end.1));
            map.blocked_tiles[from_idx] = false;
            map.blocked_tiles[to_idx] = true;
            (pos.x, pos.y) = end;
            viewshed.dirty = true;
            ap = ap.saturating_sub(cost);
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
