//! Procedural animation of the character and monster models (A09).
//!
//! Every character model is rigid parts with their origins at the joints
//! (`<model>_torso`, `_head`, `_arm_l`, ...; see the characters and monsters
//! kits), nested so that a held item hangs from its arm and the head, arms,
//! wings and tail from the torso or body. `rig_model` finds the parts in each
//! creature's model scene once and puts a `Rig` on the scene. `animate` then
//! sets each part's local transform every frame: its rest pose plus
//! `pose(plan, part, motion)`, plus an idle layer (breathing, sway) that
//! eases in while the unit plays no beat, plus the wings and hovering that
//! never stop.
//!
//! The motion comes from the beat the unit is playing (`Presentation::playing`):
//! a `Move` walks, in step with the distance glided, a `Lunge` swings so the
//! blow lands half way, a `Projectile` draws and releases, a `Cast` raises the
//! hand, a `Flinch` recoils and a `Death` goes limp. Every motion is at rest at
//! its start and end, so one motion hands over to the next without a snap.
//!
//! The body plan (`Plan`) comes from which parts the model has. Every amount,
//! speed and duration is a constant below. All of it runs on `Time`, so
//! harness captures repeat. A rig hidden by the fog of war is not posed.

use std::f32::consts::{PI, TAU};

use bevy::prelude::*;
use bevy::world_serialization::WorldInstanceReady;

use crate::entity_render::{EntityShape, ModelScene};
use crate::presentation::{
    glide_distance, path_length, Beat, Presentation, CAST_SECS, DEATH_FALL, DEATH_SECS, DRAW_SECS, FLINCH_SECS,
    GLIDE_SPEED, LUNGE_SECS, PROJECTILE_SECS,
};

/// One breath in and out, in seconds.
pub const BREATH_SECS: f32 = 3.2;
/// How far the torso rises with a breath, in metres.
pub const BREATH_RISE: f32 = 0.012;
/// Idle sway of the torso and arms, and the head's slow look about, in radians.
pub const IDLE_LEAN: f32 = 0.025;
pub const IDLE_ARM: f32 = 0.06;
pub const IDLE_HEAD: f32 = 0.08;
pub const TAIL_IDLE: f32 = 0.25;
/// Time constant of the idle layer fading in and out round other motions.
pub const IDLE_BLEND_SECS: f32 = 0.15;
/// Fliers (the wraith and the bat) bob this far, once per `HOVER_SECS`.
pub const HOVER_RISE: f32 = 0.06;
pub const HOVER_SECS: f32 = 1.7;
/// Wing beats per second, idle and moving: the bat's, and back wings (imp, demon lord).
pub const BAT_FLAP_HZ: (f32, f32) = (3.0, 5.0);
pub const BACK_FLAP_HZ: (f32, f32) = (0.6, 2.2);
/// Wing beat arcs, in radians.
pub const BAT_FLAP: f32 = 0.6;
pub const BACK_FLAP: f32 = 0.25;

/// Walking: each tile walked is one footfall, legs passing under the body at
/// every tile. Swings in radians.
pub const LEG_SWING: f32 = 0.6;
pub const ARM_SWING: f32 = 0.5;
pub const PAW_SWING: f32 = 0.55;
pub const WALK_LEAN: f32 = 0.12;
pub const WALK_TWIST: f32 = 0.1;
/// A floater's arms trail behind it while it moves.
pub const FLOAT_TRAIL: f32 = 0.35;
/// The spider's legs: strokes per tile, sweep and lift.
pub const SPIDER_STROKES: f32 = 2.0;
pub const SPIDER_SWEEP: f32 = 0.35;
pub const SPIDER_LIFT: f32 = 0.3;

/// A melee blow, keyed over the lunge (0 to 1): the weapon arm rises
/// overhead, chops down to land half way, and comes back.
pub const STRIKE_KEYS: [(f32, f32); 4] = [(0.0, 0.0), (0.3, 2.3), (0.5, 0.9), (1.0, 0.0)];
/// The arm that holds a bow or staff out, in radians.
pub const AIM_RAISE: f32 = 1.45;
/// A cast lifts the hand this far, nearly overhead.
pub const CAST_RAISE: f32 = 2.4;
/// How far a hit knocks the torso back, in radians.
pub const FLINCH_LEAN: f32 = 0.3;
/// How far the arms fall open in death.
pub const LIMP_ARMS: f32 = 0.9;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Side {
    L,
    R,
}

