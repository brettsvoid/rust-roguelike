//! Classes and their abilities.
//!
//! A class (raws `classes`) gives a new hero starting stats, gear and up to
//! two abilities (raws `abilities`), spawned by `raws::spawn_class_kit`. The
//! hero carries `HeroClass`; each ability is a child entity of the hero with
//! `HeroAbility` and its `Name`. A spell ability also carries the scroll
//! components of its effects (`Ranged`, `InflictsDamage`, `AreaOfEffect`,
//! `ProvidesHealing`, `CausesConfusion`, `Targeting`), so using it is a
//! `WantsToUseItem` naming the ability entity: the targeting screen aims it
//! and `inventory::item_use_system` casts it as it would a scroll, without
//! using anything up. The weapon abilities (Cleave, Aimed Shot) resolve in
//! `ability_system` through `combat::Strikes::strike`, like any attack.
//!
//! The cooldown is the one limit: after use an ability rests `cooldown`
//! rounds, counted down at the start of the heroes' turn (`tick_cooldowns`,
//! after the enemy phase; exploring, every turn). Used in round 1 with a
//! cooldown of 3, it is ready again in round 4.
//!
//! The rules (an ability's target and AP cost, whether it can be used now,
//! whether it has a target, who a cleave hits) are pure functions below.

use bevy::prelude::*;

use crate::bystander::Bystander;
use crate::combat::{chebyshev, in_reach, Arms, CombatStats, Fighter, Strikes};
use crate::components::{Name, WantsToUseItem, Weapon};
use crate::distance::DistanceAlg;
use crate::game_flow::HeroAction;
use crate::map::{Map, Position};
use crate::monsters::Monster;
use crate::party::{ActiveUnit, PartyMember};
use crate::raws::{self, AbilityEffect};
use crate::tactics::Action;
use crate::viewshed::Viewshed;

/// Abilities a hero can have: one per hotkey (1 and 2).
pub const ABILITY_SLOTS: usize = 2;

/// A hero's class, by its raws name. Shown on the HUD card and in the
/// inventory title, and saved (its abilities are spawned afresh on load).
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct HeroClass(pub String);

/// One of a hero's abilities, on a child entity of the hero.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct HeroAbility {
    /// The hotkey and button it sits on, from 0.
    pub slot: usize,
    pub effect: AbilityEffect,
    /// Rounds it rests after use.
    pub cooldown: u8,
    /// Rounds until it is ready; 0 when ready.
    pub left: u8,
}

/// What an ability is aimed at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AbilityTarget {
    /// Every hostile beside the hero (Cleave): nothing to pick.
    Adjacent,
    /// The hero themself: nothing to pick.
    Own,
    /// A hostile in the reach of the wielded weapon (Aimed Shot).
    WeaponReach,
    /// A hostile within `range` tiles.
    Hostile { range: i32 },
    /// A hero within `range` tiles, the caster included.
    Ally { range: i32 },
    /// Any tile within `range`; the effect covers `radius` round it.
    Tile { range: i32, radius: i32 },
}

impl AbilityTarget {
    /// The range of the targeting screen, or `None` when there is nothing to pick.
    pub fn range(self, weapon: &Weapon) -> Option<i32> {
        match self {
            AbilityTarget::Adjacent | AbilityTarget::Own => None,
            AbilityTarget::WeaponReach => Some(weapon.range),
            AbilityTarget::Hostile { range } | AbilityTarget::Ally { range } | AbilityTarget::Tile { range, .. } => {
                Some(range)
            }
        }
    }
}

impl HeroAbility {
    pub fn new(slot: usize, ability: &raws::Ability) -> Self {
        HeroAbility { slot, effect: ability.effect.clone(), cooldown: ability.cooldown, left: 0 }
    }

    pub fn target(&self) -> AbilityTarget {
        let effects = match &self.effect {
            AbilityEffect::Cleave => return AbilityTarget::Adjacent,
            AbilityEffect::AimedShot { .. } => return AbilityTarget::WeaponReach,
            AbilityEffect::Spell(effects) => effects,
        };
        let number = |key: &str| effects.get(key).and_then(|v| v.parse().ok());
        match (number("ranged"), number("area_of_effect")) {
            (None, _) => AbilityTarget::Own,
            (Some(range), Some(radius)) => AbilityTarget::Tile { range, radius },
            (Some(range), None) if effects.get("targeting").is_some_and(|t| t == "ally") => {
                AbilityTarget::Ally { range }
            }
            (Some(range), None) => AbilityTarget::Hostile { range },
        }
    }

