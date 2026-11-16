use bevy::ecs::query::QueryFilter;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use rand::RngExt;

use crate::cover::{exposure, Exposure};
use crate::components::{
    Confusion, DefenseBonus, Equipped, HungerClock, HungerState, InBackpack, Item, Name, Poison, PoisonsOnHit,
    Weapon,
};
use crate::debug::GodMode;
use crate::gamelog::GameLog;
use crate::economy::Gold;
use crate::map::{Map, Position};
use crate::overwatch::{Hunkered, Overwatch, OVERWATCH_AIM_PENALTY};
use crate::effects::Effects;
use crate::presentation::{self, Beat, BodyParts, Presentation};
use crate::party::{next_unit, ActiveUnit, PartyMember};
use crate::raws::{spawn_named_item, RAWS};
use crate::rng::GameRng;
use crate::saveload;
use crate::viewshed::line_of_sight;
use crate::RunState;

#[derive(Component, Debug, Clone)]
pub struct CombatStats {
    pub max_hp: i32,
    pub hp: i32,
    /// Taken off the damage of every hit this unit suffers.
    pub armour: i32,
    pub power: i32,
    /// Tiles moved per action point in combat.
    pub movement: i32,
    /// Base chance to hit, in percent.
    pub aim: i32,
    /// Taken off attackers' chance to hit.
    pub defence: i32,
}

/// Every unit's aim unless its raws say otherwise (XCOM's rookie aim).
pub const DEFAULT_AIM: i32 = 65;
/// No hit chance goes below this or above `MAX_HIT`, so nothing is certain.
pub const MIN_HIT: i32 = 5;
pub const MAX_HIT: i32 = 95;
/// Aim lost per tile beyond the weapon's optimal range.
pub const RANGE_PENALTY: i32 = 10;
/// Aim gained per level the attacker stands above the target.
pub const HEIGHT_BONUS: i32 = 10;
/// The height input while the map is flat: attacker and target level.
pub const LEVEL_GROUND: i32 = 0;
/// Magic missiles home in: added to the caster's aim.
pub const SPELL_AIM_BONUS: i32 = 25;

/// The chance in percent that an attack hits. `cover` is the aim it takes
/// away (`cover::Exposure::aim_penalty`), `height` the levels the attacker stands above the target. A
/// target beyond the weapon's range cannot be hit at all.
pub fn hit_chance(aim: i32, defence: i32, distance: i32, weapon: &Weapon, cover: i32, height: i32) -> i32 {
    if distance > weapon.range {
        return 0;
    }
    let falloff = RANGE_PENALTY * (distance - weapon.optimal_range).max(0);
    (aim - defence - falloff - cover + HEIGHT_BONUS * height).clamp(MIN_HIT, MAX_HIT)
}

/// Grid distance for attacks: diagonal steps count as one, as in movement.
pub fn chebyshev(from: (i32, i32), to: (i32, i32)) -> i32 {
    (to.0 - from.0).abs().max((to.1 - from.1).abs())
}

/// Why an attack cannot be made.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    OutOfRange,
    NoLineOfSight,
}

impl Refusal {
    pub fn message(self, target: &str) -> String {
        match self {
            Refusal::OutOfRange => format!("{target} is out of range."),
            Refusal::NoLineOfSight => format!("No line of sight to {target}."),
        }
    }
}

/// The distance to `to` if `weapon` can reach it from `from`: in range, and
/// a clear line of sight.
pub fn in_reach(map: &Map, weapon: &Weapon, from: (i32, i32), to: (i32, i32)) -> Result<i32, Refusal> {
    let distance = chebyshev(from, to);
    if distance > weapon.range {
        Err(Refusal::OutOfRange)
    } else if !line_of_sight(map, from, to) {
        Err(Refusal::NoLineOfSight)
    } else {
        Ok(distance)
    }
}

/// The chance to hit a target at `to` with `defence` and how its cover
/// stands against `from` (doubled if it is `hunkered`), or why it cannot be
/// attacked. Height is neutral until a map with height.
pub fn aim_at(
    map: &Map,
    weapon: &Weapon,
    aim: i32,
    from: (i32, i32),
    defence: i32,
    to: (i32, i32),
    hunkered: bool,
) -> Result<(i32, Exposure), Refusal> {
    let distance = in_reach(map, weapon, from, to)?;
    let cover = exposure(map, to, from).hunker(hunkered);
    Ok((hit_chance(aim, defence, distance, weapon, cover.aim_penalty(), LEVEL_GROUND), cover))
}