impl Side {
    /// +1 for the left (+x, the model's left as it faces +z), -1 for the right.
    fn sign(self) -> f32 {
        match self {
            Side::L => 1.0,
            Side::R => -1.0,
        }
    }

    fn of(s: &str) -> Option<Side> {
        match s {
            "l" => Some(Side::L),
            "r" => Some(Side::R),
            _ => None,
        }
    }
}

/// A rigid part of a model, from its node name.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Part {
    Torso,
    Head,
    Arm(Side),
    Leg(Side),
    /// A held item, by the side of the hand holding it.
    Held(Side),
    Tail,
    Wing(Side),
    /// A beast's body.
    Body,
    Paw { front: bool, side: Side },
    /// A spider's leg, `k` 0 (front) to 3 (back).
    SpiderLeg { side: Side, k: u8 },
}

impl Part {
    /// The part a node name stands for (`hero_mage_arm_r`, `rat_leg_fl`,
    /// `giant_spider_leg_l3`). Held items are named for what they are, so they
    /// are found by their parent instead.
    pub fn named(name: &str) -> Option<Part> {
        let (rest, last) = name.rsplit_once('_')?;
        Some(match last {
            "torso" => Part::Torso,
            "head" => Part::Head,
            "body" => Part::Body,
            "tail" => Part::Tail,
            _ => match (rest.rsplit_once('_')?.1, last) {
                ("arm", s) => Part::Arm(Side::of(s)?),
                ("wing", s) => Part::Wing(Side::of(s)?),
                ("leg", "fl" | "fr" | "bl" | "br") => {
                    Part::Paw { front: last.starts_with('f'), side: Side::of(&last[1..])? }
                }
                ("leg", s) if s.len() == 2 => {
                    Part::SpiderLeg { side: Side::of(&s[..1])?, k: s[1..].parse::<u8>().ok()?.checked_sub(1)? }
                }
                ("leg", s) => Part::Leg(Side::of(s)?),
                _ => return None,
            },
        })
    }
}

/// A body plan, from the parts a model has.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Plan {
    /// Heroes, townsfolk and the humanoid monsters (with wings: the imp, the demon lord).
    Biped,
    /// A torso with no legs that hovers: the wraith.
    Floater,
    /// The rat and the hellhound.
    Quadruped,
    /// A body, a head and wings: the giant bat.
    Bat,
    Spider,
}

impl Plan {
    pub fn of(parts: &[Part]) -> Option<Plan> {
        let has = |f: fn(&Part) -> bool| parts.iter().any(f);
        Some(if has(|p| matches!(p, Part::SpiderLeg { .. })) {
            Plan::Spider
        } else if has(|p| matches!(p, Part::Paw { .. })) {
            Plan::Quadruped
        } else if has(|p| *p == Part::Torso) {
            if has(|p| matches!(p, Part::Leg(_))) {
                Plan::Biped
            } else {
                Plan::Floater
            }
        } else if has(|p| *p == Part::Body) {
            Plan::Bat
        } else {
            return None;
        })
    }

    fn hovers(self) -> bool {
        matches!(self, Plan::Floater | Plan::Bat)
    }
}

/// What a unit is doing, with its progress. Progress runs 0 to 1, except a
/// walk's, which counts the tiles walked (whole at each tile).
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Motion {
    Idle,
    /// `speed` is the share of the top glide speed, 0 at both ends.
    Walk { tiles: f32, speed: f32 },
    Strike(f32),
    Shoot(f32),
    Cast(f32),
    Flinch(f32),
    Die(f32),
}

