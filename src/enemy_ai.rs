//! The tactical rules for monsters, as pure functions over plain data:
//! who is alert (`spread_alert`), how a tile looks to a monster
//! (`score_tile`) and what it does with its action points next (`plan`).
//! `monsters::monster_ai` applies them.
//!
//! Alertness. A monster starts `Idle` and does nothing. It turns `Alerted`
//! when it sees a hero, when it is hurt (`combat::damage_system`), or when a
//! monster within `ALERT_RADIUS` tiles of it is alerted (chained, so a group
//! wakes together; a group is simply whoever stands that close). An alerted
//! monster that sees no hero hunts towards where it last saw one, and gives
//! up (back to `Idle`) after `HUNT_TURNS` of its turns without seeing one.
//!
//! Tactics. An alerted monster in combat scores every tile it can reach
//! (`tactics::move_range`) and its own, by the weights of its `Archetype`,
//! and moves to the best; standing on the best, it attacks if it can, or ends
//! its turn hunkered, on overwatch or waiting (`plan`).

use std::collections::VecDeque;

use bevy::prelude::*;
use serde::Deserialize;

use crate::combat::{aim_at, chebyshev, in_reach};
use crate::components::Weapon;
use crate::cover::{exposure, Exposure, FULL_COVER_PENALTY, HALF_COVER_PENALTY};
use crate::map::Map;
use crate::overwatch::{reactions, Watcher};
use crate::tactics::{MoveRange, DEFAULT_MOVEMENT};
use crate::viewshed::line_of_sight;

/// Monsters this close (Chebyshev) to an alerted one wake up with it.
pub const ALERT_RADIUS: i32 = 5;
/// An alerted monster's turns without seeing a hero before it gives up.
pub const HUNT_TURNS: u8 = 3;
/// A hero this many tiles away or nearer can walk up and strike this turn.
pub const THREAT_RANGE: i32 = DEFAULT_MOVEMENT + 1;
/// How far a fleeing coward wants to be from the nearest hero, in steps.
pub const FLEE_RANGE: i32 = 2 * DEFAULT_MOVEMENT;

/// How a monster fights; the raws `archetype` (`"brute"` when missing).
#[derive(Component, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Archetype {
    /// Closes in and attacks, ignoring cover and overwatch.
    #[default]
    Brute,
    /// Flanks, uses cover, avoids overwatch, repositions if AP allows.
    Skirmisher,
    /// Keeps its weapon's optimal range from cover with a line of sight, and shoots.
    Ranged,
    /// Fights like a skirmisher with an alerted ally near; alone or badly
    /// hurt it runs for distance and hunkers.
    Coward,
}

/// Whether a monster knows the party is about.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, Deserialize)]
pub enum Alertness {
    /// Waits where it is and does nothing.
    #[default]
    Idle,
    /// Hunting the party. `last_seen` is where it last saw a hero (`None`
    /// when it was hurt without seeing who did it); `unseen` counts its turns
    /// since.
    Alerted { last_seen: Option<(i32, i32)>, unseen: u8 },
}

impl Alertness {
    pub fn alerted(self) -> bool {
        matches!(self, Alertness::Alerted { .. })
    }
}

/// One monster at the start of its side's turn, as `spread_alert` sees it.
#[derive(Clone, Copy, Debug)]
pub struct Lookout {
    pub at: (i32, i32),
    pub alert: Alertness,
    /// The hero it sees, if any.
    pub sees: Option<(i32, i32)>,
}