/// The odds for the log: "45%", or "45%, half cover" when cover or a flank applied.
pub fn odds(chance: i32, cover: Exposure) -> String {
    match cover.note() {
        Some(note) => format!("{chance}%, {note}"),
        None => format!("{chance}%"),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Miss,
    Hit,
    Crit,
}

/// Rolls to hit at `chance` percent; a hit then rolls `crit_chance` for a critical.
pub fn roll_attack(rng: &mut GameRng, chance: i32, crit_chance: i32) -> Outcome {
    if rng.0.random_range(0..100) >= chance {
        Outcome::Miss
    } else if rng.0.random_range(0..100) < crit_chance {
        Outcome::Crit
    } else {
        Outcome::Hit
    }
}

/// A hit's damage: a roll in `damage` plus `bonus`, half as much again
/// (rounded up) on a critical, less `armour`, never below 0.
pub fn roll_damage(rng: &mut GameRng, damage: (i32, i32), bonus: i32, armour: i32, crit: bool) -> i32 {
    let mut amount = rng.0.random_range(damage.0..=damage.1) + bonus;
    if crit {
        amount += (amount.max(0) + 1) / 2;
    }
    (amount - armour).max(0)
}

/// A spell's flat damage as a range a quarter either side.
pub fn spell_damage(damage: i32) -> (i32, i32) {
    (damage - damage / 4, damage + damage / 4)
}

/// What units fight with: the weapon they hold and the armour they wear.
#[derive(SystemParam)]
pub struct Arms<'w, 's> {
    wielded: Query<'w, 's, (&'static Equipped, &'static Weapon)>,
    worn: Query<'w, 's, (&'static Equipped, &'static DefenseBonus)>,
}

impl Arms<'_, '_> {
    /// The unit's equipped weapon, or its natural attack.
    pub fn weapon(&self, unit: Entity) -> Weapon {
        self.wielded.iter().find(|(e, _)| e.owner == unit).map_or(Weapon::NATURAL, |(_, w)| *w)
    }

    /// Armour from the unit's equipment, on top of its own.
    pub fn armour(&self, unit: Entity) -> i32 {
        self.worn.iter().filter(|(e, _)| e.owner == unit).map(|(_, d)| d.defense).sum()
    }
}

/// An attack on `target` with whatever the unit holds, melee or ranged.
#[derive(Component, Debug)]
pub struct WantsToAttack {
    pub target: Entity,
}

#[derive(Component, Debug, Default)]
pub struct SufferDamage {
    pub amount: Vec<i32>,
}

impl SufferDamage {
    pub fn new_damage(commands: &mut Commands, victim: Entity, amount: i32) {
        commands.entity(victim).entry::<SufferDamage>().or_default().and_modify(move |mut d| {
            d.amount.push(amount);
        });
    }
}

/// One side of an attack: who, and where they stand at that moment.
#[derive(Clone, Copy)]
pub struct Fighter<'a> {
    pub entity: Entity,
    pub name: &'a str,
    pub stats: &'a CombatStats,
    pub at: (i32, i32),
}

/// Everything an attack touches, so normal attacks and overwatch reactions
/// resolve the same way (`Strikes::strike`).
#[derive(SystemParam)]
pub struct Strikes<'w, 's> {
    pub commands: Commands<'w, 's>,
    pub log: ResMut<'w, GameLog>,
    pub effects: ResMut<'w, Effects>,
    pub rng: ResMut<'w, GameRng>,
    pub presentation: ResMut<'w, Presentation>,
    pub arms: Arms<'w, 's>,
    god_mode: Res<'w, GodMode>,
    party: Query<'w, 's, (), With<PartyMember>>,
    hunkered: Query<'w, 's, (), With<Hunkered>>,
    hunger: Query<'w, 's, &'static HungerClock>,
    poisoners: Query<'w, 's, &'static PoisonsOnHit>,
}

impl Strikes<'_, '_> {
    /// Whether `unit` is a hero.
    pub fn is_party(&self, unit: Entity) -> bool {
        self.party.contains(unit)
    }