impl Motion {
    /// The motion `t` seconds into `beat`.
    pub fn of(beat: &Beat, t: f32) -> Motion {
        let p = |secs: f32| (t / secs).clamp(0.0, 1.0);
        match beat {
            Beat::Move { cells, .. } => {
                let length = path_length(cells);
                let steps = cells.len().saturating_sub(1) as f32;
                let dt = 1.0 / 120.0;
                let speed = (glide_distance(t + dt, length) - glide_distance(t - dt, length)) / (2.0 * dt * GLIDE_SPEED);
                Motion::Walk {
                    tiles: glide_distance(t, length) * steps / length.max(f32::EPSILON),
                    speed: speed.clamp(0.0, 1.0),
                }
            }
            Beat::Lunge { .. } => Motion::Strike(p(LUNGE_SECS)),
            Beat::Projectile { .. } => Motion::Shoot(p(DRAW_SECS + PROJECTILE_SECS)),
            Beat::Cast { .. } => Motion::Cast(p(CAST_SECS)),
            Beat::Flinch { .. } => Motion::Flinch(p(FLINCH_SECS)),
            Beat::Death { .. } => Motion::Die(p(DEATH_SECS)),
            _ => Motion::Idle,
        }
    }
}

fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The value at `p` of eased keys `(p, value)`, in order of `p`.
fn keys(p: f32, k: &[(f32, f32)]) -> f32 {
    for w in k.windows(2) {
        let ((p0, v0), (p1, v1)) = (w[0], w[1]);
        if p <= p1 {
            return v0 + (v1 - v0) * smoothstep((p - p0) / (p1 - p0));
        }
    }
    k.last().map_or(0.0, |&(_, v)| v)
}

/// A hanging limb (arm, leg, paw) swung forward (towards +z, the way the model faces).
fn swing(a: f32) -> Quat {
    Quat::from_rotation_x(-a)
}

/// An upright part (torso, head) leant forward; a beast's body dips its front.
fn lean(a: f32) -> Quat {
    Quat::from_rotation_x(a)
}

/// A part on `side` swung out to that side: a hanging limb opens, a wing lifts.
fn out(side: Side, a: f32) -> Quat {
    Quat::from_rotation_z(side.sign() * a)
}

/// Turned about the vertical: positive brings the right shoulder forward.
fn twist(a: f32) -> Quat {
    Quat::from_rotation_y(a)
}

fn turn(rotation: Quat) -> Transform {
    Transform::from_rotation(rotation)
}

/// The arm a ranged attacker aims with: the bow hand, else the right.
fn aim_side(bow: bool) -> Side {
    if bow {
        Side::L
    } else {
        Side::R
    }
}

