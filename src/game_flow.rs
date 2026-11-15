//! The state machine that drives the game.
//!
//! Two pieces of state work together:
//! - `RunState` is what the frame loop is doing: a menu, an overlay, waiting
//!   for the party's input, resolving a party action (`PartyAction`) or letting
//!   the monsters act (`EnemyTurn`).
//! - `Mode` is the tactical mode: `Explore` or `Combat`. Menus and overlays
//!   leave it alone, so cancelling the inventory returns to the same mode.
//!
//! Every move between modes and phases goes through `advance`, a pure function
//! over `Mode`, the `Step` just finished and a plain-data `Situation`. A turn
//! cycles `AwaitingInput -> PartyAction -> EnemyTurn -> AwaitingInput` in
//! explore (each step costs a turn). In combat, every hero gets
//! `AP_PER_ROUND` action points a round; `PartyAction` returns to
//! `AwaitingInput` until no living hero has AP left or the turn is ended, and
//! only then do the monsters act (the enemy phase).

use std::collections::HashMap;

use bevy::prelude::*;

use crate::combat::CombatStats;
use crate::map::{self, Map, Position, Revealed, RevealedState, Tile};
use crate::monsters::Monster;
use crate::party::{next_unit, ActiveUnit, PartyMember};
use crate::player::WalkPath;
use crate::tactics::{Action, AP_PER_ROUND};
use crate::viewshed::{self, Viewshed};
use crate::{combat, hunger, inventory, levels, monsters, traps};

#[derive(States, Clone, Copy, Default, Eq, PartialEq, Debug, Hash)]
pub enum RunState {
    #[default]
    MainMenu,
    MapBuilderSelect,
    MapGeneration,
    PreRun,
    AwaitingInput,
    PartyAction,
    EnemyTurn,
    ShowInventory,
    ShowDropItem,
    ShowRemoveItem,
    ShowTargeting,
    ShowLedger,
    ShowShop,
    NextLevel,
    MagicMapReveal,
    GameOver,
}

/// The tactical mode. Not saved: a loaded game starts in explore, and the
/// next step drops back into combat if a hostile still sees the party.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    /// A click moves the whole party; every step is a turn.
    #[default]
    Explore,
    /// Rounds of a party phase then an enemy phase. `ap_spent` holds the
    /// action points each hero has spent this round; a missing hero has spent
    /// none.
    Combat { round: u32, ap_spent: HashMap<Entity, u8> },
}

impl Mode {
    /// The action points `unit` has left this round. Exploring, the budget
    /// never runs out.
    pub fn ap_left(&self, unit: Entity) -> u8 {
        match self {
            Mode::Explore => AP_PER_ROUND,
            Mode::Combat { ap_spent, .. } => AP_PER_ROUND.saturating_sub(ap_spent.get(&unit).copied().unwrap_or(0)),
        }
    }

    /// Whether `unit` may still act.
    pub fn can_act(&self, unit: Entity) -> bool {
        self.ap_left(unit) > 0
    }
}

/// What just finished, to move the state machine on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// A hero took an action.
    PartyActed(Entity, Action),
    /// The player ended the party phase (E).
    EndTurn,
    /// The monsters took their turn.
    EnemiesActed,
}

/// What one hostile and the party can see of each other.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Sight {
    /// It sees a living hero: it is aware of the party.
    pub sees_party: bool,
    /// A living hero sees it.
    pub seen_by_party: bool,
}

/// The plain facts `advance` decides from.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Situation {
    /// The living heroes.
    pub party: Vec<Entity>,
    /// One entry per living hostile.
    pub hostiles: Vec<Sight>,
}

/// The mode and the frame-loop state after `step`. Combat starts when a
/// hostile and a hero see each other, always with the party phase of round 1.
/// It ends when no hostile is aware of the party. A party with nobody left
/// alive is game over.
pub fn advance(mode: &Mode, step: Step, situation: &Situation) -> (Mode, RunState) {
    if situation.party.is_empty() {
        return (mode.clone(), RunState::GameOver);
    }
    let aware = situation.hostiles.iter().any(|h| h.sees_party);
    let contact = situation.hostiles.iter().any(|h| h.sees_party && h.seen_by_party);
    match (mode, step) {
        (Mode::Explore, _) if contact => (Mode::Combat { round: 1, ap_spent: HashMap::new() }, RunState::AwaitingInput),
        (Mode::Explore, Step::PartyActed(..)) => (Mode::Explore, RunState::EnemyTurn),
        (Mode::Explore, Step::EndTurn | Step::EnemiesActed) => (Mode::Explore, RunState::AwaitingInput),
        (Mode::Combat { .. }, _) if !aware => (Mode::Explore, RunState::AwaitingInput),
        (Mode::Combat { round, ap_spent }, Step::PartyActed(unit, action)) => {
            // The input only offers what the hero can afford; anything else spends the rest.
            let left = action.spend(mode.ap_left(unit)).unwrap_or(0);
            let mut ap_spent = ap_spent.clone();
            ap_spent.insert(unit, AP_PER_ROUND - left);
            let mode = Mode::Combat { round: *round, ap_spent };
            let phase_over = situation.party.iter().all(|&hero| !mode.can_act(hero));
            (mode, if phase_over { RunState::EnemyTurn } else { RunState::AwaitingInput })
        }
        (Mode::Combat { .. }, Step::EndTurn) => (mode.clone(), RunState::EnemyTurn),
        (Mode::Combat { round, .. }, Step::EnemiesActed) => {
            (Mode::Combat { round: round + 1, ap_spent: HashMap::new() }, RunState::AwaitingInput)
        }
    }
}

