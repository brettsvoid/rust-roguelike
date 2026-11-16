//! The combat HUD's rules as pure functions over plain data, so the HUD
//! systems only draw: which action buttons the active hero gets and why one
//! is disabled (`action_bar`), the numbers of the shot preview
//! (`shot_preview`, from the same `combat::aim_at` the attack rolls), and
//! which banner a change of mode or phase shows (`banner`).
//!
//! A button press goes through `ActionRequest`: the HUD names the key the
//! button stands for and `player::handle_player_input` takes it as if typed,
//! so a button and its hotkey share one code path.

use bevy::prelude::*;

use crate::combat::{aim_at, Refusal};
use crate::components::Weapon;
use crate::cover::Exposure;
use crate::map::Map;
use crate::tactics::Action;

/// A key a HUD button pressed for the player, taken by
/// `player::handle_player_input` as if typed this frame.
#[derive(Resource, Default, Debug)]
pub struct ActionRequest(pub Option<KeyCode>);

/// One action bar button, in bar order. Moving has no button: a click on a
/// tile in the drawn range moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HudAction {
    /// The nearest hostile in reach (F, `tactics::attack_target`).
    Attack,
    Overwatch,
    Hunker,
    /// Opens the inventory; using an item there costs 1 AP.
    UseItem,
    /// Ends this hero's turn (Space); exploring, rests a turn.
    Wait,
    /// Ends the party phase (E).
    EndTurn,
}

impl HudAction {
    pub const ALL: [HudAction; 6] = [
        HudAction::Attack,
        HudAction::Overwatch,
        HudAction::Hunker,
        HudAction::UseItem,
        HudAction::Wait,
        HudAction::EndTurn,
    ];

    /// The key `handle_player_input` maps to this action.
    pub fn key(self) -> KeyCode {
        match self {
            HudAction::Attack => KeyCode::KeyF,
            HudAction::Overwatch => KeyCode::KeyO,
            HudAction::Hunker => KeyCode::KeyH,
            HudAction::UseItem => KeyCode::KeyI,
            HudAction::Wait => KeyCode::Space,
            HudAction::EndTurn => KeyCode::KeyE,
        }
    }

    /// The key as printed on the button.
    pub fn hotkey(self) -> &'static str {
        match self {
            HudAction::Attack => "F",
            HudAction::Overwatch => "O",
            HudAction::Hunker => "H",
            HudAction::UseItem => "I",
            HudAction::Wait => "Space",
            HudAction::EndTurn => "E",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            HudAction::Attack => "Attack",
            HudAction::Overwatch => "Overwatch",
            HudAction::Hunker => "Hunker",
            HudAction::UseItem => "Use Item",
            HudAction::Wait => "Wait",
            HudAction::EndTurn => "End Turn",
        }
    }

    /// The AP rule behind the button, if it spends AP.
    fn action(self) -> Option<Action> {
        match self {
            HudAction::Attack => Some(Action::Attack),
            HudAction::Overwatch => Some(Action::Overwatch),
            HudAction::Hunker => Some(Action::Hunker),
            HudAction::UseItem => Some(Action::UseItem),
            HudAction::Wait => Some(Action::Wait),
            HudAction::EndTurn => None,
        }
    }
}

/// What the action bar decides from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ActionContext {
    pub combat: bool,
    /// The active hero's AP left this round.
    pub ap_left: u8,
    /// A hostile the party sees is within the active hero's weapon reach.
    pub target_in_reach: bool,
}

/// A button to show, and why it cannot be used if it cannot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActionButton {
    pub action: HudAction,
    pub refusal: Option<&'static str>,
}

/// The buttons for the active hero, in bar order. Exploring, only the item
/// and wait buttons show; the rest are combat actions.
pub fn action_bar(ctx: ActionContext) -> Vec<ActionButton> {
    HudAction::ALL
        .into_iter()
        .filter(|&action| ctx.combat || matches!(action, HudAction::UseItem | HudAction::Wait))
        .map(|action| {
            let affordable = !ctx.combat || action.action().is_none_or(|a| a.spend(ctx.ap_left).is_some());
            let refusal = if !affordable {
                Some(if ctx.ap_left == 0 { "No AP" } else { "Needs 1 AP" })
            } else if action == HudAction::Attack && !ctx.target_in_reach {
                Some("No target")
            } else {
                None
            };
            ActionButton { action, refusal }
        })
        .collect()
}