/// How `part` moves away from its rest pose for `motion`. `bow` is whether
/// the model holds a bow (in its left hand).
pub fn pose(plan: Plan, part: Part, motion: Motion, bow: bool) -> Transform {
    use Part::*;
    let biped = matches!(plan, Plan::Biped | Plan::Floater);
    match motion {
        Motion::Idle => Transform::IDENTITY,
        Motion::Walk { tiles, speed } => {
            // Zero at every tile: the legs pass under the body, one footfall per tile.
            let stride = (PI * tiles).sin();
            turn(match (plan, part) {
                (Plan::Biped, Leg(side)) => swing(LEG_SWING * stride * side.sign()),
                (Plan::Biped, Arm(side)) => swing(-ARM_SWING * stride * side.sign()),
                (Plan::Floater, Arm(_)) => swing(-FLOAT_TRAIL * speed),
                (_, Torso) => lean(WALK_LEAN * speed) * twist(WALK_TWIST * stride),
                (_, Head) if biped => lean(-WALK_LEAN * speed * 0.6) * twist(-WALK_TWIST * stride),
                // Trotting: the diagonal pairs (front left with back right) swing together.
                (_, Paw { front, side }) => swing(PAW_SWING * stride * side.sign() * if front { 1.0 } else { -1.0 }),
                (_, Tail) => twist(TAIL_IDLE * stride),
                (Plan::Quadruped, Head) => lean(0.08 * stride.abs()),
                (Plan::Spider, SpiderLeg { side, k }) => {
                    let phase = TAU * SPIDER_STROKES / 2.0 * tiles + k as f32 * PI / 2.0 + if side == Side::R { PI } else { 0.0 };
                    twist(side.sign() * SPIDER_SWEEP * speed * phase.sin())
                        * out(side, SPIDER_LIFT * speed * phase.cos().max(0.0))
                }
                _ => Quat::IDENTITY,
            })
        }
        Motion::Strike(p) => turn(match (plan, part) {
            (Plan::Biped, Arm(Side::R)) => swing(keys(p, &STRIKE_KEYS)),
            (Plan::Biped, Arm(Side::L)) => swing(keys(p, &[(0.0, 0.0), (0.5, 0.5), (1.0, 0.0)])),
            (Plan::Floater, Arm(_)) => swing(keys(p, &[(0.0, 0.0), (0.3, 1.9), (0.5, 1.3), (1.0, 0.0)])),
            (_, Torso) => {
                lean(keys(p, &[(0.0, 0.0), (0.3, -0.12), (0.5, 0.18), (1.0, 0.0)]))
                    * twist(keys(p, &[(0.0, 0.0), (0.3, -0.25), (0.5, 0.2), (1.0, 0.0)]))
            }
            // A bite: the head draws back and snaps down.
            (Plan::Quadruped | Plan::Bat, Head) => lean(keys(p, &[(0.0, 0.0), (0.3, -0.35), (0.5, 0.45), (1.0, 0.0)])),
            (Plan::Quadruped, Paw { front: true, .. }) => swing(keys(p, &[(0.0, 0.0), (0.3, 0.35), (0.5, 0.5), (1.0, 0.0)])),
            (_, Body) => lean(keys(p, &[(0.0, 0.0), (0.3, -0.15), (0.5, 0.2), (1.0, 0.0)])),
            // The spider rears its front legs and stabs them down.
            (Plan::Spider, SpiderLeg { side, k: 0 }) => out(side, keys(p, &[(0.0, 0.0), (0.3, 0.9), (0.5, -0.15), (1.0, 0.0)])),
            _ => Quat::IDENTITY,
        }),
        Motion::Shoot(p) => {
            // Drawn by `draw`, held until the bolt flies, lowered while it flies.
            let draw = DRAW_SECS / (DRAW_SECS + PROJECTILE_SECS);
            let raise = keys(p, &[(0.0, 0.0), (draw * 0.7, AIM_RAISE), (draw, AIM_RAISE), (1.0, 0.0)]);
            let aim = aim_side(bow);
            turn(match part {
                Arm(side) if side == aim => swing(raise),
                // The bow or staff stays upright in the raised hand.
                Held(side) if side == aim => swing(-raise),
                Arm(_) if bow => swing(keys(p, &[(0.0, 0.0), (draw * 0.7, 1.2), (draw, 1.0), (draw + 0.1, 1.5), (1.0, 0.0)])),
                Torso => twist(-aim.sign() * 0.35 * raise / AIM_RAISE),
                _ => Quat::IDENTITY,
            })
        }
        Motion::Cast(p) => {
            let raise = keys(p, &[(0.0, 0.0), (0.35, CAST_RAISE), (0.65, CAST_RAISE), (1.0, 0.0)]);
            turn(match part {
                Arm(Side::R) => swing(raise),
                Held(Side::R) => swing(-raise),
                Arm(Side::L) => swing(0.25 * raise),
                Head => lean(-0.08 * raise),
                _ => Quat::IDENTITY,
            })
        }
        Motion::Flinch(p) => {
            let knock = (PI * p).sin();
            turn(match part {
                Torso | Body => lean(-FLINCH_LEAN * knock),
                Head => lean(-0.7 * FLINCH_LEAN * knock),
                Arm(side) => swing(0.35 * knock) * out(side, 0.2 * knock),
                Wing(side) if plan == Plan::Bat => out(side, 0.5 * knock),
                _ => Quat::IDENTITY,
            })
        }
        Motion::Die(p) => {
            let limp = smoothstep(p / DEATH_FALL);
            turn(match part {
                Arm(side) => out(side, LIMP_ARMS * limp) * swing(0.3 * limp),
                Leg(side) => out(side, 0.25 * limp),
                Head => lean(0.5 * limp),
                Paw { side, .. } => out(side, 0.6 * limp),
                Tail => twist(0.5 * limp),
                SpiderLeg { side, .. } => out(side, 0.9 * limp),
                Wing(side) => out(side, -limp),
                _ => Quat::IDENTITY,
            })
        }
    }
}

