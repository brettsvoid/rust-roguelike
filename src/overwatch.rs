//! Overwatch and hunker down. A unit on `Overwatch` fires one reaction shot
//! at the first hostile that moves into its reach before its next turn; a
//! `Hunkered` unit's cover counts double (`cover::Exposure::Hunkered`). Both
//! end at the start of the unit's turn (`combat::start_of_turn`) and are not
//! saved. Which watcher fires where is `reactions`, a pure rule; moves in
//! combat go through `walk_under_watch` so it applies to heroes and monsters.

use bevy::prelude::*;

use crate::combat::{chebyshev, CombatStats, Fighter, Strikes};
use crate::components::Name;
use crate::map::{Map, Position};
use crate::viewshed::line_of_sight;

/// On watch: one reaction shot at the first hostile that moves into reach.
#[derive(Component, Debug)]
pub struct Overwatch;

/// Hunkered down: cover counts double and crits are off until its next turn.
#[derive(Component, Debug)]
pub struct Hunkered;

/// Aim taken off a reaction shot (XCOM's overwatch shots are about 0.7 times aim).
pub const OVERWATCH_AIM_PENALTY: i32 = 20;

/// A unit on overwatch, as the trigger rule sees it.
#[derive(Clone, Copy, Debug)]
pub struct Watcher {
    pub cell: (i32, i32),
    /// Its weapon's range; 1 for melee.
    pub range: i32,
    /// Its side: a hero, or a monster.
    pub party: bool,
}

/// Who reacts to a unit of side `mover_party` walking `path` (the cells it
/// steps onto, its start left out). Each watcher of the other side fires
/// once, at the first cell within its range that `sight` sees from it (the
/// `combat::in_reach` rule). Returns `(path index, watcher index)` in path
/// order; watchers firing on the same cell keep their order in `watchers`.
pub fn reactions(
    watchers: &[Watcher],
    mover_party: bool,
    path: &[(i32, i32)],
    sight: impl Fn((i32, i32), (i32, i32)) -> bool,
) -> Vec<(usize, usize)> {
    let mut fired: Vec<_> = watchers
        .iter()
        .enumerate()
        .filter(|(_, w)| w.party != mover_party)
        .filter_map(|(i, w)| {
            let step = path.iter().position(|&cell| chebyshev(w.cell, cell) <= w.range && sight(w.cell, cell))?;
            Some((step, i))
        })
        .collect();
    fired.sort_by_key(|&(step, _)| step);
    fired
}

/// Moves `mover` along `path` under the fire of `watchers` (units on
/// overwatch, either side): each reaction resolves through
/// `Strikes::strike` on the cell where it triggers, spends that watcher's
/// overwatch (taken out of `watchers` too) and a mover it kills stops there.
/// Returns the cell the mover ends on and whether it is still standing.
pub fn walk_under_watch(
    strikes: &mut Strikes,
    map: &Map,
    mover: &Fighter,
    path: &[(i32, i32)],
    watchers: &mut Vec<Fighter>,
) -> ((i32, i32), bool) {
    let plain: Vec<_> = watchers
        .iter()
        .map(|w| Watcher { cell: w.at, range: strikes.arms.weapon(w.entity).range, party: strikes.is_party(w.entity) })
        .collect();
    let fired = reactions(&plain, strikes.is_party(mover.entity), path, |from, to| line_of_sight(map, from, to));
    let mut end = path.last().copied().unwrap_or(mover.at);
    let mut hp = mover.stats.hp;
    let mut spent = Vec::new();
    for (step, watcher) in fired {
        let target = Fighter { at: path[step], ..*mover };
        hp -= strikes.strike(map, &watchers[watcher], &target, true).unwrap_or(0);
        strikes.commands.entity(watchers[watcher].entity).remove::<Overwatch>();
        spent.push(watcher);
        if hp <= 0 {
            end = target.at;
            break;
        }
    }
    spent.sort_unstable();
    for watcher in spent.into_iter().rev() {
        watchers.remove(watcher);
    }
    (end, hp > 0)
}

/// A hero's move in combat along its `tactics::MoveRange` path, walked by
/// `move_system` so overwatch can interrupt it.
#[derive(Component, Debug)]
pub struct WantsToMove {
    pub path: Vec<(i32, i32)>,
}