/// Every monster's alertness now, in `lookouts` order. A monster that sees a
/// hero is alerted with that sighting; on a `new_turn` of its own, an
/// alerted one that does not counts one more turn unseen and gives up past
/// `HUNT_TURNS`. Then each idle monster within `ALERT_RADIUS` of an alerted
/// one takes on its alertness, spreading in `lookouts` order. A hurt monster
/// that saw nobody takes the nearest of `party` as last seen.
pub fn spread_alert(lookouts: &[Lookout], party: &[(i32, i32)], new_turn: bool) -> Vec<Alertness> {
    let nearest = |from: (i32, i32)| party.iter().copied().min_by_key(|&hero| chebyshev(from, hero));
    let mut alerts: Vec<Alertness> = lookouts
        .iter()
        .map(|l| match (l.sees, l.alert) {
            (Some(hero), _) => Alertness::Alerted { last_seen: Some(hero), unseen: 0 },
            (None, Alertness::Idle) => Alertness::Idle,
            (None, Alertness::Alerted { unseen, .. }) if new_turn && unseen >= HUNT_TURNS => Alertness::Idle,
            (None, Alertness::Alerted { last_seen, unseen }) => Alertness::Alerted {
                last_seen: last_seen.or_else(|| nearest(l.at)),
                unseen: unseen + u8::from(new_turn),
            },
        })
        .collect();
    let mut waking: VecDeque<usize> = (0..alerts.len()).filter(|&i| alerts[i].alerted()).collect();
    while let Some(i) = waking.pop_front() {
        for j in 0..alerts.len() {
            if alerts[j] == Alertness::Idle && chebyshev(lookouts[i].at, lookouts[j].at) <= ALERT_RADIUS {
                alerts[j] = alerts[i];
                waking.push_back(j);
            }
        }
    }
    alerts
}

/// The monster choosing, as the scoring sees it.
#[derive(Clone, Copy, Debug)]
pub struct Me {
    pub at: (i32, i32),
    pub archetype: Archetype,
    pub weapon: Weapon,
    pub aim: i32,
    pub hp: i32,
    pub max_hp: i32,
    /// Other alerted monsters within `ALERT_RADIUS`.
    pub allies_near: usize,
    /// Action points left this turn.
    pub ap: u8,
}

impl Me {
    pub fn wounded(&self) -> bool {
        self.hp * 3 <= self.max_hp
    }

    /// A coward alone or badly hurt runs instead of fighting (and does not hunt).
    pub fn fleeing(&self) -> bool {
        self.archetype == Archetype::Coward && (self.allies_near == 0 || self.wounded())
    }
}

/// A hero the monster knows about.
#[derive(Clone, Copy, Debug)]
pub struct Foe {
    pub at: (i32, i32),
    pub defence: i32,
    pub hunkered: bool,
    /// Its weapon's range while it is on overwatch.
    pub watch: Option<i32>,
}

/// How much each term of `score_tile` counts. `preferred` is the distance in
/// steps the monster wants from the nearest hero.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Weights {
    /// Per point of aim penalty its cover puts on the heroes' shots.
    pub cover: i32,
    /// Once if it can flank a hero from the tile.
    pub flank: i32,
    /// Per step away from `preferred` (so negative).
    pub distance: i32,
    /// Per percent of its best chance to hit from the tile.
    pub hit: i32,
    /// Per overwatch shot the walk there draws (negative).
    pub overwatch: i32,
    /// Per hero within `THREAT_RANGE` of the tile (negative).
    pub reach: i32,
    pub preferred: i32,
}

/// The weights `me` scores tiles with: its archetype's, a coward's by its nerve.
pub fn weights(me: &Me) -> Weights {
    let skirmisher = Weights { cover: 1, flank: 30, distance: -5, hit: 1, overwatch: -40, reach: 0, preferred: 1 };
    match me.archetype {
        Archetype::Brute => Weights { cover: 0, flank: 0, distance: -20, hit: 1, overwatch: 0, reach: 0, preferred: 1 },
        Archetype::Skirmisher => skirmisher,
        Archetype::Ranged => Weights {
            cover: 1,
            flank: 10,
            distance: -5,
            hit: 1,
            overwatch: -40,
            reach: -15,
            preferred: me.weapon.optimal_range,
        },
        Archetype::Coward if me.fleeing() => Weights {
            cover: 1,
            flank: 0,
            distance: -10,
            hit: 0,
            overwatch: -40,
            reach: -30,
            preferred: FLEE_RANGE,
        },
        Archetype::Coward => Weights { flank: 10, ..skirmisher },
    }
}