/// The idle layer at `clock` seconds (each unit's clock is offset, so a
/// group does not breathe together).
pub fn idle(plan: Plan, part: Part, clock: f32) -> Transform {
    let breath = (TAU * clock / BREATH_SECS).sin();
    let look = (TAU * clock / (BREATH_SECS * 2.3) + 1.0).sin();
    match part {
        Part::Torso | Part::Body if plan != Plan::Bat => {
            Transform::from_xyz(0.0, BREATH_RISE * breath, 0.0).with_rotation(lean(IDLE_LEAN * breath))
        }
        Part::Head => turn(twist(IDLE_HEAD * look) * lean(0.3 * IDLE_HEAD * breath)),
        Part::Arm(side) => turn(out(side, IDLE_ARM * (0.5 + 0.5 * breath))),
        Part::Tail => turn(twist(TAIL_IDLE * (TAU * clock / (BREATH_SECS * 0.6)).sin())),
        Part::SpiderLeg { side, k } => turn(out(side, 0.05 * (TAU * clock / BREATH_SECS * 1.7 + k as f32).sin())),
        _ => Transform::IDENTITY,
    }
}

/// The motion that never stops: wings beating at `flap` (a phase in
/// radians) and fliers hovering. `alive` is 1, falling to 0 in death.
pub fn always(plan: Plan, part: Part, clock: f32, flap: f32, alive: f32) -> Transform {
    let hover = if plan.hovers() { HOVER_RISE * (TAU * clock / HOVER_SECS).sin() * alive } else { 0.0 };
    match (plan, part) {
        (Plan::Bat, Part::Body) => Transform::from_xyz(0.0, hover + 0.02 * flap.cos() * alive, 0.0),
        (Plan::Floater, Part::Torso) => Transform::from_xyz(0.0, hover, 0.0),
        (Plan::Bat, Part::Wing(side)) => turn(out(side, BAT_FLAP * flap.sin() * alive)),
        (_, Part::Wing(side)) => turn(twist(side.sign() * BACK_FLAP * flap.sin() * alive)),
        _ => Transform::IDENTITY,
    }
}

/// A part of a rigged model and its rest pose.
struct Bone {
    entity: Entity,
    part: Part,
    rest: Transform,
}

/// The posable parts of a creature's model scene (on its `ModelScene`).
#[derive(Component)]
pub struct Rig {
    plan: Plan,
    bow: bool,
    bones: Vec<Bone>,
    /// The weight of the idle layer, 0 to 1.
    idle: f32,
    /// The wing beat phase, in radians.
    flap: f32,
    /// This unit's offset into the idle clock, in seconds.
    seed: f32,
}

/// Observer: a creature's model scene is in; find its parts and rig it.
pub fn rig_model(
    ready: On<WorldInstanceReady>,
    mut commands: Commands,
    scenes: Query<&ChildOf, With<ModelScene>>,
    shapes: Query<&EntityShape>,
    children: Query<&Children>,
    // Mesh primitives may share a node's name; only the nodes are parts.
    nodes: Query<(&Name, &Transform, &ChildOf), Without<Mesh3d>>,
) {
    let Ok(owner) = scenes.get(ready.entity) else {
        return;
    };
    if shapes.get(owner.parent()) != Ok(&EntityShape::Creature) {
        return;
    }
    let named: Vec<_> = children
        .iter_descendants(ready.entity)
        .filter_map(|e| nodes.get(e).ok().map(|(name, rest, parent)| (e, name.as_str(), *rest, parent.parent())))
        .collect();
    let mut bones: Vec<Bone> = named
        .iter()
        .filter_map(|&(entity, name, rest, _)| Some(Bone { entity, part: Part::named(name)?, rest }))
        .collect();
    let mut bow = false;
    for &(entity, name, rest, parent) in &named {
        let arm = bones.iter().find_map(|b| match b.part {
            Part::Arm(side) if b.entity == parent => Some(side),
            _ => None,
        });
        if let (Some(side), None) = (arm, Part::named(name)) {
            bow |= name.ends_with("_bow");
            bones.push(Bone { entity, part: Part::Held(side), rest });
        }
    }
    let parts: Vec<_> = bones.iter().map(|b| b.part).collect();
    let Some(plan) = Plan::of(&parts) else {
        return;
    };
    let seed = (ready.entity.index().index() as f32 * 0.618_034).fract() * BREATH_SECS * 3.0;
    commands.entity(ready.entity).insert(Rig { plan, bow, bones, idle: 1.0, flap: seed, seed });
}