    /// One attack from `attacker` on `target` with whatever the attacker
    /// holds: rolls to hit (less the target's cover, doubled if hunkered), for
    /// a critical (more on a flank) and for damage, and logs the outcome with
    /// the chance and any cover. A `reaction` is an overwatch shot, at
    /// `OVERWATCH_AIM_PENALTY` less aim. Returns the damage the target will
    /// take (queued as `SufferDamage`; 0 for a hero under god mode), or `None`
    /// when no attack was made.
    pub fn strike(&mut self, map: &Map, attacker: &Fighter, target: &Fighter, reaction: bool) -> Option<i32> {
        if attacker.stats.hp <= 0 || target.stats.hp <= 0 {
            return None;
        }
        let (from, to) = (attacker.at, target.at);
        let weapon = self.arms.weapon(attacker.entity);
        let aim = attacker.stats.aim - if reaction { OVERWATCH_AIM_PENALTY } else { 0 };
        let hunkered = self.hunkered.contains(target.entity);
        let (chance, cover) = match aim_at(map, &weapon, aim, from, target.stats.defence, to, hunkered) {
            Ok(aimed) => aimed,
            Err(refusal) => {
                self.log.entries.push(refusal.message(target.name));
                return None;
            }
        };
        let ranged = weapon.range > 1;
        self.presentation.push(if ranged {
            Beat::Projectile { from, to }
        } else {
            Beat::Lunge { unit: attacker.entity, from, to }
        });
        let verb = if ranged { "shoots" } else { "attacks" };
        let prefix = if reaction { "Overwatch! " } else { "" };
        let (who, whom) = (attacker.name, target.name);
        let outcome = roll_attack(&mut self.rng, chance, cover.crit_chance(weapon.crit_chance));
        let odds = odds(chance, cover);
        if outcome == Outcome::Miss {
            self.log.entries.push(format!("{prefix}{who} {verb} {whom}: miss ({odds})"));
            self.presentation.push(Beat::Floater { at: to, outcome, damage: 0 });
            return Some(0);
        }
        let well_fed = self.hunger.get(attacker.entity).is_ok_and(|h| h.state == HungerState::WellFed);
        let bonus = attacker.stats.power + i32::from(well_fed);
        let armour = target.stats.armour + self.arms.armour(target.entity);
        let crit = outcome == Outcome::Crit;
        let damage = roll_damage(&mut self.rng, weapon.damage, bonus, armour, crit);
        let what = if crit { "critical hit" } else { "hit" };
        self.log.entries.push(format!("{prefix}{who} {verb} {whom}: {what} for {damage} ({odds})"));
        self.presentation.push(Beat::Floater { at: to, outcome, damage });
        if damage == 0 {
            return Some(0);
        }
        SufferDamage::new_damage(&mut self.commands, target.entity, damage);
        if let Ok(PoisonsOnHit(poison)) = self.poisoners.get(attacker.entity) {
            self.commands.entity(target.entity).insert(*poison);
            self.log.entries.push(format!("{whom} is poisoned!"));
        }
        self.presentation.push(Beat::Flinch { unit: target.entity, at: to, from });
        let spared = self.god_mode.0 && self.party.contains(target.entity);
        Some(if spared { 0 } else { damage })
    }
}

/// Resolves every `WantsToAttack` through `Strikes::strike`.
pub fn attack_system(
    mut strikes: Strikes,
    map: Res<Map>,
    query: Query<(Entity, &WantsToAttack, &Name, &CombatStats, &Position)>,
    targets: Query<(&Name, &CombatStats, &Position)>,
) {
    for (entity, wants, name, stats, pos) in &query {
        strikes.commands.entity(entity).remove::<WantsToAttack>();
        let Ok((target_name, target_stats, target_pos)) = targets.get(wants.target) else {
            continue;
        };
        let attacker = Fighter { entity, name: &name.name, stats, at: (pos.x, pos.y) };
        let (to, name) = ((target_pos.x, target_pos.y), &target_name.name);
        let target = Fighter { entity: wants.target, name, stats: target_stats, at: to };
        strikes.strike(&map, &attacker, &target, false);
    }
}

pub fn damage_system(
    mut commands: Commands,
    god_mode: Res<GodMode>,
    mut map: ResMut<Map>,
    mut query: Query<(Entity, &mut CombatStats, &SufferDamage)>,
    position_query: Query<&Position>,
    party: Query<(), With<PartyMember>>,
) {
    for (entity, mut stats, damage) in &mut query {
        // God mode spares the whole party
        if god_mode.0 && party.contains(entity) {
            commands.entity(entity).remove::<SufferDamage>();
            continue;
        }
        stats.hp -= damage.amount.iter().sum::<i32>();

        // Mark tile as bloody
        if let Ok(pos) = position_query.get(entity) {
            let idx = map.xy_idx(pos.x, pos.y);
            map.bloodstains.insert(idx);
        }

        commands.entity(entity).remove::<SufferDamage>();
    }
}

