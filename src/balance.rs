//! The balance simulator (P04): whole runs played headless with the game's
//! own rules and data, so every balance number can be measured again.
//!
//! Run it with
//! `cargo test --release balance::report -- --ignored --nocapture`
//! (set `BALANCE_CAMPAIGNS` for more or fewer campaigns, default 200, and see
//! `knob` for what-if changes without touching the raws).
//!
//! What is real: the raws (classes, gear, abilities, monsters, items, spawn
//! table), the level generators and spawn rolls (`map_builders::level_builder`,
//! `spawner::room_spawns`, every builder's spawn list), the alert-group rule
//! (`enemy_ai::ALERT_RADIUS`), `combat::hit_chance` / `roll_attack` /
//! `roll_damage` / `spell_damage`, cover's aim penalties and crit rules,
//! `experience` (xp, levels, level gains), the stairs' half heal, and the
//! whole economy (`shop::sell` / `buy` / `recruits`, `trade::simulate`,
//! `stock::restock`, the sector thresholds).
//!
//! What is abstracted (the simplifications, also listed in `docs/design.md`):
//! - A fight has no map. Each alert group is one fight; both sides attack every
//!   round. In round 1 a melee unit reaches its foe with `OPENING_REACH`.
//!   Ranged units shoot at their weapon's optimal range.
//! - Cover is a draw per attack (`hero_exposure`, `foe_exposure`), not a place.
//!   Overwatch, hunkering, line of sight and movement are not modelled.
//! - Monsters target heroes at random, the front line (`FRONT`) more often.
//! - Fireball hits `FIREBALL_HITS` hostiles and no hero; Cleave hits
//!   `CLEAVE_HITS`. Snare costs the target its next turn.
//! - Between fights the party rests to full (resting is 1 hp a turn with no
//!   hostile in view). Hunger, traps and found scrolls are not modelled; scrolls
//!   are sold, food is kept, healing potions are drunk in fights.
//! - Descending, the party clears the level. Climbing with the Heart it fights
//!   only the groups within `IN_THE_WAY` of the path between the stairs.
//! - The party portals home (when it holds a Town Portal) after
//!   `TRIP_LEVELS` levels or after losing a hero, sells its loot, hires
//!   recruits, and buys a portal, potions and gear upgrades.

use rand::RngExt;

use crate::combat::{hit_chance, roll_attack, roll_damage, spell_damage, CombatStats, Outcome, LEVEL_GROUND, SPELL_AIM_BONUS};
use crate::components::Weapon;
use crate::cover::{Cover, Exposure};
use crate::economy::shop::{buy, recruits, sell};
use crate::economy::stock::restock;
use crate::economy::town::{stocked_by, TownState};
use crate::economy::trade::simulate;
use crate::enemy_ai::{Archetype, ALERT_RADIUS};
use crate::experience::{level_for, level_up, mob_xp};
use crate::levels::{DEEPEST, HEART_OF_THE_ABYSS};
use crate::map::TileType;
use crate::raws::{load_raws, AbilityEffect, Item, Mob, Raws, RAWS};
use crate::rng::GameRng;

// ============================================================================
// The abstraction's knobs (guesses, checked against the harness; see design.md)
// ============================================================================

/// Chance a melee unit reaches its foe in the first round.
const OPENING_REACH: f64 = 0.5;
/// Hostiles one Fireball catches (when that many stand).
const FIREBALL_HITS: usize = 2;
/// Hostiles one Cleave hits (when that many stand).
const CLEAVE_HITS: usize = 2;
/// Classes in front: melee monsters pick them `FRONT_WEIGHT` times as often.
const FRONT: [&str; 2] = ["Fighter", "Cleric"];
const FRONT_WEIGHT: u32 = 3;
/// A heal goes to a hero below this share of their hit points.
const HEAL_BELOW: f64 = 0.5;
/// A hero below this share drinks a potion.
const POTION_BELOW: f64 = 0.3;
/// Climbing, a group or item this close (Chebyshev) to the stairs path is met.
const IN_THE_WAY: i32 = 6;
/// Levels per trip before the party portals home.
const TRIP_LEVELS: u32 = 2;
/// Potions the party buys per living hero.
const POTIONS_PER_HERO: usize = 2;
/// A group bigger than this arrives in waves of this many.
const WAVE: usize = 6;
/// A fight that lasts this long is called off (both sides live).
const MAX_ROUNDS: u32 = 40;
/// Generated levels per depth that runs draw from.
const POOL: usize = 40;

/// How a hero stands against an attacker of `archetype`: skirmishers flank,
/// the rest sometimes face a hero who hugs a wall.
fn hero_exposure(rng: &mut GameRng, archetype: Archetype) -> Exposure {
    let roll = rng.0.random_range(0..100);
    match archetype {
        Archetype::Skirmisher | Archetype::Coward if roll < 30 => Exposure::Flanked,
        Archetype::Brute | Archetype::Ranged if roll < 25 => covered(Cover::Full),
        _ => Exposure::Open,
    }
}

/// How a hostile of `archetype` stands against a hero: brutes stand in the
/// open, the rest take cover when they can.
fn foe_exposure(rng: &mut GameRng, archetype: Archetype) -> Exposure {
    let roll = rng.0.random_range(0..100);
    match archetype {
        Archetype::Brute => Exposure::Open,
        Archetype::Ranged if roll < 50 => covered(Cover::Full),
        Archetype::Skirmisher | Archetype::Coward if roll < 25 => covered(Cover::Full),
        _ => Exposure::Open,
    }
}

fn covered(cover: Cover) -> Exposure {
    Exposure::Covered { cover, side: (0, 1) }
}

// ============================================================================
// Levels from the real generators
// ============================================================================

/// One alert group: the hostiles that wake together, and whether they stand
/// near the path between the stairs.
#[derive(Clone, Debug)]
struct Group {
    mobs: Vec<String>,
    in_way: bool,
}

#[derive(Clone, Debug, Default)]
struct Level {
    groups: Vec<Group>,
    /// Items on the floor, each with whether it lies near the stairs path.
    items: Vec<(String, bool)>,
}