/// Steps (diagonals count one) from every cell to the nearest of `foes`,
/// over walkable ground and through units; `i32::MAX` where none is reachable.
pub fn steps_to(map: &Map, foes: &[Foe]) -> Vec<i32> {
    let mut steps = vec![i32::MAX; map.tiles.len()];
    let mut frontier = VecDeque::new();
    for foe in foes {
        let idx = map.xy_idx(foe.at.0, foe.at.1);
        steps[idx] = 0;
        frontier.push_back(idx);
    }
    while let Some(idx) = frontier.pop_front() {
        for (next, _) in map.get_available_exits_ignoring_entities(idx) {
            if steps[next] == i32::MAX {
                steps[next] = steps[idx] + 1;
                frontier.push_back(next);
            }
        }
    }
    steps
}

/// The best shot from `from`: the foe and the chance, the first of a tie.
pub fn best_shot(map: &Map, me: &Me, from: (i32, i32), foes: &[Foe]) -> Option<(usize, i32)> {
    let mut best: Option<(usize, i32)> = None;
    for (i, foe) in foes.iter().enumerate() {
        if let Ok((chance, _)) = aim_at(map, &me.weapon, me.aim, from, foe.defence, foe.at, foe.hunkered) {
            if best.is_none_or(|(_, b)| chance > b) {
                best = Some((i, chance));
            }
        }
    }
    best
}

/// The aim penalty `cell` puts on the worst shot at it from the foes that
/// see it: half or full cover, 0 open, minus half cover's worth flanked.
/// `None` when no foe sees it (`score_tile` counts that as full cover).
fn cover_value(map: &Map, cell: (i32, i32), foes: &[Foe]) -> Option<i32> {
    foes.iter()
        .filter(|foe| line_of_sight(map, foe.at, cell))
        .map(|foe| match exposure(map, cell, foe.at) {
            Exposure::Flanked => -HALF_COVER_PENALTY,
            exposure => exposure.aim_penalty(),
        })
        .min()
}

/// How good `cell`, reached on `path` for `cost` AP, is for `me`. Pure; the
/// terms are named in `Weights`. `steps` is `steps_to(map, foes)`.
pub fn score_tile(
    map: &Map,
    me: &Me,
    foes: &[Foe],
    steps: &[i32],
    cell: (i32, i32),
    path: &[(i32, i32)],
    cost: u8,
) -> i32 {
    let w = weights(me);
    let cover = cover_value(map, cell, foes).unwrap_or(FULL_COVER_PENALTY);
    let flanks =
        foes.iter().any(|foe| exposure(map, foe.at, cell) == Exposure::Flanked && in_reach(map, &me.weapon, cell, foe.at).is_ok());
    let nearest = match steps[map.xy_idx(cell.0, cell.1)] {
        i32::MAX => foes.iter().map(|foe| chebyshev(cell, foe.at)).min().unwrap_or(0),
        n => n,
    };
    // A shot counts only with AP left to take it after the move.
    let hit = if cost < me.ap { best_shot(map, me, cell, foes).map_or(0, |(_, chance)| chance) } else { 0 };
    let watchers: Vec<_> =
        foes.iter().filter_map(|foe| foe.watch.map(|range| Watcher { cell: foe.at, range, party: true })).collect();
    let shots = reactions(&watchers, false, path, |from, to| line_of_sight(map, from, to)).len() as i32;
    let threats = foes.iter().filter(|foe| chebyshev(cell, foe.at) <= THREAT_RANGE).count() as i32;
    w.cover * cover
        + w.flank * i32::from(flanks)
        + w.distance * (nearest - w.preferred).abs()
        + w.hit * hit
        + w.overwatch * shots
        + w.reach * threats
}