/// The start of a side's turn for every unit `F` picks: overwatch and
/// hunkering end, poison bites and confusion wears down.
pub fn start_of_turn<F: QueryFilter>(
    mut commands: Commands,
    mut log: ResMut<GameLog>,
    mut units: Query<(Entity, &Name, &CombatStats, Option<&mut Poison>, Option<&mut Confusion>), F>,
) {
    for (entity, name, stats, poison, confusion) in &mut units {
        commands.entity(entity).remove::<(Overwatch, Hunkered)>();
        if stats.hp <= 0 {
            continue;
        }
        if let Some(mut poison) = poison {
            let (damage, left) = poison.tick();
            SufferDamage::new_damage(&mut commands, entity, damage);
            log.entries.push(format!("{} takes {damage} poison damage.", name.name));
            match left {
                Some(left) => *poison = left,
                None => {
                    commands.entity(entity).remove::<Poison>();
                    log.entries.push(format!("{} is no longer poisoned.", name.name));
                }
            }
        }
        if let Some(mut confused) = confusion {
            confused.turns -= 1;
            if confused.turns < 1 {
                commands.entity(entity).remove::<Confusion>();
                log.entries.push(format!("{} is no longer confused.", name.name));
            }
        }
    }
}

/// Despawns everything at 0 hp, leaving a stand-in body to fall
/// (`presentation::fall`). A fallen hero drops what they carry and the
/// selection moves on; the run ends when the last one falls.
#[allow(clippy::too_many_arguments)]
pub fn delete_the_dead(
    mut commands: Commands,
    mut presentation: ResMut<Presentation>,
    bodies: BodyParts,
    mut log: ResMut<GameLog>,
    mut next_state: ResMut<NextState<RunState>>,
    mut active: ResMut<ActiveUnit>,
    query: Query<(Entity, &CombatStats, &Name, Option<&Position>, Option<&Gold>, Option<&PartyMember>)>,
    carried: Query<(Entity, Option<&InBackpack>, Option<&Equipped>), With<Item>>,
) {
    let mut party = Vec::new();
    for (entity, stats, name, pos, gold, member) in &query {
        if let Some(member) = member {
            party.push((member.slot, entity, stats.hp > 0));
        }
        if stats.hp > 0 {
            continue;
        }
        log.entries.push(format!("{} is dead", name.name));
        if let Some(pos) = pos {
            presentation::fall(&mut commands, &mut presentation, &bodies, entity, (pos.x, pos.y));
        }
        commands.entity(entity).despawn();

        if let (Some(pos), Some(&gold)) = (pos, gold) {
            let raws = RAWS.lock().unwrap();
            if let Some(pile) = spawn_named_item(&raws, &mut commands, "Gold", pos.x, pos.y) {
                commands.entity(pile).insert(gold);
            }
        }
        if let (Some(&Position { x, y }), Some(_)) = (pos, member) {
            let owned = |owner: Option<Entity>| owner == Some(entity);
            for (item, backpack, equipped) in &carried {
                if owned(backpack.map(|b| b.owner)) || owned(equipped.map(|e| e.owner)) {
                    commands
                        .entity(item)
                        .remove::<(InBackpack, Equipped)>()
                        .insert((Position { x, y }, Visibility::Inherited));
                }
            }
        }
    }

    if party.iter().any(|&(_, entity, alive)| !alive && active.0 == Some(entity)) {
        active.0 = next_unit(&party, active.0);
    }
    if !party.is_empty() && !party.iter().any(|&(_, _, alive)| alive) {
        log.entries.push("Your whole party is dead".to_string());
        // Delete save file on death (permadeath)
        saveload::delete_save_file();
        next_state.set_if_different(RunState::GameOver);
    }
}

#[cfg(test)]
mod tests {
    use bevy::ecs::system::RunSystemOnce;

    use super::*;

    fn hero(world: &mut World, slot: usize, hp: i32) -> Entity {
        world
            .spawn((
                PartyMember { slot },
                Name { name: format!("Hero {slot}") },
                Position { x: slot as i32, y: 0 },
                CombatStats {
                    max_hp: 30,
                    hp,
                    armour: 0,
                    power: 0,
                    movement: crate::tactics::DEFAULT_MOVEMENT,
                    aim: DEFAULT_AIM,
                    defence: 0,
                },
            ))
            .id()
    }