/// Builds a level at `depth` with the game's generator and lists what it spawns.
fn generate(depth: i32, seed: u64) -> Level {
    let mut rng = GameRng::seeded(seed);
    let town = TownState::fresh(0);
    let mut builder = crate::map_builders::level_builder(depth, &mut rng, &town, false);
    builder.build_map(&mut rng);
    let map = builder.get_map();
    let start = builder.get_starting_position();
    let raws = RAWS.lock().unwrap();
    let mut spawns: Vec<((i32, i32), String)> = Vec::new();
    for room in builder.rooms().iter().skip(1) {
        spawns.extend(crate::spawner::room_spawns(&raws, &mut rng, room, depth));
    }
    let w = map.width;
    spawns.extend(builder.spawn_list().iter().map(|(idx, name)| ((*idx as i32 % w, *idx as i32 / w), name.clone())));

    // The path the party walks: up stairs to down stairs, or to the Heart.
    let goal = map
        .tiles
        .iter()
        .position(|&t| t == TileType::DownStairs)
        .or_else(|| spawns.iter().find(|s| s.1 == HEART_OF_THE_ABYSS).map(|s| map.xy_idx(s.0 .0, s.0 .1)));
    let path: Vec<(i32, i32)> = goal
        .and_then(|g| crate::pathfinding::a_star_ignoring_entities(&map, map.xy_idx(start.0, start.1), g))
        .unwrap_or_default()
        .into_iter()
        .map(|i| (i as i32 % w, i as i32 / w))
        .collect();
    let near = |at: (i32, i32)| path.is_empty() || path.iter().any(|&p| crate::combat::chebyshev(p, at) <= IN_THE_WAY);

    let hostile = |name: &str| raws.mob_index.get(name).is_some_and(|&i| raws.raws.mobs[i].ai.is_none());
    let mut hostiles: Vec<((i32, i32), String)> = spawns.iter().filter(|s| hostile(&s.1)).cloned().collect();
    let items = spawns
        .iter()
        .filter(|s| raws.item_index.contains_key(&s.1))
        .map(|(at, name)| (name.clone(), near(*at)))
        .collect();

    // Chain hostiles within ALERT_RADIUS into groups, as `enemy_ai::spread_alert` wakes them.
    let mut groups = Vec::new();
    while let Some(first) = hostiles.pop() {
        let mut members = vec![first];
        let mut i = 0;
        while i < members.len() {
            let at = members[i].0;
            let (joined, rest): (Vec<_>, Vec<_>) =
                hostiles.into_iter().partition(|h| crate::combat::chebyshev(h.0, at) <= ALERT_RADIUS);
            members.extend(joined);
            hostiles = rest;
            i += 1;
        }
        let in_way = members.iter().any(|m| near(m.0));
        groups.push(Group { mobs: members.into_iter().map(|m| m.1).collect(), in_way });
    }
    Level { groups, items }
}

/// `POOL` generated levels for every depth, index 0 unused.
fn level_pool() -> Vec<Vec<Level>> {
    load_raws();
    (0..=DEEPEST)
        .map(|depth| if depth == 0 { Vec::new() } else { (0..POOL).map(|i| generate(depth, 1000 * depth as u64 + i as u64)).collect() })
        .collect()
}

// ============================================================================
// Units
// ============================================================================

#[derive(Clone, Debug)]
struct Power {
    effect: AbilityEffect,
    cooldown: u8,
    left: u8,
}