/// What a monster does next with its AP.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Plan {
    Move((i32, i32)),
    /// Attack the foe at this index.
    Attack(usize),
    Hunker,
    Overwatch,
    Wait,
}

/// The next thing `me` does: move to the best-scoring tile of `range` (its
/// own tile counts, costing nothing), or, already on it, attack the foe it
/// hits most surely; with no shot it hunkers if it is in cover and fleeing or
/// wounded (brutes never hunker), goes on overwatch if it sees a foe, else
/// waits. A tie goes to its own tile, then the cheaper tile, then the lower
/// x, then the lower y.
pub fn plan(map: &Map, me: &Me, foes: &[Foe], range: &MoveRange) -> Plan {
    let steps = steps_to(map, foes);
    let mut cells: Vec<_> = range.cells().map(|(cell, cost)| (cost, cell)).collect();
    cells.sort_unstable();
    let mut best = (me.at, score_tile(map, me, foes, &steps, me.at, &[], 0));
    for (cost, cell) in cells {
        let score = score_tile(map, me, foes, &steps, cell, &range.path(cell), cost);
        if score > best.1 {
            best = (cell, score);
        }
    }
    if best.0 != me.at {
        return Plan::Move(best.0);
    }
    if !me.fleeing() {
        if let Some((foe, _)) = best_shot(map, me, me.at, foes) {
            return Plan::Attack(foe);
        }
    }
    let covered = cover_value(map, me.at, foes).is_some_and(|cover| cover > 0);
    if covered && (me.fleeing() || (me.wounded() && me.archetype != Archetype::Brute)) {
        Plan::Hunker
    } else if !me.fleeing() && foes.iter().any(|foe| line_of_sight(map, me.at, foe.at)) {
        Plan::Overwatch
    } else {
        Plan::Wait
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::DEFAULT_AIM;
    use crate::cover::Cover;
    use crate::map::TileType;
    use crate::tactics::move_range;

    const BOW: Weapon = Weapon { damage: (1, 4), range: 8, optimal_range: 5, crit_chance: 10 };

    /// A `w` x `h` room: walls round the edge, floor inside.
    fn room(w: i32, h: i32) -> Map {
        let mut map = Map::new(w as usize, h as usize, 2);
        for y in 1..h - 1 {
            for x in 1..w - 1 {
                let idx = map.xy_idx(x, y);
                map.tiles[idx] = TileType::Floor;
            }
        }
        map.populate_blocked();
        map
    }

    fn prop(map: &mut Map, cell: (i32, i32), cover: Cover) {
        let idx = map.xy_idx(cell.0, cell.1);
        map.prop_cover.insert(idx, cover);
    }

    fn wall(map: &mut Map, cell: (i32, i32)) {
        let idx = map.xy_idx(cell.0, cell.1);
        map.tiles[idx] = TileType::Wall;
        map.populate_blocked();
    }

    fn me(at: (i32, i32), archetype: Archetype, weapon: Weapon) -> Me {
        Me { at, archetype, weapon, aim: DEFAULT_AIM, hp: 10, max_hp: 10, allies_near: 1, ap: 2 }
    }

    fn foe(at: (i32, i32)) -> Foe {
        Foe { at, defence: 0, hunkered: false, watch: None }
    }

    fn plan_for(map: &Map, me: &Me, foes: &[Foe]) -> Plan {
        let range = move_range(map, me.at, DEFAULT_MOVEMENT, me.ap, |cell| foes.iter().all(|f| f.at != cell));
        plan(map, me, foes, &range)
    }

    fn score(map: &Map, me: &Me, foes: &[Foe], cell: (i32, i32), cost: u8) -> i32 {
        score_tile(map, me, foes, &steps_to(map, foes), cell, &[], cost)
    }

    #[test]
    fn a_ranged_monster_takes_cover_with_a_line_of_sight_then_shoots() {
        let mut map = room(14, 10);
        // Half cover west of (7, 6), facing the hero at (2, 4).
        prop(&mut map, (6, 6), Cover::Half);
        let archer = me((8, 4), Archetype::Ranged, BOW);
        let hero = [foe((2, 4))];
        assert!(score(&map, &archer, &hero, (7, 6), 1) > score(&map, &archer, &hero, (7, 4), 1), "covered beats open");
        assert_eq!(plan_for(&map, &archer, &hero), Plan::Move((7, 6)));
        // There, with one AP left, it shoots rather than move again.
        assert_eq!(plan_for(&map, &Me { at: (7, 6), ap: 1, ..archer }, &hero), Plan::Attack(0));
    }

    #[test]
    fn a_brute_goes_beside_the_hero_and_attacks() {
        let map = room(14, 10);
        let brute = me((8, 4), Archetype::Brute, Weapon::NATURAL);
        let hero = [foe((3, 4))];
        let Plan::Move(cell) = plan_for(&map, &brute, &hero) else {
            panic!("the brute should move");
        };
        assert_eq!(chebyshev(cell, (3, 4)), 1);
        assert_eq!(plan_for(&map, &Me { at: cell, ap: 1, ..brute }, &hero), Plan::Attack(0));
    }

    #[test]
    fn a_brute_ignores_overwatch_a_skirmisher_does_not() {
        let map = room(14, 10);
        let (watching, idle) = ([Foe { watch: Some(1), ..foe((3, 4)) }], [foe((3, 4))]);
        let steps = steps_to(&map, &idle);
        let path = [(7, 4), (6, 4), (5, 4), (4, 4)];
        let walk = |me: &Me, foes: &[Foe]| score_tile(&map, me, foes, &steps, (4, 4), &path, 1);
        let brute = me((8, 4), Archetype::Brute, Weapon::NATURAL);
        assert_eq!(walk(&brute, &watching), walk(&brute, &idle));
        let skirmisher = Me { archetype: Archetype::Skirmisher, ..brute };
        assert_eq!(walk(&skirmisher, &watching), walk(&skirmisher, &idle) - 40, "one reaction shot on the way");
    }

    #[test]
    fn a_skirmisher_flanks_a_hero_in_cover() {
        let mut map = room(14, 10);
        // The hero at (4, 4) has a wall on its east side, towards the skirmisher.
        wall(&mut map, (5, 4));
        let skirmisher = me((7, 2), Archetype::Skirmisher, Weapon::NATURAL);
        let hero = [foe((4, 4))];
        let Plan::Move(cell) = plan_for(&map, &skirmisher, &hero) else {
            panic!("the skirmisher should move");
        };
        assert_eq!(chebyshev(cell, (4, 4)), 1);
        assert_eq!(exposure(&map, (4, 4), cell), Exposure::Flanked, "from {cell:?}");
    }

    #[test]
    fn a_lone_coward_runs_for_distance_and_one_with_friends_fights() {
        let map = room(20, 10);
        let hero = [foe((3, 4))];
        let alone = Me { allies_near: 0, ..me((6, 4), Archetype::Coward, Weapon::NATURAL) };
        let Plan::Move(cell) = plan_for(&map, &alone, &hero) else {
            panic!("the coward should run");
        };
        assert!(chebyshev(cell, (3, 4)) > 3 + DEFAULT_MOVEMENT / 2, "ran to {cell:?}");
        // With an ally near it closes in like a skirmisher.
        let Plan::Move(cell) = plan_for(&map, &Me { allies_near: 1, ..alone }, &hero) else {
            panic!("the coward should close in");
        };
        assert_eq!(chebyshev(cell, (3, 4)), 1);
        // Badly hurt, it runs even with friends.
        let hurt = Me { hp: 3, allies_near: 1, ..alone };
        assert!(matches!(plan_for(&map, &hurt, &hero), Plan::Move(cell) if chebyshev(cell, (3, 4)) > 3));
    }

    #[test]
    fn a_cornered_coward_in_cover_hunkers() {
        let mut map = room(5, 10);
        prop(&mut map, (2, 7), Cover::Full);
        let hero = [foe((2, 2))];
        // Bottom of a corridor, nowhere further to run, full cover north.
        let coward = Me { allies_near: 0, ap: 1, ..me((2, 8), Archetype::Coward, Weapon::NATURAL) };
        assert_eq!(plan(&map, &coward, &hero, &MoveRange::default()), Plan::Hunker);
    }

    #[test]
    fn with_no_shot_and_nowhere_better_it_goes_on_overwatch() {
        let map = room(14, 10);
        let brute = me((8, 4), Archetype::Brute, Weapon::NATURAL);
        assert_eq!(plan(&map, &brute, &[foe((3, 4))], &MoveRange::default()), Plan::Overwatch);
    }

    #[test]
    fn ties_go_to_the_cheapest_tile_then_the_lowest_x_and_y() {
        let map = room(12, 10);
        // Straight south of the brute: three tiles beside the hero cost 1 AP each.
        let brute = me((5, 2), Archetype::Brute, Weapon::NATURAL);
        let hero = [foe((5, 6))];
        assert_eq!(plan_for(&map, &brute, &hero), Plan::Move((4, 5)));
        assert_eq!(plan_for(&map, &brute, &hero), plan_for(&map, &brute, &hero));
    }

    fn lookout(at: (i32, i32), alert: Alertness, sees: Option<(i32, i32)>) -> Lookout {
        Lookout { at, alert, sees }
    }

    #[test]
    fn a_sighting_wakes_the_whole_group_and_no_further() {
        let idle = Alertness::Idle;
        let lookouts = [
            lookout((10, 10), idle, None),
            lookout((12, 10), idle, Some((15, 10))),
            lookout((30, 10), idle, None),
            // Out of the spotter's radius, but within the first one's.
            lookout((6, 10), idle, None),
        ];
        let alerted = Alertness::Alerted { last_seen: Some((15, 10)), unseen: 0 };
        assert_eq!(spread_alert(&lookouts, &[(15, 10)], false), vec![alerted, alerted, idle, alerted]);
        let nobody = [lookout((10, 10), idle, None), lookout((12, 10), idle, None)];
        assert_eq!(spread_alert(&nobody, &[(15, 10)], true), vec![idle, idle]);
    }

    #[test]
    fn an_alerted_monster_hunts_unseen_for_a_while_then_gives_up() {
        let mut alert = Alertness::Alerted { last_seen: Some((4, 4)), unseen: 0 };
        for unseen in 1..=HUNT_TURNS {
            alert = spread_alert(&[lookout((9, 9), alert, None)], &[(1, 1)], true)[0];
            assert_eq!(alert, Alertness::Alerted { last_seen: Some((4, 4)), unseen });
        }
        // Between its turns the count stands still.
        assert_eq!(spread_alert(&[lookout((9, 9), alert, None)], &[(1, 1)], false)[0], alert);
        assert_eq!(spread_alert(&[lookout((9, 9), alert, None)], &[(1, 1)], true)[0], Alertness::Idle);
        // Seeing a hero again starts the count afresh.
        let seen = spread_alert(&[lookout((9, 9), alert, Some((2, 2)))], &[(2, 2)], true)[0];
        assert_eq!(seen, Alertness::Alerted { last_seen: Some((2, 2)), unseen: 0 });
    }

    #[test]
    fn a_monster_hurt_unseen_goes_after_the_nearest_hero() {
        let hurt = Alertness::Alerted { last_seen: None, unseen: 0 };
        let after = spread_alert(&[lookout((9, 9), hurt, None)], &[(1, 1), (7, 9)], true)[0];
        assert_eq!(after, Alertness::Alerted { last_seen: Some((7, 9)), unseen: 1 });
    }
}