    fn game_over_pending(world: &World) -> bool {
        matches!(world.resource::<NextState<RunState>>(), NextState::PendingIfDifferent(RunState::GameOver))
    }

    const SWORD: Weapon = Weapon { damage: (3, 5), range: 1, optimal_range: 1, crit_chance: 10 };
    const BOW: Weapon = Weapon { damage: (1, 4), range: 8, optimal_range: 5, crit_chance: 10 };

    const HALF: i32 = crate::cover::HALF_COVER_PENALTY;
    const FULL: i32 = crate::cover::FULL_COVER_PENALTY;

    #[test]
    fn hit_chance_table() {
        // (aim, defence, distance, weapon, cover, height, expected)
        let cases = [
            (65, 0, 1, SWORD, 0, 0, 65),   // plain melee
            (65, 20, 1, SWORD, 0, 0, 45),  // defence comes straight off
            (80, 0, 1, SWORD, 0, 0, 80),   // aim adds straight on
            (65, 0, 2, SWORD, 0, 0, 0),    // melee cannot reach two tiles
            (65, 0, 5, BOW, 0, 0, 65),     // the edge of optimal range: no penalty
            (65, 0, 6, BOW, 0, 0, 55),     // one tile past optimal
            (65, 0, 8, BOW, 0, 0, 35),     // maximum range
            (65, 0, 9, BOW, 0, 0, 0),      // beyond maximum range
            (65, 0, 3, BOW, HALF, 0, 45),  // half cover
            (65, 0, 3, BOW, FULL, 0, 25),  // full cover
            (65, 0, 1, SWORD, FULL, 0, 25), // cover counts against melee too
            (65, 0, 7, BOW, HALF, 0, 25),  // cover stacks with range falloff
            (65, 0, 3, BOW, 0, 1, 75),     // one level above the target
            (65, 0, 3, BOW, 0, -1, 55),    // one level below
            (65, 90, 1, SWORD, 0, 0, 5),   // the floor
            (150, 0, 1, SWORD, 0, 0, 95),  // the ceiling
            (65, 30, 8, BOW, FULL, 0, 5),  // penalties stack down to the floor
        ];
        for (aim, defence, distance, weapon, cover, height, expected) in cases {
            assert_eq!(
                hit_chance(aim, defence, distance, &weapon, cover, height),
                expected,
                "aim {aim} defence {defence} distance {distance} range {} cover {cover} height {height}",
                weapon.range
            );
        }
    }

    #[test]
    fn certain_rolls_go_the_way_the_odds_say() {
        let mut rng = GameRng::seeded(1);
        for _ in 0..50 {
            assert_eq!(roll_attack(&mut rng, 0, 100), Outcome::Miss);
            assert_eq!(roll_attack(&mut rng, 100, 0), Outcome::Hit);
            assert_eq!(roll_attack(&mut rng, 100, 100), Outcome::Crit);
        }
    }