/// Walks every `WantsToMove` under the watchers' fire.
pub fn move_system(
    mut strikes: Strikes,
    map: Res<Map>,
    mut movers: Query<(Entity, &WantsToMove, &Name, &CombatStats, &mut Position)>,
    on_watch: Query<(Entity, &Name, &CombatStats, &Position), (With<Overwatch>, Without<WantsToMove>)>,
) {
    let mut watchers: Vec<_> = on_watch
        .iter()
        .map(|(entity, name, stats, pos)| Fighter { entity, name: &name.name, stats, at: (pos.x, pos.y) })
        .collect();
    for (entity, wants, name, stats, mut pos) in &mut movers {
        strikes.commands.entity(entity).remove::<WantsToMove>();
        let mover = Fighter { entity, name: &name.name, stats, at: (pos.x, pos.y) };
        let (end, _) = walk_under_watch(&mut strikes, &map, &mover, &wants.path, &mut watchers);
        (pos.x, pos.y) = end;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HERO: bool = true;
    const MONSTER: bool = false;

    fn watcher(cell: (i32, i32), range: i32, party: bool) -> Watcher {
        Watcher { cell, range, party }
    }

    /// A wall along x = 5 blocks every line that crosses it.
    fn wall_at_five(from: (i32, i32), to: (i32, i32)) -> bool {
        (from.0 < 5) == (to.0 < 5) && from.0 != 5 && to.0 != 5
    }

    fn clear(_: (i32, i32), _: (i32, i32)) -> bool {
        true
    }

    /// A goblin walking east along y = 0 from (0, 0).
    fn walk_east(to: i32) -> Vec<(i32, i32)> {
        (1..=to).map(|x| (x, 0)).collect()
    }

    #[test]
    fn nothing_fires_out_of_range_or_out_of_sight() {
        let bow = [watcher((10, 0), 3, HERO)];
        assert_eq!(reactions(&bow, MONSTER, &walk_east(6), clear), vec![], "never within 3 tiles");
        let behind_wall = [watcher((7, 0), 8, HERO)];
        assert_eq!(reactions(&behind_wall, MONSTER, &walk_east(4), wall_at_five), vec![]);
    }

    #[test]
    fn a_watcher_fires_on_the_first_cell_in_reach_and_only_once() {
        let bow = [watcher((10, 0), 3, HERO)];
        // (7, 0) is the first cell within 3 tiles; the walk goes on to (9, 0).
        assert_eq!(reactions(&bow, MONSTER, &walk_east(9), clear), vec![(6, 0)]);
        // In range all along, but the wall hides the walk until (6, 0).
        let past_wall = [watcher((8, 0), 8, HERO)];
        assert_eq!(reactions(&past_wall, MONSTER, &walk_east(9), wall_at_five), vec![(5, 0)]);
    }

    #[test]
    fn a_melee_watcher_reacts_to_a_step_beside_it() {
        let sword = [watcher((4, 1), 1, HERO)];
        assert_eq!(reactions(&sword, MONSTER, &walk_east(6), clear), vec![(2, 0)], "diagonally beside it at (3, 0)");
    }

    #[test]
    fn several_watchers_each_fire_in_path_order() {
        let watchers = [watcher((9, 0), 1, HERO), watcher((0, 3), 4, HERO), watcher((5, 2), 2, HERO)];
        let fired = reactions(&watchers, MONSTER, &walk_east(8), clear);
        // (1, 0) is in reach of the second, (3, 0) of the third, (8, 0) of the first.
        assert_eq!(fired, vec![(0, 1), (2, 2), (7, 0)]);
        // Two in reach of the same cell fire there in the order given.
        let pair = [watcher((3, 2), 2, HERO), watcher((1, 1), 1, HERO)];
        assert_eq!(reactions(&pair, MONSTER, &walk_east(5), clear), vec![(0, 0), (0, 1)]);
    }

    #[test]
    fn friendly_movers_do_not_trigger() {
        let watchers = [watcher((2, 1), 4, HERO), watcher((3, 1), 4, MONSTER)];
        assert_eq!(reactions(&watchers, HERO, &walk_east(4), clear), vec![(0, 1)], "only the monster fires on a hero");
        assert_eq!(reactions(&watchers, MONSTER, &walk_east(4), clear), vec![(0, 0)]);
    }

    #[test]
    fn a_mover_in_reach_that_does_not_move_does_not_trigger() {
        let bow = [watcher((2, 0), 8, HERO)];
        assert_eq!(reactions(&bow, MONSTER, &[], clear), vec![]);
    }
}