    /// The AP rule it spends under: one aimed at a hostile ends the turn like
    /// an attack; one on the party (a heal) costs 1 AP like an item.
    pub fn action(&self) -> Action {
        match self.target() {
            AbilityTarget::Own | AbilityTarget::Ally { .. } => Action::UseItem,
            _ => Action::Attack,
        }
    }

    pub fn aim_bonus(&self) -> i32 {
        match self.effect {
            AbilityEffect::AimedShot { aim_bonus } => aim_bonus,
            _ => 0,
        }
    }

    /// Used: it rests its full cooldown.
    pub fn start_cooldown(&mut self) {
        self.left = self.cooldown;
    }

    /// The start of the hero's turn: a round less to wait.
    pub fn tick(&mut self) {
        self.left = self.left.saturating_sub(1);
    }

    pub fn status(&self, has_target: bool) -> AbilityStatus {
        AbilityStatus { action: self.action(), left: self.left, has_target }
    }
}

/// What deciding whether an ability can be used needs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AbilityStatus {
    pub action: Action,
    /// Rounds of cooldown left.
    pub left: u8,
    /// Something to use it on (`has_target`).
    pub has_target: bool,
}

/// Why an ability cannot be used now, or `None` if it can. Attacks are for
/// combat; exploring, AP never run out.
pub fn refusal(combat: bool, ap_left: u8, status: AbilityStatus) -> Option<&'static str> {
    if !combat && status.action == Action::Attack {
        Some("Combat only")
    } else if status.left > 0 {
        Some("Cooldown")
    } else if combat && status.action.spend(ap_left).is_none() {
        Some(if ap_left == 0 { "No AP" } else { "Needs 1 AP" })
    } else if !status.has_target {
        Some("No target")
    } else {
        None
    }
}

/// The targeting screen's distance rule: straight-line tiles.
fn within(from: (i32, i32), to: (i32, i32), range: i32) -> bool {
    let point = |(x, y): (i32, i32)| Vec2::new(x as f32, y as f32);
    DistanceAlg::Euclidean.distance2d(point(from), point(to)) <= range as f32
}

/// Whether a hero at `at` holding `weapon` has something to use the ability
/// on. `hostiles` are where the hostiles the party sees stand, `wounded`
/// where the living heroes below full health stand.
pub fn has_target(
    map: &Map,
    target: AbilityTarget,
    weapon: &Weapon,
    at: (i32, i32),
    hostiles: &[(i32, i32)],
    wounded: &[(i32, i32)],
) -> bool {
    match target {
        AbilityTarget::Adjacent => hostiles.iter().any(|&h| chebyshev(at, h) == 1),
        AbilityTarget::Own => wounded.contains(&at),
        AbilityTarget::WeaponReach => hostiles.iter().any(|&h| in_reach(map, weapon, at, h).is_ok()),
        AbilityTarget::Hostile { range } | AbilityTarget::Tile { range, .. } => {
            hostiles.iter().any(|&h| within(at, h, range))
        }
        AbilityTarget::Ally { range } => wounded.iter().any(|&w| within(at, w, range)),
    }
}

/// The hostiles a cleave from `at` hits: every one beside it, in the order given.
pub fn cleave_targets<T: Copy>(at: (i32, i32), hostiles: &[(T, (i32, i32))]) -> Vec<T> {
    hostiles.iter().filter(|h| chebyshev(at, h.1) == 1).map(|h| h.0).collect()
}

/// Hit points after healing `amount`, never above `max_hp`.
pub fn healed(hp: i32, max_hp: i32, amount: i32) -> i32 {
    (hp + amount).min(max_hp).max(hp)
}

/// One of the active hero's abilities as it stands this frame.
#[derive(Clone, Copy, Debug)]
pub struct ReadyAbility {
    pub entity: Entity,
    pub target: AbilityTarget,
    pub status: AbilityStatus,
}