    #[test]
    fn a_seeded_run_rolls_the_same_attacks() {
        let rolls = |seed| {
            let mut rng = GameRng::seeded(seed);
            (0..8)
                .map(|_| {
                    let outcome = roll_attack(&mut rng, 65, 50);
                    let hit = outcome != Outcome::Miss;
                    (outcome, hit.then(|| roll_damage(&mut rng, (1, 4), 5, 1, outcome == Outcome::Crit)))
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(rolls(42), rolls(42));
        use Outcome::*;
        let expected = [
            (Hit, Some(5)),
            (Hit, Some(7)),
            (Miss, None),
            (Hit, Some(5)),
            (Crit, Some(10)),
            (Miss, None),
            (Hit, Some(7)),
            (Crit, Some(8)),
        ];
        assert_eq!(rolls(42), expected);
    }

    #[test]
    fn damage_adds_power_takes_armour_and_crits_add_half() {
        let mut rng = GameRng::seeded(3);
        assert_eq!(roll_damage(&mut rng, (4, 4), 2, 1, false), 5);
        // 6, plus half of 6, less 1 armour.
        assert_eq!(roll_damage(&mut rng, (4, 4), 2, 1, true), 8);
        // A critical's half rounds up: 5 becomes 8.
        assert_eq!(roll_damage(&mut rng, (5, 5), 0, 0, true), 8);
        assert_eq!(roll_damage(&mut rng, (1, 1), 0, 5, false), 0, "armour never heals");
        for _ in 0..50 {
            let natural = roll_damage(&mut rng, Weapon::NATURAL.damage, 4, 0, false);
            assert!((3..=5).contains(&natural), "natural attacks are power give or take one: {natural}");
        }
    }

    #[test]
    fn spells_roll_a_quarter_either_side() {
        assert_eq!(spell_damage(8), (6, 10));
        assert_eq!(spell_damage(20), (15, 25));
    }

    /// A 10 by 3 corridor of floor with a wall at (4, 1).
    fn corridor() -> Map {
        let mut map = Map::new(10, 3, 2);
        for x in 0..10 {
            let idx = map.xy_idx(x, 1);
            map.tiles[idx] = crate::map::TileType::Floor;
        }
        map
    }

    #[test]
    fn a_shot_needs_range_and_a_clear_line() {
        let mut map = corridor();
        assert_eq!(in_reach(&map, &BOW, (0, 1), (7, 1)), Ok(7));
        assert_eq!(in_reach(&map, &BOW, (0, 1), (9, 1)), Err(Refusal::OutOfRange));
        assert_eq!(in_reach(&map, &SWORD, (0, 1), (1, 0)), Ok(1), "diagonal melee reaches");
        // Walls north and south, the shot from the west: flanked, no cover.
        assert_eq!(aim_at(&map, &BOW, 65, (0, 1), 10, (7, 1), false), Ok((35, Exposure::Flanked)));
        let idx = map.xy_idx(4, 1);
        map.tiles[idx] = crate::map::TileType::Wall;
        assert_eq!(in_reach(&map, &BOW, (0, 1), (7, 1)), Err(Refusal::NoLineOfSight));
        assert_eq!(in_reach(&map, &BOW, (0, 1), (3, 1)), Ok(3), "the wall is beyond the target");
    }

    #[test]
    fn cover_comes_off_the_chance_and_shows_in_the_odds() {
        use crate::cover::Cover;
        let mut map = corridor();
        // Open the row above, then put a table east of the target at (5, 1).
        for x in 0..10 {
            let idx = map.xy_idx(x, 0);
            map.tiles[idx] = crate::map::TileType::Floor;
        }
        let table = map.xy_idx(6, 1);
        map.prop_cover.insert(table, Cover::Half);
        let (chance, cover) = aim_at(&map, &BOW, 65, (8, 1), 0, (5, 1), false).unwrap();
        assert_eq!((chance, odds(chance, cover)), (45, "45%, half cover".to_string()));
        let (chance, cover) = aim_at(&map, &BOW, 65, (2, 1), 0, (5, 1), false).unwrap();
        assert_eq!((chance, odds(chance, cover)), (65, "65%, flanked".to_string()));
        assert_eq!(odds(65, Exposure::Open), "65%");
        // Hunkered behind the table: the half cover counts as 40.
        let (chance, cover) = aim_at(&map, &BOW, 65, (8, 1), 0, (5, 1), true).unwrap();
        assert_eq!((chance, odds(chance, cover)), (25, "25%, half cover, hunkered".to_string()));
    }

    #[test]
    fn a_fallen_hero_drops_their_kit_and_the_run_ends_with_the_last() {
        let mut world = World::new();
        world.insert_resource(GameLog { entries: Vec::new() });
        world.init_resource::<NextState<RunState>>();
        world.init_resource::<Presentation>();
        let first = hero(&mut world, 0, 0);
        let second = hero(&mut world, 1, 12);
        world.insert_resource(ActiveUnit(Some(first)));
        let dagger = world.spawn((Item, InBackpack { owner: first })).id();

        world.run_system_once(delete_the_dead).unwrap();

        assert!(world.get_entity(first).is_err(), "the fallen hero leaves play");
        assert_eq!(world.resource::<ActiveUnit>().0, Some(second));
        let dropped = world.get::<Position>(dagger).map(|p| (p.x, p.y));
        assert_eq!(dropped, Some((0, 0)));
        assert!(world.get::<InBackpack>(dagger).is_none());
        assert!(!game_over_pending(&world), "one hero still stands");

        world.get_mut::<CombatStats>(second).unwrap().hp = 0;
        world.run_system_once(delete_the_dead).unwrap();

        assert!(game_over_pending(&world));
        assert_eq!(world.resource::<ActiveUnit>().0, None);
        assert!(world.resource::<GameLog>().entries.contains(&"Your whole party is dead".to_string()));
    }
}