/// Set when a scroll of magic mapping is read; picked up at end of the party action.
#[derive(Resource, Default)]
pub struct PendingMagicMap(pub bool);

/// The magic map animation: the row revealed so far and where play goes after.
#[derive(Resource)]
pub struct MagicMapReveal {
    row: i32,
    then: RunState,
}

impl Default for MagicMapReveal {
    fn default() -> Self {
        Self { row: 0, then: RunState::AwaitingInput }
    }
}

/// Set by the end turn key so the party action that follows ends the phase.
#[derive(Resource, Default)]
pub struct TurnEnded(pub bool);

/// The action the active hero just took, set by the map input. The inventory
/// and targeting screens leave it unset: those are item actions.
#[derive(Resource, Default)]
pub struct HeroAction(pub Option<Action>);

pub struct GameFlowPlugin;

impl Plugin for GameFlowPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<RunState>()
            .init_resource::<Mode>()
            .init_resource::<PendingMagicMap>()
            .init_resource::<MagicMapReveal>()
            .init_resource::<TurnEnded>()
            .init_resource::<HeroAction>()
            // PreRun: one frame of setup, then hand control to the party
            .add_systems(OnEnter(RunState::PreRun), |mut mode: ResMut<Mode>| *mode = Mode::Explore)
            .add_systems(
                Update,
                transition_to_awaiting_input.run_if(in_state(RunState::PreRun)),
            )
            // PartyAction: resolve the hero's action, then move the mode on
            .add_systems(
                Update,
                (
                    traps::trap_trigger_system,
                    inventory::item_collection_system,
                    inventory::item_use_system,
                    inventory::item_drop_system,
                    inventory::item_remove_system,
                    combat::melee_combat_system,
                    combat::damage_system,
                    combat::delete_the_dead,
                    viewshed::update_viewshed,
                    after_party_action,
                )
                    .chain()
                    .run_if(in_state(RunState::PartyAction)),
            )
            // EnemyTurn: run monster AI, then move the mode on
            .add_systems(
                Update,
                (
                    // Each side's status effects bite at the start of its turn.
                    combat::start_of_turn::<With<Monster>>,
                    combat::damage_system,
                    monsters::monster_ai,
                    traps::trap_trigger_system,
                    combat::melee_combat_system,
                    hunger::hunger_system,
                    combat::start_of_turn::<With<PartyMember>>,
                    combat::damage_system,
                    combat::delete_the_dead,
                    viewshed::update_viewshed,
                    after_enemy_turn,
                )
                    .chain()
                    .run_if(in_state(RunState::EnemyTurn)),
            )
            // NextLevel: tear down the old level and build the next one
            .add_systems(
                Update,
                levels::go_next_level.run_if(in_state(RunState::NextLevel)),
            )
            // MagicMapReveal: uncover the map one row per frame
            .add_systems(
                Update,
                magic_map_reveal.run_if(in_state(RunState::MagicMapReveal)),
            );
    }
}

fn transition_to_awaiting_input(mut next_state: ResMut<NextState<RunState>>) {
    next_state.set_if_different(RunState::AwaitingInput);
}

type PartySight<'w, 's> =
    Query<'w, 's, (Entity, &'static PartyMember, &'static CombatStats, &'static Position, &'static Viewshed)>;