impl Power {
    fn spell(&self, key: &str) -> Option<i32> {
        match &self.effect {
            AbilityEffect::Spell(effects) => effects.get(key).map(|v| v.parse().unwrap_or(0)),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
struct Hero {
    class: String,
    base_hp: i32,
    stats: CombatStats,
    level: i32,
    xp: i32,
    weapon: Weapon,
    /// Armour bonuses worn: shield, chest, cloak.
    worn: [i32; 3],
    powers: Vec<Power>,
    poison: Option<(i32, i32)>,
    /// Damage dealt and hit points healed, for the class shares.
    dealt: i32,
}

impl Hero {
    fn new(raws: &Raws, class: &str, level: i32) -> Hero {
        let c = raws.classes.iter().find(|c| c.name == class).unwrap();
        let s = &c.stats;
        let mut hero = Hero {
            class: class.to_string(),
            base_hp: s.max_hp,
            stats: CombatStats {
                max_hp: s.max_hp,
                hp: s.max_hp,
                armour: s.armour,
                power: s.power,
                movement: s.movement,
                aim: s.aim,
                defence: s.defence,
            },
            level: 1,
            xp: 0,
            weapon: Weapon::NATURAL,
            worn: [0; 3],
            powers: c
                .abilities
                .iter()
                .filter_map(|n| raws.abilities.iter().find(|a| &a.name == n))
                .map(|a| Power { effect: a.effect.clone(), cooldown: a.cooldown, left: 0 })
                .collect(),
            poison: None,
            dealt: 0,
        };
        for name in &c.gear {
            let item = raws.items.iter().find(|i| &i.name == name).unwrap();
            if gear_gain(&hero, item) > 0 {
                wear(&mut hero, item);
            }
        }
        for _ in 1..level {
            level_up(&mut hero.stats, hero.base_hp);
        }
        hero.level = level;
        hero.xp = crate::experience::xp_for(level);
        hero
    }

    fn alive(&self) -> bool {
        self.stats.hp > 0
    }

    fn share(&self) -> f64 {
        self.stats.hp as f64 / self.stats.max_hp as f64
    }

    fn armour(&self) -> i32 {
        self.stats.armour + self.worn.iter().sum::<i32>()
    }

    fn heal(&mut self, amount: i32) {
        self.stats.hp = crate::abilities::healed(self.stats.hp, self.stats.max_hp, amount);
    }

    fn gain_xp(&mut self, xp: i32) {
        self.xp += xp * knob("XP_PCT", 100) / 100;
        while self.level < level_for(self.xp) {
            self.level += 1;
            level_up(&mut self.stats, self.base_hp);
        }
    }
}

fn weapon_of(w: &crate::raws::Weapon) -> Weapon {
    Weapon { damage: w.damage, range: w.range, optimal_range: w.optimal_range.unwrap_or(w.range), crit_chance: w.crit_chance }
}

fn mean(w: &Weapon) -> i32 {
    w.damage.0 + w.damage.1
}

/// How much better `item` makes `hero`: twice the mean damage of a weapon
/// they would use, or the armour gained. 0 when it is no better.
fn gear_gain(hero: &Hero, item: &Item) -> i32 {
    if let Some(w) = &item.weapon {
        let ranged = w.range > 1;
        let wants = if hero.class == "Ranger" { ranged } else { !ranged && hero.class != "Mage" || hero.weapon == Weapon::NATURAL };
        return if wants { (mean(&weapon_of(w)) - mean(&hero.weapon)).max(0) } else { 0 };
    }
    match armour_slot(item) {
        Some((slot, bonus)) => (bonus - hero.worn[slot]).max(0),
        None => 0,
    }
}

fn armour_slot(item: &Item) -> Option<(usize, i32)> {
    if let Some(s) = &item.shield {
        Some((0, s.defense_bonus))
    } else {
        item.armour.as_ref().map(|a| (if a.slot == "cloak" { 2 } else { 1 }, a.defense_bonus))
    }
}

/// Puts `item` on `hero`; returns nothing (what it replaces is not kept: the
/// sim sells it at once in `Party::take`).
fn wear(hero: &mut Hero, item: &Item) {
    if let Some(w) = &item.weapon {
        hero.weapon = weapon_of(w);
    } else if let Some((slot, bonus)) = armour_slot(item) {
        hero.worn[slot] = bonus;
    }
}

#[derive(Clone, Debug)]
struct Foe {
    stats: CombatStats,
    weapon: Weapon,
    archetype: Archetype,
    xp: i32,
    gold: (i32, i32),
    poison: Option<(i32, i32)>,
    confused: i32,
}

impl Foe {
    fn new(raws: &Raws, name: &str) -> Foe {
        let m: &Mob = raws.mobs.iter().find(|m| m.name == name).unwrap();
        let s = &m.stats;
        let weapon = m
            .weapon
            .as_ref()
            .and_then(|w| raws.items.iter().find(|i| &i.name == w)?.weapon.as_ref())
            .map_or(Weapon::NATURAL, weapon_of);
        let depth = raws.spawn_table.iter().find(|e| e.name == name).map_or(1, |e| e.min_depth.max(1));
        let hp = s.max_hp * (knob("HP_PCT", 100) + knob("HP_DEPTH", 0) * depth) / 100;
        Foe {
            stats: CombatStats {
                max_hp: hp,
                hp,
                armour: s.defense,
                power: s.power + knob("POWER_ADD", 0) + knob("POWER_DEPTH10", 0) * depth / 10,
                movement: s.movement.unwrap_or(crate::tactics::DEFAULT_MOVEMENT),
                aim: s.aim.unwrap_or(crate::combat::DEFAULT_AIM),
                defence: s.defence,
            },
            weapon,
            archetype: m.archetype.unwrap_or_default(),
            xp: m.xp.unwrap_or_else(|| mob_xp(s.max_hp, s.power)),
            gold: m.gold.unwrap_or((0, 0)),
            poison: m.poison,
            confused: 0,
        }
    }

    fn alive(&self) -> bool {
        self.stats.hp > 0
    }
}

/// One attack by the game's rules: hit chance less cover, crit (more on a
/// flank), damage plus `bonus` less `armour`. Ranged weapons fire from their
/// optimal range.
fn strike(rng: &mut GameRng, aim: i32, weapon: &Weapon, bonus: i32, defence: i32, armour: i32, exposure: Exposure) -> i32 {
    let chance = hit_chance(aim, defence, weapon.optimal_range, weapon, exposure.aim_penalty(), LEVEL_GROUND);
    match roll_attack(rng, chance, exposure.crit_chance(weapon.crit_chance)) {
        Outcome::Miss => 0,
        outcome => roll_damage(rng, weapon.damage, bonus, armour, outcome == Outcome::Crit),
    }
}

// ============================================================================
// A fight
// ============================================================================

#[derive(Default)]
struct Fight {
    xp: i32,
    gold: i32,
    won: bool,
    rounds: u32,
}

fn living(foes: &[Foe]) -> usize {
    foes.iter().filter(|f| f.alive()).count()
}

/// The hostile the party focuses: the one nearest death.
fn focus(foes: &[Foe]) -> Option<usize> {
    (0..foes.len()).filter(|&i| foes[i].alive()).min_by_key(|&i| foes[i].stats.hp)
}

/// One fight to the end. `staged` is the harness check (`balance_lord`,
/// `balance_d3`): everyone in reach from the start, in the open, and the
/// heroes only attack with their weapons, as pressing F does.
fn fight(rng: &mut GameRng, heroes: &mut [Option<Hero>], potions: &mut Vec<i32>, mut foes: Vec<Foe>, staged: bool) -> Fight {
    let mut result = Fight::default();
    for hero in heroes.iter_mut().flatten() {
        hero.powers.iter_mut().for_each(|p| p.left = 0);
        hero.poison = None;
    }
    let kill = |foe: &Foe, heroes: &mut [Option<Hero>], rng: &mut GameRng, result: &mut Fight| {
        result.xp += foe.xp;
        result.gold += rng.0.random_range(foe.gold.0..=foe.gold.1);
        for hero in heroes.iter_mut().flatten().filter(|h| h.alive()) {
            hero.gain_xp(foe.xp);
        }
    };
    for round in 1..=MAX_ROUNDS {
        result.rounds = round;
        // The party's phase, in slot order.
        for slot in 0..heroes.len() {
            let Some(hero) = heroes[slot].as_mut().filter(|h| h.alive()) else { continue };
            hero.powers.iter_mut().for_each(|p| p.left = p.left.saturating_sub(1));
            if let Some((damage, turns)) = hero.poison {
                hero.stats.hp -= damage;
                hero.poison = (turns > 1).then_some((damage, turns - 1));
                if !hero.alive() {
                    continue;
                }
            }
            if living(&foes) == 0 {
                break;
            }
            hero_turn(rng, slot, heroes, potions, &mut foes, round, staged, &mut |f, h, r| kill(f, h, r, &mut result));
        }
        if living(&foes) == 0 {
            result.won = true;
            return result;
        }
        // The hostiles' phase.
        for i in 0..foes.len() {
            if !foes[i].alive() {
                continue;
            }
            if foes[i].confused > 0 {
                foes[i].confused -= 1;
                if foes[i].confused > 0 {
                    continue;
                }
            }
            let foe = &foes[i];
            let wounded = foe.stats.hp * 3 <= foe.stats.max_hp;
            if foe.archetype == Archetype::Coward && (living(&foes) == 1 || wounded) {
                continue;
            }
            let ranged = foe.weapon.range > 1;
            if round == 1 && !ranged && !staged && !rng.0.random_bool(OPENING_REACH) {
                continue;
            }
            let weights: Vec<u32> = heroes
                .iter()
                .map(|h| match h.as_ref().filter(|h| h.alive()) {
                    None => 0,
                    Some(h) if !ranged && FRONT.contains(&h.class.as_str()) => FRONT_WEIGHT,
                    Some(_) => 1,
                })
                .collect();
            let total: u32 = weights.iter().sum();
            if total == 0 {
                return result;
            }
            let mut pick = rng.0.random_range(0..total);
            let target = weights.iter().position(|&w| if pick < w { true } else { pick -= w; false }).unwrap();
            let hero = heroes[target].as_mut().unwrap();
            let exposure = if staged { Exposure::Open } else { hero_exposure(rng, foe.archetype) };
            let damage = strike(rng, foe.stats.aim, &foe.weapon, foe.stats.power, hero.stats.defence, hero.armour(), exposure);
            hero.stats.hp -= damage;
            if damage > 0 {
                if let Some(poison) = foe.poison {
                    hero.poison = Some(poison);
                }
            }
        }
        if heroes.iter().flatten().all(|h| !h.alive()) {
            return result;
        }
    }
    result
}

/// One hero's turn: a heal or a potion first if someone needs it (1 AP), then
/// the best attack it has (which ends the turn).
fn hero_turn(
    rng: &mut GameRng,
    slot: usize,
    heroes: &mut [Option<Hero>],
    potions: &mut Vec<i32>,
    foes: &mut [Foe],
    round: u32,
    staged: bool,
    kill: &mut dyn FnMut(&Foe, &mut [Option<Hero>], &mut GameRng),
) {
    let hero = heroes[slot].as_ref().unwrap();
    let ready = |name: &dyn Fn(&Power) -> bool| hero.powers.iter().position(|p| !staged && p.left == 0 && name(p));
    // Support for 1 AP.
    let heal_ally = ready(&|p| p.spell("provides_healing").is_some() && p.spell("ranged").is_some());
    let heal_self = ready(&|p| p.spell("provides_healing").is_some() && p.spell("ranged").is_none());
    let neediest = (0..heroes.len())
        .filter(|&i| heroes[i].as_ref().is_some_and(|h| h.alive() && h.share() < HEAL_BELOW))
        .min_by(|&a, &b| heroes[a].as_ref().unwrap().share().total_cmp(&heroes[b].as_ref().unwrap().share()));
    let own_share = hero.share();
    if let (Some(p), Some(patient)) = (heal_ally, neediest) {
        let amount = hero.powers[p].spell("provides_healing").unwrap();
        let h = heroes[slot].as_mut().unwrap();
        h.powers[p].left = h.powers[p].cooldown;
        let patient = heroes[patient].as_mut().unwrap();
        let before = patient.stats.hp;
        patient.heal(amount);
        let healed = patient.stats.hp - before;
        heroes[slot].as_mut().unwrap().dealt += healed;
    } else if let (Some(p), true) = (heal_self, own_share < HEAL_BELOW) {
        let h = heroes[slot].as_mut().unwrap();
        let amount = h.powers[p].spell("provides_healing").unwrap();
        h.powers[p].left = h.powers[p].cooldown;
        let before = h.stats.hp;
        h.heal(amount);
        h.dealt += h.stats.hp - before;
    } else if own_share < POTION_BELOW && !potions.is_empty() {
        let best = (0..potions.len()).max_by_key(|&i| potions[i]).unwrap();
        let amount = potions.swap_remove(best);
        heroes[slot].as_mut().unwrap().heal(amount);
    }

    // The attack, which ends the turn.
    let hero = heroes[slot].as_mut().unwrap();
    let Some(target) = focus(foes) else { return };
    let engaged = round > 1 || staged || rng.0.random_bool(OPENING_REACH);
    let reach = hero.weapon.range > 1 || engaged;
    let many = living(foes) >= 2;
    let pick = |hero: &Hero, test: &dyn Fn(&Power) -> bool| hero.powers.iter().position(|p| !staged && p.left == 0 && test(p));
    let foe_exposure = |rng: &mut GameRng, archetype| if staged { Exposure::Open } else { foe_exposure(rng, archetype) };
    let area = pick(hero, &|p| p.spell("area_of_effect").is_some() && p.spell("damage").is_some()).filter(|_| many);
    let snare = pick(hero, &|p| p.spell("confusion").is_some()).filter(|_| many);
    let cleave = pick(hero, &|p| p.effect == AbilityEffect::Cleave).filter(|_| many && engaged);
    let aimed = pick(hero, &|p| matches!(p.effect, AbilityEffect::AimedShot { .. })).filter(|_| reach);
    let bolt = pick(hero, &|p| p.spell("damage").is_some() && p.spell("area_of_effect").is_none());
    let hit = |rng: &mut GameRng, hero: &Hero, foe: &mut Foe, aim_bonus: i32| {
        let exposure = foe_exposure(rng, foe.archetype);
        let d = strike(rng, hero.stats.aim + aim_bonus, &hero.weapon, hero.stats.power, foe.stats.defence, foe.stats.armour, exposure);
        foe.stats.hp -= d;
    };
    let standing = |foes: &[Foe]| foes.iter().map(|f| f.stats.hp.max(0)).sum::<i32>();
    let before = standing(foes);
    let mut struck: Vec<usize> = Vec::new();
    if let Some(p) = area {
        let damage = spell_damage(hero.powers[p].spell("damage").unwrap());
        hero.powers[p].left = hero.powers[p].cooldown;
        let mut order: Vec<usize> = (0..foes.len()).filter(|&i| foes[i].alive()).collect();
        order.sort_by_key(|&i| (i != target, i));
        for &i in order.iter().take(FIREBALL_HITS) {
            foes[i].stats.hp -= roll_damage(rng, damage, 0, 0, false);
            struck.push(i);
        }
    } else if let Some(p) = snare {
        hero.powers[p].left = hero.powers[p].cooldown;
        let turns = hero.powers[p].spell("confusion").unwrap();
        if let Some(i) = (0..foes.len()).filter(|&i| foes[i].alive() && foes[i].confused == 0).max_by_key(|&i| foes[i].stats.power) {
            foes[i].confused = turns;
        }
    } else if let Some(p) = cleave {
        hero.powers[p].left = hero.powers[p].cooldown;
        let mut order: Vec<usize> = (0..foes.len()).filter(|&i| foes[i].alive()).collect();
        order.sort_by_key(|&i| (i != target, i));
        for &i in order.iter().take(CLEAVE_HITS) {
            hit(rng, hero, &mut foes[i], 0);
            struck.push(i);
        }
    } else if let Some(p) = aimed {
        let AbilityEffect::AimedShot { aim_bonus } = hero.powers[p].effect else { unreachable!() };
        hero.powers[p].left = hero.powers[p].cooldown;
        hit(rng, hero, &mut foes[target], aim_bonus);
        struck.push(target);
    } else if let Some(p) = bolt {
        let damage = spell_damage(hero.powers[p].spell("damage").unwrap());
        let range = hero.powers[p].spell("ranged").unwrap_or(1);
        hero.powers[p].left = hero.powers[p].cooldown;
        let spell = Weapon { damage, range, optimal_range: range, crit_chance: 0 };
        let foe = &mut foes[target];
        let exposure = foe_exposure(rng, foe.archetype);
        foe.stats.hp -= strike(rng, hero.stats.aim + SPELL_AIM_BONUS, &spell, 0, foe.stats.defence, 0, exposure);
        struck.push(target);
    } else if reach {
        hit(rng, hero, &mut foes[target], 0);
        struck.push(target);
    }
    hero.dealt += before - standing(foes);
    for i in struck {
        if foes[i].stats.hp <= 0 && foes[i].stats.hp > -1000 {
            let dead = foes[i].clone();
            foes[i].stats.hp = -1000;
            kill(&dead, heroes, rng);
        }
    }
}

// ============================================================================
// Runs and the town
// ============================================================================

struct Party {
    heroes: Vec<Option<Hero>>,
    gold: i32,
    potions: Vec<i32>,
    portals: u32,
    loot: Vec<String>,
    /// Damage dealt and healed by each class, the fallen's included.
    dealt: [i64; 4],
}

const CLASSES: [&str; 4] = ["Fighter", "Ranger", "Mage", "Cleric"];

impl Party {
    fn new(raws: &Raws) -> Party {
        Party { heroes: CLASSES.iter().map(|c| Some(Hero::new(raws, c, 1))).collect(), gold: 0, potions: Vec::new(), portals: 0, loot: Vec::new(), dealt: [0; 4] }
    }

    fn alive(&self) -> usize {
        self.heroes.iter().flatten().filter(|h| h.alive()).count()
    }

    /// Bury the dead: their slots empty for recruits.
    fn bury(&mut self) {
        for slot in self.heroes.iter_mut() {
            if let Some(hero) = slot.as_mut() {
                self.dealt[CLASSES.iter().position(|c| *c == hero.class).unwrap()] += std::mem::take(&mut hero.dealt) as i64;
            }
            if slot.as_ref().is_some_and(|h| !h.alive()) {
                *slot = None;
            }
        }
    }

    /// Pick up `name`: worn if it beats what a hero has, drunk later if a
    /// potion, kept if a portal or food, else carried home to sell.
    fn take(&mut self, raws: &Raws, name: &str) {
        let Some(item) = raws.items.iter().find(|i| i.name == name) else { return };
        if let Some((min, max)) = item.gold {
            self.gold += (min + max) / 2;
            return;
        }
        let effects = item.consumable.as_ref().map(|c| &c.effects);
        let has = |key: &str| effects.is_some_and(|e| e.contains_key(key));
        if has("provides_food") {
            return;
        }
        if has("provides_healing") && !has("ranged") {
            self.potions.push(effects.unwrap()["provides_healing"].parse().unwrap());
            return;
        }
        if has("town_portal") && self.portals == 0 {
            self.portals = 1;
            return;
        }
        let best = (0..self.heroes.len())
            .filter_map(|i| Some((i, gear_gain(self.heroes[i].as_ref().filter(|h| h.alive())?, item))))
            .max_by_key(|&(i, gain)| (gain, std::cmp::Reverse(i)));
        match best {
            Some((i, gain)) if gain > 0 => wear(self.heroes[i].as_mut().unwrap(), item),
            _ => self.loot.push(name.to_string()),
        }
    }
}

/// What one run (a party from new game to wipe or victory) did.
#[derive(Clone, Debug, Default)]
struct RunLog {
    /// Per depth descending: (arrived, survived, heroes lost, sum of mean party level on arrival).
    down: [(u32, u32, u32, f64); 13],
    /// Per depth descending: the sum of the party's mean armour on arrival.
    armour: [f64; 13],
    /// Per depth climbing: (arrived, survived).
    up: [(u32, u32); 13],
    guard: Option<bool>,
    won: bool,
    gold: i32,
    sold: i32,
    town_level: u32,
    deepest: i32,
    dealt: [i64; 4],
    /// Heroes found fallen on a town visit, and recruits hired for them.
    fallen: u32,
    hired: u32,
}

/// The guard fight on depth 12 is the group with the Demon Lord the Heart's guard.
fn play_level(rng: &mut GameRng, raws: &Raws, party: &mut Party, level: &Level, climbing: bool, log: &mut RunLog) -> Option<bool> {
    let mut guard = None;
    for group in level.groups.iter().filter(|g| !climbing || g.in_way) {
        let is_guard = !climbing && level.items.iter().any(|i| i.0 == HEART_OF_THE_ABYSS) && group.mobs.iter().any(|m| m == "Demon Lord");
        // A big group arrives in waves, with no rest between them.
        for wave in group.mobs.chunks(WAVE) {
            let foes: Vec<Foe> = wave.iter().map(|n| Foe::new(raws, n)).collect();
            let result = fight(rng, &mut party.heroes, &mut party.potions, foes, false);
            party.gold += result.gold;
            log.gold += result.gold;
            if is_guard {
                guard = Some(result.won && guard != Some(false));
            }
            if party.alive() == 0 {
                return guard;
            }
        }
        // Rest to full between fights.
        for hero in party.heroes.iter_mut().flatten().filter(|h| h.alive()) {
            hero.stats.hp = (hero.stats.hp + hero.stats.max_hp * knob("REST_PCT", 100) / 100).min(hero.stats.max_hp);
        }
    }
    for (name, near) in &level.items {
        let gear = raws.items.iter().any(|i| &i.name == name && (i.weapon.is_some() || i.armour.is_some() || i.shield.is_some()));
        if gear && rng.0.random_range(0..100) >= knob("GEAR_PCT", 100) {
            continue;
        }
        if (!climbing || *near) && name != HEART_OF_THE_ABYSS {
            party.take(raws, name);
        }
    }
    guard
}

fn town_visit(raws: &Raws, town: &mut TownState, party: &mut Party, days: u32, log: &mut RunLog) {
    if days > 0 {
        simulate(town, days);
        restock(town, &raws.items);
    }
    party.bury();
    for name in std::mem::take(&mut party.loot) {
        let item = raws.items.iter().find(|i| i.name == name).unwrap();
        let Some(vendor) = item.sector.and_then(|s| stocked_by(s, item.tier).next()) else { continue };
        let before = party.gold;
        if sell(town, &mut party.gold, vendor, item).is_ok() {
            log.sold += party.gold - before;
        }
    }
    // Recruits for the fallen, cheapest classes missing first.
    let names: Vec<&str> = Vec::new();
    for slot in 0..party.heroes.len() {
        if party.heroes[slot].is_some() {
            continue;
        }
        let class = CLASSES[slot];
        log.fallen += 1;
        if let Some(r) = recruits(town, &names).into_iter().find(|r| r.class == class) {
            if party.gold >= r.price {
                log.hired += 1;
                party.gold -= r.price;
                party.heroes[slot] = Some(Hero::new(raws, class, r.level));
            }
        }
    }
    let shop = |town: &mut TownState, party: &mut Party, name: &str| -> bool {
        let item = raws.items.iter().find(|i| i.name == name).unwrap();
        let Some(vendor) = item.sector.and_then(|s| stocked_by(s, item.tier).next()) else { return false };
        buy(town, &mut party.gold, vendor, item).is_ok()
    };
    if party.portals == 0 && shop(town, party, "Town Portal") {
        party.portals = 1;
    }
    let want = |party: &Party, per: usize| party.alive() * per;
    let potion_names: Vec<(&str, i32)> = raws
        .items
        .iter()
        .filter_map(|i| {
            let e = &i.consumable.as_ref()?.effects;
            (!e.contains_key("ranged") && !e.contains_key("provides_food")).then_some((i.name.as_str(), e.get("provides_healing")?.parse().ok()?))
        })
        .collect();
    // The strongest potions on the shelf the purse pays for, then cheaper ones.
    let buy_potions = |town: &mut TownState, party: &mut Party, per: usize| {
        let mut order = potion_names.clone();
        order.sort_by_key(|p| -p.1);
        for &(name, heal) in &order {
            let item = raws.items.iter().find(|i| i.name == name).unwrap();
            let price = |town: &TownState| crate::economy::trade::buy_price(town, item.sector.unwrap(), item.base_value);
            // No potion takes more than its share of the purse.
            let fair = |town: &TownState, party: &Party| price(town) * (want(party, per) - party.potions.len()) as i32 <= party.gold;
            while party.potions.len() < want(party, per) && fair(town, party) && shop(town, party, name) {
                party.potions.push(heal);
            }
        }
        let &(name, heal) = order.last().unwrap();
        while party.potions.len() < want(party, per) && shop(town, party, name) {
            party.potions.push(heal);
        }
    };
    buy_potions(town, party, 1);
    // Gear upgrades, best gain per gold first, while affordable.
    loop {
        let shelf: Vec<&Item> = town
            .vendor_stock
            .values()
            .flat_map(|s| s.iter().filter(|(_, &q)| q > 0).map(|(n, _)| n))
            .filter_map(|n| raws.items.iter().find(|i| &i.name == n))
            .collect();
        let best = shelf
            .into_iter()
            .filter_map(|item| {
                let (i, gain) = (0..party.heroes.len())
                    .filter_map(|i| Some((i, gear_gain(party.heroes[i].as_ref()?, item))))
                    .max_by_key(|x| x.1)?;
                let price = crate::economy::trade::buy_price(town, item.sector?, item.base_value);
                (gain > 0 && price <= party.gold).then_some((item, i, gain * 1000 / price.max(1)))
            })
            .max_by_key(|x| x.2);
        let Some((item, i, _)) = best else { break };
        let vendor = stocked_by(item.sector.unwrap(), item.tier).next().unwrap();
        if buy(town, &mut party.gold, vendor, item).is_err() {
            break;
        }
        wear(party.heroes[i].as_mut().unwrap(), item);
    }
    buy_potions(town, party, knob("POTIONS", POTIONS_PER_HERO as i32) as usize);
    for hero in party.heroes.iter_mut().flatten() {
        hero.stats.hp = hero.stats.max_hp;
    }
}

fn play_run(rng: &mut GameRng, raws: &Raws, pool: &[Vec<Level>], town: &mut TownState) -> RunLog {
    let mut log = RunLog::default();
    let mut party = Party::new(raws);
    restock(town, &raws.items);
    let (mut depth, mut deepest, mut days, mut trip, mut heart) = (1, 0, 0u32, 0u32, false);
    loop {
        let level = &pool[depth as usize][rng.0.random_range(0..POOL)];
        days += 1;
        trip += 1;
        deepest = deepest.max(depth);
        let before = party.alive();
        for hero in party.heroes.iter_mut().flatten() {
            hero.stats.hp = (hero.stats.hp + hero.stats.max_hp / 2).min(hero.stats.max_hp);
        }
        if heart {
            log.up[depth as usize].0 += 1;
        } else {
            let levels: Vec<i32> = party.heroes.iter().flatten().map(|h| h.level).collect();
            let d = &mut log.down[depth as usize];
            d.0 += 1;
            d.3 += levels.iter().sum::<i32>() as f64 / levels.len().max(1) as f64;
            let armour: Vec<i32> = party.heroes.iter().flatten().map(|h| h.armour()).collect();
            log.armour[depth as usize] += armour.iter().sum::<i32>() as f64 / armour.len().max(1) as f64;
        }
        let guard = play_level(rng, raws, &mut party, level, heart, &mut log);
        if depth == DEEPEST && !heart {
            log.guard = guard;
        }
        let lost = (before - party.alive()) as u32;
        party.bury();
        if !heart {
            log.down[depth as usize].2 += lost;
        }
        if party.alive() == 0 {
            break;
        }
        if heart {
            log.up[depth as usize].1 += 1;
        } else {
            log.down[depth as usize].1 += 1;
        }
        if depth == DEEPEST && !heart {
            heart = true;
        }
        if heart {
            depth -= 1;
            if depth == 0 {
                log.won = true;
                town_visit(raws, town, &mut party, days, &mut log);
                break;
            }
            continue;
        }
        if party.portals > 0 && (trip >= TRIP_LEVELS || lost > 0) {
            party.portals -= 1;
            town_visit(raws, town, &mut party, days, &mut log);
            days = 0;
            trip = 0;
            depth = deepest;
        } else {
            depth += 1;
        }
    }
    party.bury();
    log.dealt = party.dealt;
    log.deepest = deepest;
    log.town_level = town.level();
    log
}

/// `runs` runs one after another on one town; the first is a first-time party.
fn campaign(seed: u64, runs: usize, raws: &Raws, pool: &[Vec<Level>]) -> Vec<RunLog> {
    let mut rng = GameRng::seeded(seed);
    let mut town = TownState::fresh(seed);
    (0..runs).map(|_| play_run(&mut rng, raws, pool, &mut town)).collect()
}

// ============================================================================
// Per-fight numbers
// ============================================================================

/// Expected damage of one attack: the chance to hit, crits included, armour off.
fn expected(aim: i32, weapon: &Weapon, bonus: i32, defence: i32, armour: i32) -> (i32, f64) {
    let chance = hit_chance(aim, defence, weapon.optimal_range, weapon, 0, LEVEL_GROUND);
    let rolls = weapon.damage.0..=weapon.damage.1;
    let n = rolls.clone().count() as f64;
    let dmg = |crit: bool| {
        rolls.clone().map(|d| {
            let mut a = d + bonus;
            if crit {
                a += (a.max(0) + 1) / 2;
            }
            (a - armour).max(0) as f64
        }).sum::<f64>() / n
    };
    let c = weapon.crit_chance as f64 / 100.0;
    (chance, chance as f64 / 100.0 * ((1.0 - c) * dmg(false) + c * dmg(true)))
}

fn fight_table(raws: &Raws) -> String {
    let mut out = String::from("| Monster (depths) | HP | Fighter hit, dmg, attacks to kill | Ranger | Mage (Missile) | It hits Fighter, dmg | Mage | Hits to kill Fighter, Mage |\n|---|---|---|---|---|---|---|---|\n");
    let heroes: Vec<Hero> = CLASSES.iter().map(|c| Hero::new(raws, c, 1)).collect();
    for mob in raws.mobs.iter().filter(|m| m.ai.is_none()) {
        let depths: Vec<_> = raws.spawn_table.iter().filter(|e| e.name == mob.name).map(|e| format!("{}-{}", e.min_depth.max(1), e.max_depth.min(12))).collect();
        let foe = Foe::new(raws, &mob.name);
        let hp = foe.stats.max_hp as f64;
        let cell = |h: &Hero, spell: bool| {
            let (w, aim, bonus, armour) = if spell {
                let missile = h.powers.iter().find_map(|p| p.spell("damage").filter(|_| p.spell("area_of_effect").is_none())).unwrap_or(0);
                (Weapon { damage: spell_damage(missile), range: 6, optimal_range: 6, crit_chance: 0 }, h.stats.aim + SPELL_AIM_BONUS, 0, 0)
            } else {
                (h.weapon, h.stats.aim, h.stats.power, foe.stats.armour)
            };
            let (c, e) = expected(aim, &w, bonus, foe.stats.defence, armour);
            format!("{c}%, {e:.1}, {:.1}", hp / e.max(0.01))
        };
        let back = |h: &Hero| expected(foe.stats.aim, &foe.weapon, foe.stats.power, h.stats.defence, h.armour());
        let (fc, fe) = back(&heroes[0]);
        let (mc, me) = back(&heroes[2]);
        out += &format!(
            "| {} ({}) | {} | {} | {} | {} | {fc}%, {fe:.1} | {mc}%, {me:.1} | {:.1}, {:.1} |\n",
            mob.name,
            depths.join(", "),
            foe.stats.max_hp,
            cell(&heroes[0], false),
            cell(&heroes[1], false),
            cell(&heroes[2], true),
            heroes[0].stats.max_hp as f64 / fe.max(0.01),
            heroes[2].stats.max_hp as f64 / me.max(0.01),
        );
    }
    out
}

// ============================================================================
// The report
// ============================================================================

fn report_text(campaigns: u64, runs: usize) -> String {
    let pool = level_pool();
    let raws = RAWS.lock().unwrap().raws.clone();
    let mut out = String::new();
    // Spawns per depth.
    out += "| Depth | Groups | Hostiles | Hostiles in the way | Biggest group |\n|---|---|---|---|---|\n";
    for (depth, levels) in pool.iter().enumerate().skip(1) {
        let n = levels.len() as f64;
        let groups = levels.iter().map(|l| l.groups.len()).sum::<usize>() as f64 / n;
        let mobs = levels.iter().flat_map(|l| &l.groups).map(|g| g.mobs.len()).sum::<usize>() as f64 / n;
        let way = levels.iter().flat_map(|l| &l.groups).filter(|g| g.in_way).map(|g| g.mobs.len()).sum::<usize>() as f64 / n;
        let big = levels.iter().flat_map(|l| &l.groups).map(|g| g.mobs.len()).max().unwrap_or(0);
        out += &format!("| {depth} | {groups:.1} | {mobs:.1} | {way:.1} | {big} |\n");
    }
    out += "\n";
    out += &fight_table(&raws);

    let logs: Vec<Vec<RunLog>> = (0..campaigns).map(|seed| campaign(seed, runs, &raws, &pool)).collect();
    let summarise = |label: &str, picked: Vec<&RunLog>| -> String {
        let n = picked.len() as f64;
        let mut s = format!("\n{label} ({} runs)\n\n| Depth | Reach | Survive | Heroes lost | Mean level on arrival | Mean armour | Climb survive |\n|---|---|---|---|---|---|---|\n", picked.len());
        for d in 1..=12 {
            let (arr, surv, lost, lvl) = picked.iter().fold((0, 0, 0, 0.0), |a, l| {
                let x = l.down[d];
                (a.0 + x.0, a.1 + x.1, a.2 + x.2, a.3 + x.3)
            });
            let reached = picked.iter().filter(|l| l.down[d].0 > 0).count() as f64;
            let (ua, us) = picked.iter().fold((0, 0), |a, l| (a.0 + l.up[d].0, a.1 + l.up[d].1));
            let pct = |a: u32, b: u32| if b == 0 { "-".to_string() } else { format!("{:.0}%", 100.0 * a as f64 / b as f64) };
            s += &format!(
                "| {d} | {:.0}% | {} | {} | {} | {} | {} |\n",
                100.0 * reached / n,
                pct(surv, arr),
                if arr == 0 { "-".to_string() } else { format!("{:.2}", lost as f64 / arr as f64) },
                if arr == 0 { "-".to_string() } else { format!("{:.1}", lvl / arr as f64) },
                if arr == 0 { "-".to_string() } else { format!("{:.1}", picked.iter().map(|l| l.armour[d]).sum::<f64>() / arr as f64) },
                pct(us, ua),
            );
        }
        let guard: Vec<bool> = picked.iter().filter_map(|l| l.guard).collect();
        let wins = picked.iter().filter(|l| l.won).count() as f64;
        let gold = picked.iter().map(|l| l.gold + l.sold).sum::<i32>() as f64 / n;
        let sold = picked.iter().map(|l| l.sold).sum::<i32>() as f64 / n;
        let deepest = picked.iter().map(|l| l.deepest).sum::<i32>() as f64 / n;
        let dealt: Vec<i64> = (0..4).map(|c| picked.iter().map(|l| l.dealt[c]).sum()).collect();
        let total = dealt.iter().sum::<i64>().max(1) as f64;
        let shares: Vec<String> = (0..4).map(|c| format!("{} {:.0}%", CLASSES[c], 100.0 * dealt[c] as f64 / total)).collect();
        let (fallen, hired) = picked.iter().fold((0, 0), |a, l| (a.0 + l.fallen, a.1 + l.hired));
        s += &format!("\nShare of damage dealt and healed: {}. Fallen heroes found on a town visit: {fallen}, recruits hired: {hired}.", shares.join(", "));
        s += &format!(
            "\nDemon Lord guard beaten: {} of {} fights. Wins: {:.1}%. Mean deepest depth: {deepest:.1}. Gold per run: {gold:.0} ({sold:.0} from sales).\n",
            guard.iter().filter(|&&w| w).count(),
            guard.len(),
            100.0 * wins / n
        );
        s
    };
    out += &summarise("First run, fresh town", logs.iter().map(|c| &c[0]).collect());
    out += &summarise(&format!("Runs 6 to {runs}, the same town"), logs.iter().flat_map(|c| &c[5..]).collect());

    // Town levels by run.
    let first: Vec<usize> = logs.iter().map(|c| c.iter().position(|l| l.town_level > 1).map_or(runs + 1, |r| r + 1)).collect();
    let by = |r: usize| 100.0 * first.iter().filter(|&&f| f <= r).count() as f64 / first.len() as f64;
    out += &format!("\nFirst sector level-up: by run 1 {:.0}%, by run 2 {:.0}%, by run 3 {:.0}%, by run 5 {:.0}%.\n", by(1), by(2), by(3), by(5));
    out += "\n| Run | Mean town level after | Campaigns with a sector levelled |\n|---|---|---|\n";
    for r in 0..runs {
        let levels: Vec<u32> = logs.iter().map(|c| c[r].town_level).collect();
        let mean = levels.iter().sum::<u32>() as f64 / levels.len() as f64;
        let up = levels.iter().filter(|&&l| l > 1).count() as f64 / levels.len() as f64;
        out += &format!("| {} | {mean:.2} | {:.0}% |\n", r + 1, 100.0 * up);
    }
    out
}

#[test]
#[ignore = "diagnostics"]
fn hostiles_per_level() {
    load_raws();
    for depth in [1, 6, 12] {
        let mut counts: Vec<usize> = (0..40).map(|i| generate(depth, 1000 * depth as u64 + i).groups.iter().map(|g| g.mobs.len()).sum()).collect();
        counts.sort();
        println!("depth {depth}: {counts:?}");
    }
}

/// The staged harness fights, simulated: (party wins, mean heroes lost, mean rounds).
fn staged(raws: &Raws, mobs: &[&str], trials: u64) -> (f64, f64, f64) {
    let (mut wins, mut lost, mut rounds) = (0, 0, 0);
    for seed in 0..trials {
        let mut rng = GameRng::seeded(seed);
        let mut heroes: Vec<Option<Hero>> = CLASSES.iter().map(|c| Some(Hero::new(raws, c, 1))).collect();
        let foes = mobs.iter().map(|m| Foe::new(raws, m)).collect();
        let result = fight(&mut rng, &mut heroes, &mut Vec::new(), foes, true);
        wins += u32::from(result.won);
        lost += heroes.iter().flatten().filter(|h| !h.alive()).count();
        rounds += result.rounds;
    }
    let n = trials as f64;
    (wins as f64 / n, lost as f64 / n, rounds as f64 / n)
}

#[test]
#[ignore = "the staged fights for the harness check; run by name"]
fn staged_fights() {
    load_raws();
    let raws = RAWS.lock().unwrap().raws.clone();
    for mobs in [&["Demon Lord"][..], &["Hobgoblin", "Orc"]] {
        let (won, lost, rounds) = staged(&raws, mobs, 2000);
        println!("{mobs:?}: party wins {:.0}%, heroes lost {lost:.2}, rounds {rounds:.1}", 100.0 * won);
    }
}

#[test]
#[ignore = "the full balance report takes a minute; run it by name"]
fn report() {
    let campaigns = std::env::var("BALANCE_CAMPAIGNS").ok().and_then(|v| v.parse().ok()).unwrap_or(200);
    println!("{}", report_text(campaigns, 10));
}

#[test]
fn a_seeded_campaign_plays_the_same_twice() {
    load_raws();
    let raws = RAWS.lock().unwrap().raws.clone();
    let pool: Vec<Vec<Level>> = (0..=DEEPEST).map(|d| if d == 0 { Vec::new() } else { vec![generate(d, d as u64); POOL] }).collect();
    let a = campaign(7, 2, &raws, &pool);
    let b = campaign(7, 2, &raws, &pool);
    assert_eq!(format!("{a:?}"), format!("{b:?}"));
    assert!(a[0].down[1].0 == 1, "every run starts on depth 1");
}

/// A tuning knob from the environment, to try a change before it goes into
/// the raws: `POWER_ADD`, `POWER_DEPTH10` (power per 10 depths of a monster's
/// first depth), `HP_PCT`, `HP_DEPTH`, `XP_PCT`, `GEAR_PCT` (share of found gear
/// kept), `REST_PCT` (healed between fights) and `POTIONS` (bought per hero).
fn knob(name: &str, default: i32) -> i32 {
    std::env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}