/// Poses every rig in view from the beat its unit plays (after `play_beats`).
pub fn animate(
    time: Res<Time>,
    presentation: Res<Presentation>,
    mut rigs: Query<(&mut Rig, &ChildOf, &InheritedVisibility)>,
    mut transforms: Query<&mut Transform>,
) {
    let dt = time.delta_secs();
    let ease = 1.0 - (-dt / IDLE_BLEND_SECS).exp();
    for (mut rig, owner, visible) in &mut rigs {
        if !visible.get() {
            continue;
        }
        let motion = presentation.playing(owner.parent()).map_or(Motion::Idle, |(beat, t)| Motion::of(beat, t));
        let target = if motion == Motion::Idle { 1.0 } else { 0.0 };
        rig.idle += (target - rig.idle) * ease;
        let hz = match rig.plan {
            Plan::Bat => BAT_FLAP_HZ,
            _ => BACK_FLAP_HZ,
        };
        rig.flap = (rig.flap + TAU * dt * (hz.1 + (hz.0 - hz.1) * rig.idle)) % TAU;
        let alive = match motion {
            Motion::Die(p) => 1.0 - smoothstep(p / DEATH_FALL),
            _ => 1.0,
        };
        let clock = time.elapsed_secs() + rig.seed;
        for bone in &rig.bones {
            let Ok(mut transform) = transforms.get_mut(bone.entity) else {
                continue;
            };
            let moved = pose(rig.plan, bone.part, motion, rig.bow);
            let idle = idle(rig.plan, bone.part, clock);
            let always = always(rig.plan, bone.part, clock, rig.flap, alive);
            let posed = Transform {
                translation: bone.rest.translation + moved.translation + idle.translation * rig.idle + always.translation,
                rotation: bone.rest.rotation
                    * moved.rotation
                    * Quat::IDENTITY.slerp(idle.rotation, rig.idle)
                    * always.rotation,
                scale: bone.rest.scale,
            };
            transform.set_if_neq(posed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIMBS: [Part; 4] = [Part::Arm(Side::L), Part::Arm(Side::R), Part::Leg(Side::L), Part::Leg(Side::R)];

    fn at_rest(t: Transform) -> bool {
        t.translation.length() < 1e-4 && t.rotation.angle_between(Quat::IDENTITY) < 1e-4
    }

    /// Where a hanging limb's tip goes (forward is +z).
    fn tip(t: Transform) -> Vec3 {
        t.rotation * Vec3::NEG_Y
    }

    #[test]
    fn parts_are_found_by_name() {
        assert_eq!(Part::named("hero_fighter_arm_r"), Some(Part::Arm(Side::R)));
        assert_eq!(Part::named("orc_warrior_leg_l"), Some(Part::Leg(Side::L)));
        assert_eq!(Part::named("demon_lord_wing_l"), Some(Part::Wing(Side::L)));
        assert_eq!(Part::named("rat_leg_br"), Some(Part::Paw { front: false, side: Side::R }));
        assert_eq!(Part::named("giant_spider_leg_l3"), Some(Part::SpiderLeg { side: Side::L, k: 2 }));
        assert_eq!(Part::named("giant_bat_body"), Some(Part::Body));
        assert_eq!(Part::named("hero_ranger_bow"), None);
        assert_eq!(Part::named("giant_spider_leg_l0"), None);
    }

    #[test]
    fn the_plan_comes_from_the_parts() {
        use Part::*;
        assert_eq!(Plan::of(&[Torso, Head, Leg(Side::L), Arm(Side::R)]), Some(Plan::Biped));
        assert_eq!(Plan::of(&[Torso, Head, Arm(Side::R)]), Some(Plan::Floater));
        assert_eq!(Plan::of(&[Body, Head, Paw { front: true, side: Side::L }]), Some(Plan::Quadruped));
        assert_eq!(Plan::of(&[Body, Head, Wing(Side::L)]), Some(Plan::Bat));
        assert_eq!(Plan::of(&[Body, SpiderLeg { side: Side::R, k: 0 }]), Some(Plan::Spider));
        assert_eq!(Plan::of(&[Head]), None);
    }

    #[test]
    fn walking_legs_and_arms_swing_in_opposition() {
        let walk = Motion::Walk { tiles: 0.5, speed: 1.0 };
        let leg = |s| tip(pose(Plan::Biped, Part::Leg(s), walk, false)).z;
        let arm = |s| tip(pose(Plan::Biped, Part::Arm(s), walk, false)).z;
        assert!(leg(Side::L) > 0.3 && leg(Side::R) < -0.3, "{} {}", leg(Side::L), leg(Side::R));
        assert!(arm(Side::L) < -0.3 && arm(Side::R) > 0.3, "the arms counter the legs");
        // The next tile is the other foot's.
        let next = Motion::Walk { tiles: 1.5, speed: 1.0 };
        assert!(tip(pose(Plan::Biped, Part::Leg(Side::L), next, false)).z < -0.3);
    }

    #[test]
    fn quadrupeds_trot_on_diagonal_pairs() {
        let walk = Motion::Walk { tiles: 0.5, speed: 1.0 };
        let paw = |front, side| tip(pose(Plan::Quadruped, Part::Paw { front, side }, walk, false)).z;
        assert!(paw(true, Side::L) > 0.3 && paw(false, Side::R) > 0.3);
        assert!(paw(true, Side::R) < -0.3 && paw(false, Side::L) < -0.3);
    }

    #[test]
    fn a_walk_is_at_rest_on_every_tile_and_at_both_ends() {
        for tiles in [0.0, 1.0, 2.0, 5.0] {
            for part in LIMBS {
                assert!(at_rest(pose(Plan::Biped, part, Motion::Walk { tiles, speed: 0.0 }, false)), "{part:?} at {tiles}");
            }
            let leg = Part::SpiderLeg { side: Side::L, k: 1 };
            assert!(at_rest(pose(Plan::Spider, leg, Motion::Walk { tiles: tiles + 0.3, speed: 0.0 }, false)));
        }
        // A real glide: still at its start and its end, moving between.
        let u = World::new().spawn_empty().id();
        let beat = Beat::Move { unit: u, cells: vec![(0, 0), (1, 0), (2, 0)] };
        let Motion::Walk { tiles, speed } = Motion::of(&beat, 0.0) else { panic!() };
        assert!(tiles == 0.0 && speed < 0.2, "{tiles} {speed}");
        let Motion::Walk { tiles, speed } = Motion::of(&beat, beat.duration()) else { panic!() };
        assert!((tiles - 2.0).abs() < 1e-4 && speed < 0.2, "{tiles} {speed}");
        let Motion::Walk { speed, .. } = Motion::of(&beat, beat.duration() / 2.0) else { panic!() };
        assert!((speed - 1.0).abs() < 1e-3);
    }

    #[test]
    fn every_beat_motion_starts_and_ends_at_rest() {
        let plans = [Plan::Biped, Plan::Floater, Plan::Quadruped, Plan::Bat, Plan::Spider];
        let parts = [
            Part::Torso,
            Part::Head,
            Part::Arm(Side::L),
            Part::Arm(Side::R),
            Part::Held(Side::L),
            Part::Held(Side::R),
            Part::Body,
            Part::Tail,
            Part::Paw { front: true, side: Side::L },
            Part::SpiderLeg { side: Side::L, k: 0 },
            Part::Wing(Side::R),
        ];
        for motion in [Motion::Strike, Motion::Shoot, Motion::Cast, Motion::Flinch] {
            for plan in plans {
                for part in parts {
                    for bow in [false, true] {
                        assert!(at_rest(pose(plan, part, motion(0.0), bow)), "{plan:?} {part:?} {:?}", motion(0.0));
                        assert!(at_rest(pose(plan, part, motion(1.0), bow)), "{plan:?} {part:?} {:?}", motion(1.0));
                    }
                }
            }
        }
    }

    #[test]
    fn the_blow_lands_half_way_through_the_lunge() {
        let arm = |p| tip(pose(Plan::Biped, Part::Arm(Side::R), Motion::Strike(p), false));
        // Raised overhead in the wind-up, then down in front as the blow lands.
        assert!(arm(0.3).y > 0.5, "{}", arm(0.3));
        let blow = arm(0.5);
        assert!(blow.z > 0.7, "{blow}");
        // Half way is where the arm stops coming down: the swing is fastest just before.
        assert!((arm(0.49) - blow).length() < (arm(0.4) - arm(0.41)).length());
        // The blow lands at the beat's hit: the lunge's hold.
        let u = World::new().spawn_empty().id();
        let lunge = Beat::Lunge { unit: u, from: (0, 0), to: (1, 0) };
        assert_eq!(Motion::of(&lunge, lunge.hold()), Motion::Strike(0.5));
    }

    #[test]
    fn a_shot_raises_the_bow_arm_and_keeps_the_bow_upright() {
        let p = DRAW_SECS / (DRAW_SECS + PROJECTILE_SECS);
        let arm = pose(Plan::Biped, Part::Arm(Side::L), Motion::Shoot(p), true);
        assert!(tip(arm).z > 0.9, "held out level in front");
        let bow = pose(Plan::Biped, Part::Held(Side::L), Motion::Shoot(p), true);
        assert!((arm.rotation * bow.rotation).angle_between(Quat::IDENTITY) < 1e-4);
        // Without a bow (a staff), the right arm aims.
        assert!(tip(pose(Plan::Biped, Part::Arm(Side::R), Motion::Shoot(p), false)).z > 0.9);
    }

    #[test]
    fn a_hit_knocks_the_torso_back_and_death_lets_the_arms_fall_open() {
        let torso = pose(Plan::Biped, Part::Torso, Motion::Flinch(0.5), false);
        assert!((torso.rotation * Vec3::Y).z < -0.2, "leans back, away from the attacker it faces");
        let dead = |side| tip(pose(Plan::Biped, Part::Arm(side), Motion::Die(1.0), false));
        assert!(dead(Side::L).x > 0.6 && dead(Side::R).x < -0.6);
    }

    /// The real model files, headless: each kit's body plans are found.
    #[test]
    fn the_models_are_rigged() {
        use crate::models::{AvailableModels, ModelsPlugin};
        use bevy::render::{settings::WgpuSettings, RenderPlugin};
        let mut app = App::new();
        app.add_plugins((
            DefaultPlugins
                .set(RenderPlugin { render_creation: WgpuSettings { backends: None, ..default() }.into(), ..default() })
                .disable::<bevy::winit::WinitPlugin>(),
            ModelsPlugin,
        ))
        .add_observer(rig_model);
        app.finish();
        app.cleanup();
        let models = ["hero_ranger", "hero_fighter", "wraith", "rat", "giant_bat", "giant_spider", "demon_lord"];
        for name in models {
            let scene = app.world().resource::<AvailableModels>().scene(name).expect("the model is committed");
            app.world_mut().spawn((EntityShape::Creature, Transform::default(), Visibility::default())).with_child((ModelScene, scene));
        }
        let mut rigs = app.world_mut().query::<&Rig>();
        for _ in 0..500 {
            if rigs.iter(app.world()).count() == models.len() {
                break;
            }
            app.update();
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let mut found: Vec<_> = rigs.iter(app.world()).map(|r| (r.plan, r.bow, r.bones.len())).collect();
        found.sort_by_key(|f| format!("{f:?}"));
        assert_eq!(
            found,
            vec![
                (Plan::Bat, false, 4),
                (Plan::Biped, false, 10),
                (Plan::Biped, false, 8),
                (Plan::Biped, true, 7),
                (Plan::Floater, false, 4),
                (Plan::Quadruped, false, 7),
                (Plan::Spider, false, 9),
            ]
        );
    }

    #[test]
    fn idle_and_wings_keep_moving_and_the_dead_stop() {
        let a = idle(Plan::Biped, Part::Torso, 0.0).translation.y;
        let b = idle(Plan::Biped, Part::Torso, BREATH_SECS / 4.0).translation.y;
        assert!((b - a - BREATH_RISE).abs() < 1e-5);
        let wing = |alive| always(Plan::Bat, Part::Wing(Side::L), 0.0, PI / 2.0, alive).rotation;
        assert!(wing(1.0).angle_between(Quat::IDENTITY) > 0.5);
        assert!(wing(0.0).angle_between(Quat::IDENTITY) < 1e-5);
    }
}