/// The active hero's abilities by slot, for the action bar and the hotkeys,
/// so both decide from the same facts.
#[derive(Resource, Default, Debug)]
pub struct ActiveAbilities(pub [Option<ReadyAbility>; ABILITY_SLOTS]);

pub struct AbilitiesPlugin;

impl Plugin for AbilitiesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ActiveAbilities>()
            .add_systems(Update, update_active_abilities.before(crate::player::handle_player_input));
    }
}

pub fn update_active_abilities(
    mut ready: ResMut<ActiveAbilities>,
    active: Res<ActiveUnit>,
    map: Res<Map>,
    arms: Arms,
    abilities: Query<(Entity, &ChildOf, &HeroAbility)>,
    heroes: Query<(&Position, &CombatStats, &Viewshed), With<PartyMember>>,
    monsters: Query<(&Position, &CombatStats), (With<Monster>, Without<Bystander>)>,
) {
    let mut now = [None; ABILITY_SLOTS];
    if let Some((hero, pos)) = active.0.and_then(|unit| Some((unit, heroes.get(unit).ok()?.0))) {
        let seen = |p: &Position| heroes.iter().any(|(.., view)| view.visible_tiles.contains(&(p.x, p.y)));
        let hostiles: Vec<_> =
            monsters.iter().filter(|(p, s)| s.hp > 0 && seen(p)).map(|(p, _)| (p.x, p.y)).collect();
        let wounded: Vec<_> = heroes
            .iter()
            .filter(|(_, s, _)| s.hp > 0 && s.hp < s.max_hp)
            .map(|(p, ..)| (p.x, p.y))
            .collect();
        let weapon = arms.weapon(hero);
        for (entity, parent, ability) in &abilities {
            if parent.parent() != hero || ability.slot >= ABILITY_SLOTS {
                continue;
            }
            let target = ability.target();
            let has_target = has_target(&map, target, &weapon, (pos.x, pos.y), &hostiles, &wounded);
            now[ability.slot] = Some(ReadyAbility { entity, target, status: ability.status(has_target) });
        }
    }
    ready.0 = now;
}

/// A hero uses an ability (a `WantsToUseItem` naming it): its cooldown
/// starts and its AP rule is recorded. Cleave and Aimed Shot resolve here
/// through `Strikes::strike`; spells are left to `inventory::item_use_system`.
pub fn ability_system(
    mut strikes: Strikes,
    map: Res<Map>,
    mut hero_action: ResMut<HeroAction>,
    users: Query<(Entity, &WantsToUseItem, &Name, &CombatStats, &Position)>,
    mut abilities: Query<&mut HeroAbility>,
    foes: Query<(Entity, &Name, &CombatStats, &Position), (With<Monster>, Without<Bystander>, Without<PartyMember>)>,
) {
    for (hero, wants, name, stats, pos) in &users {
        let Ok(mut ability) = abilities.get_mut(wants.item) else {
            continue;
        };
        ability.start_cooldown();
        hero_action.0 = Some(ability.action());
        let at = (pos.x, pos.y);
        let living: Vec<_> = foes.iter().filter(|f| f.2.hp > 0).map(|f| (f, (f.3.x, f.3.y))).collect();
        let targets = match ability.effect {
            AbilityEffect::Spell(_) => continue,
            AbilityEffect::Cleave => {
                strikes.log.entries.push(format!("{} cleaves.", name.name));
                cleave_targets(at, &living)
            }
            AbilityEffect::AimedShot { .. } => {
                strikes.log.entries.push(format!("{} takes an aimed shot.", name.name));
                living.iter().filter(|f| Some(f.1) == wants.target).map(|f| f.0).collect()
            }
        };
        strikes.commands.entity(hero).remove::<WantsToUseItem>();
        let aimed = CombatStats { aim: stats.aim + ability.aim_bonus(), ..stats.clone() };
        let attacker = Fighter { entity: hero, name: &name.name, stats: &aimed, at };
        for (foe, foe_name, foe_stats, foe_pos) in targets {
            let target = Fighter { entity: foe, name: &foe_name.name, stats: foe_stats, at: (foe_pos.x, foe_pos.y) };
            strikes.strike(&map, &attacker, &target, false);
        }
    }
}