/// The attacker side of a shot preview.
#[derive(Clone, Copy, Debug)]
pub struct Shooter {
    pub at: (i32, i32),
    pub aim: i32,
    /// Added to the weapon's damage: power, +1 well fed (as `Strikes::strike`).
    pub bonus: i32,
}

/// The target side of a shot preview.
#[derive(Clone, Copy, Debug)]
pub struct Mark {
    pub at: (i32, i32),
    pub defence: i32,
    /// Its own armour and its equipment's.
    pub armour: i32,
    pub hunkered: bool,
}

/// What an attack would do before it is made.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShotPreview {
    /// Percent to hit.
    pub hit: i32,
    /// Percent of hits that are critical.
    pub crit: i32,
    /// Damage of a plain hit, least and most (a critical adds half).
    pub damage: (i32, i32),
    pub exposure: Exposure,
}

/// The numbers `Strikes::strike` rolls against, or why it would refuse.
pub fn shot_preview(map: &Map, weapon: &Weapon, shooter: Shooter, mark: Mark) -> Result<ShotPreview, Refusal> {
    let (hit, exposure) = aim_at(map, weapon, shooter.aim, shooter.at, mark.defence, mark.at, mark.hunkered)?;
    let damage = |roll: i32| (roll + shooter.bonus - mark.armour).max(0);
    Ok(ShotPreview {
        hit,
        crit: exposure.crit_chance(weapon.crit_chance),
        damage: (damage(weapon.damage.0), damage(weapon.damage.1)),
        exposure,
    })
}

/// Where the turn loop stands, as the banner sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Explore,
    /// The party phase of a combat round.
    Party(u32),
    /// The enemy phase of a combat round.
    Enemy(u32),
}