type HostileSight<'w, 's> =
    Query<'w, 's, (&'static CombatStats, &'static Position, &'static Viewshed), (With<Monster>, Without<PartyMember>)>;

/// Applies `step` to the mode and the frame loop. In combat, the walk stops
/// and a hero out of AP hands control to the next one who can act.
fn take_step(
    step: Step,
    mode: &mut Mode,
    next_state: &mut NextState<RunState>,
    active: &mut ActiveUnit,
    walk: &mut WalkPath,
    party: &PartySight,
    hostiles: &HostileSight,
) -> RunState {
    let living: Vec<_> = party.iter().filter(|m| m.2.hp > 0).collect();
    let situation = Situation {
        party: living.iter().map(|m| m.0).collect(),
        hostiles: hostiles
            .iter()
            .filter(|(stats, ..)| stats.hp > 0)
            .map(|(_, pos, view)| Sight {
                sees_party: living.iter().any(|m| view.visible_tiles.contains(&(m.3.x, m.3.y))),
                seen_by_party: living.iter().any(|m| m.4.visible_tiles.contains(&(pos.x, pos.y))),
            })
            .collect(),
    };
    let (new_mode, next) = advance(mode, step, &situation);
    *mode = new_mode;
    if matches!(mode, Mode::Combat { .. }) {
        *walk = WalkPath::default();
        if !active.0.is_some_and(|unit| mode.can_act(unit)) {
            let ready: Vec<_> = living.iter().map(|m| (m.1.slot, m.0, mode.can_act(m.0))).collect();
            if let Some(unit) = next_unit(&ready, active.0) {
                active.0 = Some(unit);
            }
        }
    }
    next_state.set_if_different(next);
    next
}

#[allow(clippy::too_many_arguments)]
fn after_party_action(
    mut mode: ResMut<Mode>,
    mut next_state: ResMut<NextState<RunState>>,
    mut active: ResMut<ActiveUnit>,
    mut walk: ResMut<WalkPath>,
    mut turn_ended: ResMut<TurnEnded>,
    mut hero_action: ResMut<HeroAction>,
    mut pending_magic_map: ResMut<PendingMagicMap>,
    mut reveal: ResMut<MagicMapReveal>,
    party: PartySight,
    hostiles: HostileSight,
) {
    let action = hero_action.0.take().unwrap_or(Action::UseItem);
    let step = match (std::mem::take(&mut turn_ended.0), active.0) {
        (false, Some(unit)) => Step::PartyActed(unit, action),
        _ => Step::EndTurn,
    };
    let next = take_step(step, &mut mode, &mut next_state, &mut active, &mut walk, &party, &hostiles);
    if next != RunState::GameOver && std::mem::take(&mut pending_magic_map.0) {
        *reveal = MagicMapReveal { row: 0, then: next };
        next_state.set_if_different(RunState::MagicMapReveal);
    }
}

fn after_enemy_turn(
    mut mode: ResMut<Mode>,
    mut next_state: ResMut<NextState<RunState>>,
    mut active: ResMut<ActiveUnit>,
    mut walk: ResMut<WalkPath>,
    party: PartySight,
    hostiles: HostileSight,
) {
    take_step(Step::EnemiesActed, &mut mode, &mut next_state, &mut active, &mut walk, &party, &hostiles);
}

fn magic_map_reveal(
    mut reveal: ResMut<MagicMapReveal>,
    mut map: ResMut<Map>,
    mut next_state: ResMut<NextState<RunState>>,
    mut tile_query: Query<(&Position, &mut Revealed), With<Tile>>,
) {
    let row = reveal.row;

    if row >= map::MAP_HEIGHT as i32 {
        // Done revealing: carry on where the action left off
        next_state.set_if_different(reveal.then);
        return;
    }

    // Reveal all tiles in this row
    for x in 0..map::MAP_WIDTH as i32 {
        let idx = map.xy_idx(x, row);
        map.revealed_tiles[idx] = true;
    }

    // Update tile entities in this row
    for (pos, mut revealed) in &mut tile_query {
        if pos.y == row {
            *revealed = Revealed(RevealedState::Explored);
        }
    }

    reveal.row += 1;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn heroes(n: usize) -> Vec<Entity> {
        let mut world = World::new();
        (0..n).map(|_| world.spawn_empty().id()).collect()
    }

    fn situation(party: &[Entity], hostiles: &[(bool, bool)]) -> Situation {
        Situation {
            party: party.to_vec(),
            hostiles: hostiles
                .iter()
                .map(|&(sees_party, seen_by_party)| Sight { sees_party, seen_by_party })
                .collect(),
        }
    }

    /// Combat in `round` with the AP each listed hero has spent.
    fn combat(round: u32, ap_spent: &[(Entity, u8)]) -> Mode {
        Mode::Combat { round, ap_spent: ap_spent.iter().copied().collect() }
    }

    fn acted(unit: Entity, action: Action) -> Step {
        Step::PartyActed(unit, action)
    }

    #[test]
    fn exploring_without_contact_lets_the_monsters_take_each_step() {
        let p = heroes(2);
        // Only one side sees the other: no combat yet.
        let s = situation(&p, &[(true, false), (false, true)]);
        assert_eq!(advance(&Mode::Explore, acted(p[0], Action::Move), &s), (Mode::Explore, RunState::EnemyTurn));
        assert_eq!(advance(&Mode::Explore, Step::EnemiesActed, &s), (Mode::Explore, RunState::AwaitingInput));
        assert_eq!(advance(&Mode::Explore, Step::EndTurn, &s), (Mode::Explore, RunState::AwaitingInput));
        assert!(Mode::Explore.can_act(p[0]));
    }

    #[test]
    fn mutual_sight_drops_into_the_party_phase_of_round_one() {
        let p = heroes(2);
        let s = situation(&p, &[(false, false), (true, true)]);
        let start = (combat(1, &[]), RunState::AwaitingInput);
        assert_eq!(advance(&Mode::Explore, acted(p[0], Action::Move), &s), start);
        assert_eq!(advance(&Mode::Explore, Step::EnemiesActed, &s), start);
        assert_eq!(start.0.ap_left(p[0]), AP_PER_ROUND);
    }

    #[test]
    fn each_action_spends_its_ap_and_a_hero_with_ap_left_keeps_acting() {
        let p = heroes(2);
        let s = situation(&p, &[(true, true)]);
        let (mode, next) = advance(&combat(1, &[]), acted(p[0], Action::Move), &s);
        assert_eq!((mode.clone(), next), (combat(1, &[(p[0], 1)]), RunState::AwaitingInput));
        assert_eq!(mode.ap_left(p[0]), 1);
        assert!(mode.can_act(p[0]));
        let (mode, _) = advance(&mode, acted(p[0], Action::UseItem), &s);
        assert_eq!(mode.ap_left(p[0]), 0);
        assert!(!mode.can_act(p[0]));
        // A dash spends both, as does an attack or a wait with any AP left.
        for action in [Action::Dash, Action::Attack, Action::Wait] {
            assert_eq!(advance(&combat(1, &[]), acted(p[1], action), &s).0.ap_left(p[1]), 0, "{action:?}");
        }
        assert_eq!(advance(&combat(1, &[(p[1], 1)]), acted(p[1], Action::Attack), &s).0.ap_left(p[1]), 0);
    }

    #[test]
    fn the_party_phase_ends_when_no_living_hero_has_ap_left() {
        let p = heroes(3);
        let s = situation(&p, &[(true, true)]);
        let (mode, next) = advance(&combat(2, &[]), acted(p[1], Action::Dash), &s);
        assert_eq!((mode.clone(), next), (combat(2, &[(p[1], 2)]), RunState::AwaitingInput));
        let (mode, next) = advance(&mode, acted(p[0], Action::Attack), &s);
        assert_eq!((mode.clone(), next), (combat(2, &[(p[1], 2), (p[0], 2)]), RunState::AwaitingInput));
        let (mode, next) = advance(&mode, acted(p[2], Action::Move), &s);
        assert_eq!(next, RunState::AwaitingInput, "one AP left");
        let (mode, next) = advance(&mode, acted(p[2], Action::Move), &s);
        assert_eq!((mode, next), (combat(2, &[(p[1], 2), (p[0], 2), (p[2], 2)]), RunState::EnemyTurn));
        // A fallen hero does not hold the phase open.
        let s = situation(&p[1..], &[(true, true)]);
        assert_eq!(advance(&combat(1, &[(p[1], 2)]), acted(p[2], Action::Wait), &s).1, RunState::EnemyTurn);
    }

    #[test]
    fn ending_the_turn_hands_over_to_the_enemy_phase() {
        let p = heroes(2);
        let s = situation(&p, &[(true, false)]);
        let mode = combat(3, &[(p[0], 1)]);
        assert_eq!(advance(&mode, Step::EndTurn, &s), (mode, RunState::EnemyTurn));
    }

    #[test]
    fn the_enemy_phase_leads_to_the_next_round_with_full_ap() {
        let p = heroes(2);
        let s = situation(&p, &[(true, false)]);
        assert_eq!(
            advance(&combat(1, &[(p[0], 2), (p[1], 2)]), Step::EnemiesActed, &s),
            (combat(2, &[]), RunState::AwaitingInput)
        );
    }

    #[test]
    fn combat_ends_when_no_hostile_is_aware_of_the_party() {
        let p = heroes(2);
        let explore = (Mode::Explore, RunState::AwaitingInput);
        // The last hostile died mid party phase.
        assert_eq!(advance(&combat(1, &[]), acted(p[0], Action::Attack), &situation(&p, &[])), explore);
        // Seen but not seeing: it has lost the party.
        let s = situation(&p, &[(false, true)]);
        assert_eq!(advance(&combat(4, &[]), Step::EnemiesActed, &s), explore);
        assert_eq!(advance(&combat(4, &[]), Step::EndTurn, &s), explore);
    }

    #[test]
    fn a_wiped_party_is_game_over_in_either_mode() {
        let s = situation(&[], &[(true, true)]);
        assert_eq!(advance(&combat(2, &[]), Step::EnemiesActed, &s).1, RunState::GameOver);
        assert_eq!(advance(&Mode::Explore, Step::EnemiesActed, &s).1, RunState::GameOver);
    }
}