/// The start of the heroes' turn: every ability's cooldown comes down a round.
pub fn tick_cooldowns(mut abilities: Query<&mut HeroAbility>) {
    for mut ability in &mut abilities {
        if ability.left > 0 {
            ability.tick();
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy::ecs::system::RunSystemOnce;

    use super::*;
    use crate::combat::{hit_chance, SufferDamage, LEVEL_GROUND};
    use crate::debug::GodMode;
    use crate::effects::Effects;
    use crate::gamelog::GameLog;
    use crate::map::TileType;
    use crate::party::HEROES;
    use crate::rng::GameRng;

    fn shipped() -> raws::Raws {
        serde_json::from_str(include_str!("../raws/spawns.json")).unwrap()
    }

    fn ability(name: &str) -> HeroAbility {
        let raws = shipped();
        HeroAbility::new(0, raws.abilities.iter().find(|a| a.name == name).unwrap())
    }

    #[test]
    fn each_hero_has_a_class_whose_gear_and_abilities_exist() {
        let raws = shipped();
        let kits: Vec<(&str, &str, Vec<&str>)> = HEROES
            .iter()
            .map(|&(hero, _, class)| {
                let class = raws.classes.iter().find(|c| c.name == class).unwrap();
                for item in &class.gear {
                    assert!(raws.items.iter().any(|i| &i.name == item), "{item}");
                }
                for name in &class.abilities {
                    assert!(raws.abilities.iter().any(|a| &a.name == name), "{name}");
                }
                (hero, class.name.as_str(), class.abilities.iter().map(String::as_str).collect())
            })
            .collect();
        assert_eq!(
            kits,
            vec![
                ("Aldric", "Fighter", vec!["Cleave", "Second Wind"]),
                ("Brenna", "Ranger", vec!["Aimed Shot", "Snare"]),
                ("Corin", "Mage", vec!["Fireball", "Magic Missile"]),
                ("Dagny", "Cleric", vec!["Heal", "Smite"]),
            ]
        );
    }

    #[test]
    fn each_ability_has_its_target_ap_rule_and_cooldown() {
        use AbilityTarget::*;
        let raws = shipped();
        let table: Vec<_> = raws
            .abilities
            .iter()
            .map(|a| {
                let ability = HeroAbility::new(0, a);
                (a.name.as_str(), ability.target(), ability.action(), ability.cooldown)
            })
            .collect();
        assert_eq!(
            table,
            vec![
                ("Cleave", Adjacent, Action::Attack, 3),
                ("Second Wind", Own, Action::UseItem, 5),
                ("Aimed Shot", WeaponReach, Action::Attack, 3),
                ("Snare", Hostile { range: 6 }, Action::Attack, 4),
                ("Fireball", Tile { range: 6, radius: 2 }, Action::Attack, 4),
                ("Magic Missile", Hostile { range: 6 }, Action::Attack, 2),
                ("Heal", Ally { range: 4 }, Action::UseItem, 3),
                ("Smite", Hostile { range: 5 }, Action::Attack, 2),
            ]
        );
    }

    fn status(action: Action, left: u8, has_target: bool) -> AbilityStatus {
        AbilityStatus { action, left, has_target }
    }

    #[test]
    fn an_ability_needs_combat_a_rested_cooldown_ap_and_a_target() {
        let cases = [
            // (combat, ap left, status, refusal)
            (true, 2, status(Action::Attack, 0, true), None),
            (true, 1, status(Action::Attack, 0, true), None),
            (true, 0, status(Action::Attack, 0, true), Some("No AP")),
            (true, 2, status(Action::Attack, 2, true), Some("Cooldown")),
            (true, 0, status(Action::Attack, 1, true), Some("Cooldown")),
            (true, 2, status(Action::Attack, 0, false), Some("No target")),
            (false, 2, status(Action::Attack, 0, true), Some("Combat only")),
            // A heal works exploring too.
            (false, 2, status(Action::UseItem, 0, true), None),
            (false, 2, status(Action::UseItem, 3, true), Some("Cooldown")),
            (true, 1, status(Action::UseItem, 0, true), None),
            (true, 1, status(Action::UseItem, 0, false), Some("No target")),
        ];
        for (combat, ap, status, expected) in cases {
            assert_eq!(refusal(combat, ap, status), expected, "{combat} {ap} {status:?}");
        }
    }

    #[test]
    fn a_used_ability_is_ready_again_after_its_cooldown_in_rounds() {
        let mut cleave = ability("Cleave");
        cleave.start_cooldown();
        let mut ready_in = Vec::new();
        for round in 2..=5 {
            cleave.tick(); // the start of round `round`
            ready_in.push((round, refusal(true, 2, cleave.status(true))));
        }
        assert_eq!(ready_in, vec![(2, Some("Cooldown")), (3, Some("Cooldown")), (4, None), (5, None)]);
    }

    /// A 9 by 9 room of floor inside a wall ring, with a wall at (4, 2).
    fn room() -> Map {
        let mut map = Map::new(9, 9, 1);
        for y in 1..8 {
            for x in 1..8 {
                let idx = map.xy_idx(x, y);
                map.tiles[idx] = TileType::Floor;
            }
        }
        let idx = map.xy_idx(4, 2);
        map.tiles[idx] = TileType::Wall;
        map
    }

    const SWORD: Weapon = Weapon { damage: (3, 5), range: 1, optimal_range: 1, crit_chance: 10 };
    const BOW: Weapon = Weapon { damage: (1, 4), range: 8, optimal_range: 5, crit_chance: 10 };

    #[test]
    fn each_target_kind_finds_what_it_can_act_on() {
        use AbilityTarget::*;
        let map = room();
        let at = (4, 4);
        let target = |kind, weapon: &Weapon, hostiles: &[(i32, i32)], wounded: &[(i32, i32)]| {
            has_target(&map, kind, weapon, at, hostiles, wounded)
        };
        assert!(target(Adjacent, &SWORD, &[(5, 5)], &[]));
        assert!(!target(Adjacent, &SWORD, &[(6, 4)], &[]));
        assert!(target(WeaponReach, &BOW, &[(7, 7)], &[]));
        // The wall at (4, 2) blocks the line to (4, 1).
        assert!(!target(WeaponReach, &BOW, &[(4, 1)], &[]));
        assert!(!target(WeaponReach, &SWORD, &[(6, 4)], &[]));
        assert!(target(Hostile { range: 3 }, &SWORD, &[(7, 4)], &[]));
        // Straight-line range, as the targeting screen draws it: (7, 7) is 4.24 away.
        assert!(!target(Hostile { range: 4 }, &SWORD, &[(7, 7)], &[]));
        assert!(target(Tile { range: 6, radius: 2 }, &SWORD, &[(7, 7)], &[]));
        assert!(!target(Tile { range: 6, radius: 2 }, &SWORD, &[], &[]));
        assert!(target(Ally { range: 4 }, &SWORD, &[], &[(1, 4)]));
        assert!(!target(Ally { range: 2 }, &SWORD, &[], &[(1, 4)]));
        assert!(target(Own, &SWORD, &[], &[at]));
        assert!(!target(Own, &SWORD, &[], &[(1, 4)]));
    }

    #[test]
    fn a_cleave_hits_every_hostile_beside_the_hero() {
        let foes = [('a', (5, 5)), ('b', (6, 4)), ('c', (3, 4)), ('d', (4, 3))];
        assert_eq!(cleave_targets((4, 4), &foes), vec!['a', 'c', 'd']);
        assert_eq!(cleave_targets((1, 1), &foes), Vec::<char>::new());
    }

    #[test]
    fn healing_stops_at_full_health() {
        assert_eq!(healed(12, 30, 10), 22);
        assert_eq!(healed(25, 30, 10), 30);
        assert_eq!(healed(30, 30, 10), 30);
    }

    #[test]
    fn an_aimed_shot_adds_its_bonus_before_the_range_falloff() {
        let aimed = ability("Aimed Shot");
        // A ranger (aim 75) at 7 tiles with the Shortbow: 20 lost past the optimal 5.
        let plain = hit_chance(75, 0, 7, &BOW, 0, LEVEL_GROUND);
        let with_bonus = hit_chance(75 + aimed.aim_bonus(), 0, 7, &BOW, 0, LEVEL_GROUND);
        assert_eq!((plain, with_bonus), (55, 75));
    }

    fn unit(world: &mut World, name: &str, at: (i32, i32), aim: i32) -> Entity {
        world
            .spawn((
                Name { name: name.into() },
                Position { x: at.0, y: at.1 },
                CombatStats { max_hp: 16, hp: 16, armour: 1, power: 3, movement: 4, aim, defence: 0 },
            ))
            .id()
    }

    fn world() -> World {
        let mut world = World::new();
        world.insert_resource(room());
        world.insert_resource(GameLog { entries: Vec::new() });
        world.insert_resource(Effects::default());
        world.insert_resource(GameRng::seeded(42));
        world.insert_resource(GodMode::default());
        world.insert_resource(HeroAction::default());
        world.init_resource::<crate::presentation::Presentation>();
        world
    }

    /// Uses `name` from a hero at (4, 4) on `target` and returns the log.
    fn use_ability(world: &mut World, name: &str, target: Option<(i32, i32)>) -> (Entity, Vec<String>) {
        let hero = unit(world, "Aldric", (4, 4), 65);
        world.entity_mut(hero).insert(PartyMember { slot: 0 });
        let ability = world.spawn((ability(name), ChildOf(hero))).id();
        world.entity_mut(hero).insert(WantsToUseItem { item: ability, target });
        world.run_system_once(ability_system).unwrap();
        let log = world.resource::<GameLog>().entries.clone();
        (ability, log)
    }

    fn damage(world: &World, unit: Entity) -> Option<Vec<i32>> {
        world.get::<SufferDamage>(unit).map(|d| d.amount.clone())
    }

    #[test]
    fn a_seeded_cleave_rolls_each_adjacent_hostile_separately() {
        let mut world = world();
        let left = unit(&mut world, "Goblin #0", (3, 4), 65);
        let below = unit(&mut world, "Goblin #1", (5, 5), 65);
        let far = unit(&mut world, "Goblin #2", (7, 4), 65);
        for goblin in [left, below, far] {
            world.entity_mut(goblin).insert(Monster);
        }
        let (cleave, log) = use_ability(&mut world, "Cleave", None);
        // Natural weapon (-1..1) + power 3, less armour 1, at 65%.
        assert_eq!(
            log,
            vec![
                "Aldric cleaves.",
                "Aldric attacks Goblin #0: hit for 1 (65%)",
                "Aldric attacks Goblin #1: hit for 2 (65%)",
            ]
        );
        assert_eq!((damage(&world, left), damage(&world, below), damage(&world, far)), (Some(vec![1]), Some(vec![2]), None));
        assert_eq!(world.get::<HeroAbility>(cleave).unwrap().left, 3);
        assert_eq!(world.resource::<HeroAction>().0, Some(Action::Attack));
    }

    #[test]
    fn a_seeded_aimed_shot_rolls_with_the_bonus() {
        let mut world = world();
        let goblin = unit(&mut world, "Goblin", (5, 4), 65);
        world.entity_mut(goblin).insert(Monster);
        let (_, log) = use_ability(&mut world, "Aimed Shot", Some((5, 4)));
        // 65 aim + 20, natural weapon (-1..1) + power 3, less armour 1.
        assert_eq!(log, vec!["Aldric takes an aimed shot.", "Aldric attacks Goblin: hit for 1 (85%)"]);
    }

    #[test]
    fn a_spell_ability_is_left_to_the_item_code_with_its_cooldown_started() {
        let mut world = world();
        let (heal, log) = use_ability(&mut world, "Heal", Some((4, 4)));
        assert!(log.is_empty());
        assert_eq!(world.get::<HeroAbility>(heal).unwrap().left, 3);
        assert_eq!(world.resource::<HeroAction>().0, Some(Action::UseItem));
        let hero = world.query_filtered::<Entity, With<PartyMember>>().single(&world).unwrap();
        assert!(world.get::<WantsToUseItem>(hero).is_some());
    }
}