/// The banner for moving from phase `from` to `to`, if any.
pub fn banner(from: Phase, to: Phase) -> Option<String> {
    match (from, to) {
        (Phase::Explore, Phase::Party(_) | Phase::Enemy(_)) => Some("Combat".into()),
        (Phase::Party(_) | Phase::Enemy(_), Phase::Explore) => Some("Victory".into()),
        (Phase::Party(_), Phase::Enemy(_)) => Some("Enemy phase".into()),
        (Phase::Enemy(old) | Phase::Party(old), Phase::Party(round)) if round > old => Some(format!("Round {round}")),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cover::Cover;
    use crate::map::TileType;

    fn bar(combat: bool, ap_left: u8, target_in_reach: bool) -> Vec<(HudAction, Option<&'static str>)> {
        action_bar(ActionContext { combat, ap_left, target_in_reach })
            .into_iter()
            .map(|b| (b.action, b.refusal))
            .collect()
    }

    #[test]
    fn a_fresh_hero_beside_a_hostile_can_do_everything() {
        assert_eq!(
            bar(true, 2, true),
            vec![
                (HudAction::Attack, None),
                (HudAction::Overwatch, None),
                (HudAction::Hunker, None),
                (HudAction::UseItem, None),
                (HudAction::Wait, None),
                (HudAction::EndTurn, None),
            ]
        );
    }

    #[test]
    fn attack_needs_a_target_in_reach() {
        assert_eq!(bar(true, 2, false)[0], (HudAction::Attack, Some("No target")));
        assert_eq!(bar(true, 1, false)[0], (HudAction::Attack, Some("No target")));
    }

    #[test]
    fn one_ap_is_enough_for_overwatch_hunker_and_an_item() {
        let one = bar(true, 1, true);
        assert!(one.iter().all(|b| b.1.is_none()), "{one:?}");
    }

    #[test]
    fn with_no_ap_only_end_turn_is_left() {
        assert_eq!(
            bar(true, 0, true),
            vec![
                (HudAction::Attack, Some("No AP")),
                (HudAction::Overwatch, Some("No AP")),
                (HudAction::Hunker, Some("No AP")),
                (HudAction::UseItem, Some("No AP")),
                (HudAction::Wait, Some("No AP")),
                (HudAction::EndTurn, None),
            ]
        );
    }

    #[test]
    fn exploring_shows_only_item_and_wait() {
        assert_eq!(bar(false, 2, false), vec![(HudAction::UseItem, None), (HudAction::Wait, None)]);
        assert_eq!(bar(false, 2, true), vec![(HudAction::UseItem, None), (HudAction::Wait, None)]);
    }

    #[test]
    fn each_button_names_its_key() {
        let keys: Vec<_> = HudAction::ALL.iter().map(|a| (a.key(), a.hotkey())).collect();
        assert_eq!(
            keys,
            vec![
                (KeyCode::KeyF, "F"),
                (KeyCode::KeyO, "O"),
                (KeyCode::KeyH, "H"),
                (KeyCode::KeyI, "I"),
                (KeyCode::Space, "Space"),
                (KeyCode::KeyE, "E"),
            ]
        );
    }

    /// A 9 by 9 room of floor inside a wall ring.
    fn room() -> Map {
        let mut map = Map::new(9, 9, 1);
        for y in 1..8 {
            for x in 1..8 {
                let idx = map.xy_idx(x, y);
                map.tiles[idx] = TileType::Floor;
            }
        }
        map
    }

    const SWORD: Weapon = Weapon { damage: (3, 5), range: 1, optimal_range: 1, crit_chance: 10 };
    const BOW: Weapon = Weapon { damage: (1, 4), range: 8, optimal_range: 5, crit_chance: 10 };

    fn shooter(at: (i32, i32)) -> Shooter {
        Shooter { at, aim: 65, bonus: 1 }
    }

    fn mark(at: (i32, i32), armour: i32, hunkered: bool) -> Mark {
        Mark { at, defence: 0, armour, hunkered }
    }

    #[test]
    fn preview_of_a_plain_melee_hit() {
        let map = room();
        assert_eq!(
            shot_preview(&map, &SWORD, shooter((3, 4)), mark((4, 4), 2, false)),
            Ok(ShotPreview { hit: 65, crit: 10, damage: (2, 4), exposure: Exposure::Open })
        );
    }

    #[test]
    fn preview_matches_aim_at_through_cover_flank_and_hunker() {
        let mut map = room();
        let idx = map.xy_idx(4, 3);
        map.prop_cover.insert(idx, Cover::Half);
        let north = (0, -1);
        // From the north the table counts: 65 - 20.
        let covered = shot_preview(&map, &BOW, shooter((4, 1)), mark((4, 4), 0, false)).unwrap();
        assert_eq!((covered.hit, covered.crit), (45, 10));
        assert_eq!(covered.exposure, Exposure::Covered { cover: Cover::Half, side: north });
        // Hunkered, the cover doubles and crits are off: 65 - 40.
        let hunkered = shot_preview(&map, &BOW, shooter((4, 1)), mark((4, 4), 0, true)).unwrap();
        assert_eq!((hunkered.hit, hunkered.crit), (25, 0));
        assert_eq!(hunkered.exposure.note(), Some("half cover, hunkered"));
        // From the south the table faces away: flanked, +30 crit.
        let flanked = shot_preview(&map, &BOW, shooter((4, 7)), mark((4, 4), 0, false)).unwrap();
        assert_eq!((flanked.hit, flanked.crit, flanked.exposure), (65, 40, Exposure::Flanked));
        // Every case agrees with the attack's own rule.
        for (from, hunker) in [((4, 1), false), ((4, 1), true), ((4, 7), false), ((7, 7), false)] {
            let preview = shot_preview(&map, &BOW, shooter(from), mark((4, 4), 0, hunker)).unwrap();
            assert_eq!(Ok((preview.hit, preview.exposure)), aim_at(&map, &BOW, 65, from, 0, (4, 4), hunker));
        }
    }

    #[test]
    fn preview_damage_never_goes_below_zero_and_refusals_pass_through() {
        let map = room();
        let armoured = shot_preview(&map, &SWORD, shooter((3, 4)), mark((4, 4), 5, false)).unwrap();
        assert_eq!(armoured.damage, (0, 1));
        assert_eq!(shot_preview(&map, &SWORD, shooter((1, 4)), mark((4, 4), 0, false)), Err(Refusal::OutOfRange));
    }

    #[test]
    fn banners_for_each_change_of_mode_or_phase() {
        assert_eq!(banner(Phase::Explore, Phase::Party(1)).as_deref(), Some("Combat"));
        assert_eq!(banner(Phase::Party(1), Phase::Enemy(1)).as_deref(), Some("Enemy phase"));
        assert_eq!(banner(Phase::Enemy(1), Phase::Party(2)).as_deref(), Some("Round 2"));
        assert_eq!(banner(Phase::Party(3), Phase::Explore).as_deref(), Some("Victory"));
        assert_eq!(banner(Phase::Enemy(2), Phase::Explore).as_deref(), Some("Victory"));
        assert_eq!(banner(Phase::Party(1), Phase::Party(1)), None);
        assert_eq!(banner(Phase::Explore, Phase::Explore), None);
    }
}
