//! How the rules' results play out on screen. The rules stay instant and
//! tile-based (`Position` changes at once); the systems where things happen
//! push typed `Beat`s onto the `Presentation` queue, and `play_beats` plays
//! them over time on the 3D bodies. It owns the transform of every creature
//! (`EntityShape::Creature`): a creature no beat names rests on its tile.
//!
//! Ordering (`holds`): a beat starts once every earlier beat has played its
//! `Beat::hold`. Floaters and deaths hold nothing, so they overlap what
//! follows; a lunge holds until it strikes (half way); a unit's beats never
//! overlap each other; and moves pushed while exploring overlap the moves of
//! other units, so the party and the monsters walk together. A move that
//! starts where the same unit's queued move ends extends it instead, so a
//! click walk glides as one continuous path.
//!
//! Facing (`heading`): a creature's model turns, eased over `TURN_SECS`, to
//! the way it last moved, struck, or was struck from. Only the model scene
//! child turns, so a hero's torch stays on the camera side.
//!
//! All animation runs on `Time`, so harness captures are the same every run.

use std::f32::consts::PI;

use bevy::prelude::*;
use bevy::transform::TransformSystems;
use bevy::ui::UiSystems;

use crate::combat::Outcome;
use crate::effects::{EffectKind, Effects};
use crate::entity_render::{EntityShape, ModelScene};
use crate::game_flow::Mode;
use crate::map::{grid_to_world, Position};
use crate::player;
use crate::resources::UiFont;
use crate::RunState;

/// Walking speed along a path, in tiles per second.
pub const GLIDE_SPEED: f32 = 9.0;
/// Distance at each end of a glide over which it eases in or out, in tiles.
pub const GLIDE_EASE: f32 = 0.25;
/// How far before and after a corner a path starts rounding it, in tiles.
pub const CORNER_RADIUS: f32 = 0.3;
/// The little hop of each step, in world units.
pub const HOP_HEIGHT: f32 = 0.06;
/// Exploring, a click walk takes its next step once the walker has no more
/// than this many tiles left to glide, so steps chain with no stop.
pub const WALK_LEAD: f32 = 1.5;
/// A melee lunge out and back; the blow lands half way.
pub const LUNGE_SECS: f32 = 0.2;
pub const LUNGE_REACH: f32 = 0.35;
/// The missile bolt's lead piece lands at 1/1.3 of its 0.45 s life (`effects.rs`).
pub const PROJECTILE_SECS: f32 = 0.35;
pub const FLOATER_SECS: f32 = 0.9;
/// How far a floater rises over its life, in world units.
pub const FLOATER_RISE: f32 = 0.6;
/// A floater that starts while others show over the same tile sits this much
/// higher per floater, so they do not overprint, in world units.
pub const FLOATER_STACK: f32 = 0.7;
pub const FLINCH_SECS: f32 = 0.2;
pub const FLINCH_REACH: f32 = 0.15;
pub const DEATH_SECS: f32 = 0.6;
/// The part of a death spent falling over; the rest fades and sinks.
pub const DEATH_FALL: f32 = 0.5;
/// Hold for the camera to glide to off-screen action (most of the way at
/// `camera::FOLLOW_TIME`). Action already on screen does not wait.
pub const CAMERA_FOCUS_SECS: f32 = 0.4;
/// Action closer than this share of the screen to an edge counts as off-screen.
pub const CAMERA_FOCUS_MARGIN: f32 = 0.1;
/// A pause after each monster that acted in combat.
pub const BETWEEN_UNITS_SECS: f32 = 0.15;
/// Time constant of a creature turning to face its way (exponential ease).
pub const TURN_SECS: f32 = 0.08;

/// One visual step of what the rules just did. Cells are grid cells; each
/// beat carries all it needs, so playing it never reads `Position`.
#[derive(Clone, Debug, PartialEq)]
pub enum Beat {
    /// Glide along `cells`, the first being where the unit stands.
    Move { unit: Entity, cells: Vec<(i32, i32)> },
    /// A melee blow: out towards `to` and back.
    Lunge { unit: Entity, from: (i32, i32), to: (i32, i32) },
    /// A missile bolt from `from` to `to`.
    Projectile { from: (i32, i32), to: (i32, i32) },
    /// Text over the target: miss, hit or critical, and the damage.
    Floater { at: (i32, i32), outcome: Outcome, damage: i32 },
    /// Knocked back from `from` with a spark.
    Flinch { unit: Entity, at: (i32, i32), from: (i32, i32) },
    /// The stand-in of a dead unit falls over and fades, then is despawned.
    Death { unit: Entity, at: (i32, i32) },
    /// Glide the camera to `at` if it is off-screen, until the queue is idle.
    CameraFocus { at: (i32, i32) },
    Wait { secs: f32 },
}

impl Beat {
    /// Seconds the beat plays for.
    pub fn duration(&self) -> f32 {
        match self {
            Beat::Move { cells, .. } => glide_secs(path_length(cells)),
            Beat::Lunge { .. } => LUNGE_SECS,
            Beat::Projectile { .. } => PROJECTILE_SECS,
            Beat::Floater { .. } => FLOATER_SECS,
            Beat::Flinch { .. } => FLINCH_SECS,
            Beat::Death { .. } => DEATH_SECS,
            Beat::CameraFocus { .. } => CAMERA_FOCUS_SECS,
            Beat::Wait { secs } => *secs,
        }
    }

    /// Seconds later beats wait on this one once it starts.
    pub fn hold(&self) -> f32 {
        match self {
            Beat::Floater { .. } | Beat::Death { .. } => 0.0,
            Beat::Lunge { .. } => LUNGE_SECS / 2.0,
            _ => self.duration(),
        }
    }

    /// The body this beat moves, if any.
    fn unit(&self) -> Option<Entity> {
        match self {
            Beat::Move { unit, .. } | Beat::Lunge { unit, .. } | Beat::Flinch { unit, .. } | Beat::Death { unit, .. } => {
                Some(*unit)
            }
            _ => None,
        }
    }
}

/// A queued beat and how far it has played (`None` until it starts).
#[derive(Clone, Debug, PartialEq)]
struct Playing {
    beat: Beat,
    elapsed: Option<f32>,
    /// Pushed while exploring: its moves overlap other units' moves.
    together: bool,
    /// A floater's text entity.
    spawned: Option<Entity>,
    /// Its slot above the tile, so floaters showing together do not overprint.
    stack: usize,
}

/// Whether `earlier` (still in the queue) keeps `later` from starting.
fn holds(earlier: &Playing, later: &Playing) -> bool {
    let (a, b) = (earlier.beat.unit(), later.beat.unit());
    if a.is_some() && a == b {
        return true;
    }
    if let (Beat::Move { .. }, Beat::Move { .. }) = (&earlier.beat, &later.beat) {
        if earlier.together && later.together {
            return false;
        }
    }
    earlier.elapsed.unwrap_or(0.0) < earlier.beat.hold()
}

/// The presentation queue. Rules systems `push`; `play_beats` plays.
#[derive(Resource, Default)]
pub struct Presentation {
    beats: Vec<Playing>,
    /// The mode is explore: moves pushed now overlap other units' moves.
    explore: bool,
}

impl Presentation {
    pub fn push(&mut self, beat: Beat) {
        let new = Playing { beat, elapsed: None, together: self.explore, spawned: None, stack: 0 };
        if let Beat::Move { unit, cells } = &new.beat {
            if cells.len() < 2 {
                return;
            }
            // Extend the unit's last move if nothing in between holds the new one.
            let mut extend = None;
            for (i, earlier) in self.beats.iter().enumerate().rev() {
                if let Beat::Move { unit: u, cells: c } = &earlier.beat {
                    if u == unit {
                        if c.last() == cells.first() && earlier.together == new.together {
                            extend = Some(i);
                        }
                        break;
                    }
                }
                if holds(earlier, &new) {
                    break;
                }
            }
            if let Some(i) = extend {
                if let Beat::Move { cells: c, .. } = &mut self.beats[i].beat {
                    c.extend_from_slice(&cells[1..]);
                }
                return;
            }
        }
        self.beats.push(new);
    }

    /// Push a glide along `cells` for `unit`.
    pub fn push_move(&mut self, unit: Entity, cells: &[(i32, i32)]) {
        self.push(Beat::Move { unit, cells: cells.to_vec() });
    }

    /// Something that holds input is still playing (floaters and deaths do not).
    pub fn busy(&self) -> bool {
        self.beats.iter().any(|p| p.elapsed.unwrap_or(0.0) < p.beat.hold())
    }

    /// Tiles `unit` still has to glide.
    pub fn glide_left(&self, unit: Entity) -> f32 {
        self.beats
            .iter()
            .filter_map(|p| match &p.beat {
                Beat::Move { unit: u, cells } if *u == unit => {
                    let length = path_length(cells);
                    Some(length - glide_distance(p.elapsed.unwrap_or(0.0), length))
                }
                _ => None,
            })
            .sum()
    }

    /// Finish every playing beat that holds input (any key or click in combat).
    pub fn skip(&mut self) {
        for p in &mut self.beats {
            if let Some(elapsed) = &mut p.elapsed {
                if p.beat.hold() > 0.0 {
                    *elapsed = elapsed.max(p.beat.duration());
                }
            }
        }
    }

    /// Ages the started beats by `dt` and takes out the finished ones.
    fn tick(&mut self, dt: f32) -> Vec<Playing> {
        for p in &mut self.beats {
            if let Some(elapsed) = &mut p.elapsed {
                *elapsed += dt;
            }
        }
        let (done, left) = std::mem::take(&mut self.beats)
            .into_iter()
            .partition(|p| p.elapsed.is_some_and(|e| e >= p.beat.duration()));
        self.beats = left;
        done
    }

    /// Starts every beat the ordering rule lets start; returns their indices.
    fn start_ready(&mut self) -> Vec<usize> {
        let mut started = Vec::new();
        for i in 0..self.beats.len() {
            if self.beats[i].elapsed.is_none() && self.beats[..i].iter().all(|e| !holds(e, &self.beats[i])) {
                self.beats[i].elapsed = Some(0.0);
                started.push(i);
            }
        }
        started
    }

    fn involves(&self, unit: Entity) -> bool {
        self.beats.iter().any(|p| p.beat.unit() == Some(unit))
    }

    /// Re-aims queued beats at a dead unit's stand-in.
    fn rename(&mut self, from: Entity, to: Entity) {
        for p in &mut self.beats {
            if let Beat::Move { unit, .. } | Beat::Lunge { unit, .. } | Beat::Flinch { unit, .. } | Beat::Death { unit, .. } =
                &mut p.beat
            {
                if *unit == from {
                    *unit = to;
                }
            }
        }
    }
}

/// The body of a dead unit while its fall plays; despawned at the end.
#[derive(Component)]
pub struct Fallen;

/// A floating text over a unit.
#[derive(Component)]
struct FloaterText;

/// The text and colour of a floater for an attack's outcome and damage.
pub fn floater_text(outcome: Outcome, damage: i32) -> (String, Color) {
    match outcome {
        Outcome::Miss => ("Miss".into(), Color::srgb(0.7, 0.75, 0.85)),
        _ if damage <= 0 => ("No damage".into(), Color::srgb(0.7, 0.7, 0.7)),
        Outcome::Hit => (damage.to_string(), Color::srgb(1.0, 0.95, 0.6)),
        Outcome::Crit => (format!("Crit! {damage}"), Color::srgb(1.0, 0.45, 0.2)),
    }
}

/// Grid distance along `cells` (diagonal steps are √2).
pub fn path_length(cells: &[(i32, i32)]) -> f32 {
    cells.windows(2).map(|w| cell(w[0]).distance(cell(w[1]))).sum()
}

fn cell(c: (i32, i32)) -> Vec2 {
    Vec2::new(c.0 as f32, c.1 as f32)
}

/// The grid point `s` tiles along `cells`, corners rounded off within
/// `CORNER_RADIUS`. Exactly the first cell at 0 and the last at the end.
pub fn path_point(cells: &[(i32, i32)], s: f32) -> Vec2 {
    let Some(&first) = cells.first() else {
        return Vec2::ZERO;
    };
    let last = cells.len().saturating_sub(2);
    let mut start = 0.0;
    for i in 0..cells.len().saturating_sub(1) {
        let (a, b) = (cell(cells[i]), cell(cells[i + 1]));
        let len = a.distance(b);
        if s <= start + len || i == last {
            let local = (s - start).clamp(0.0, len);
            if i + 2 < cells.len() {
                let next = cell(cells[i + 2]);
                let r = corner_radius(len, b.distance(next));
                if local > len - r {
                    return corner(a, b, next, r, local - (len - r));
                }
            }
            if i > 0 {
                let prev = cell(cells[i - 1]);
                let r = corner_radius(prev.distance(a), len);
                if local < r {
                    return corner(prev, a, b, r, r + local);
                }
            }
            return if len > 0.0 { a.lerp(b, local / len) } else { b };
        }
        start += len;
    }
    cell(first)
}

fn corner_radius(len_in: f32, len_out: f32) -> f32 {
    CORNER_RADIUS.min(len_in / 2.0).min(len_out / 2.0)
}

/// A quadratic curve round the corner at `c`, from `r` before it to `r` after;
/// `d` runs 0 to 2r along it.
fn corner(prev: Vec2, c: Vec2, next: Vec2, r: f32, d: f32) -> Vec2 {
    let p0 = c + (prev - c).normalize() * r;
    let p2 = c + (next - c).normalize() * r;
    let u = d / (2.0 * r);
    p0 * (1.0 - u) * (1.0 - u) + c * 2.0 * u * (1.0 - u) + p2 * u * u
}

fn glide_accel() -> f32 {
    GLIDE_SPEED * GLIDE_SPEED / (2.0 * GLIDE_EASE)
}

/// The eased stretch at each end and the top speed of a glide of `length`.
fn ease_and_top(length: f32) -> (f32, f32) {
    let ease = GLIDE_EASE.min(length / 2.0);
    (ease, (2.0 * glide_accel() * ease).sqrt())
}

/// Seconds to glide `length` tiles: constant speed, easing in and out.
pub fn glide_secs(length: f32) -> f32 {
    if length <= 0.0 {
        return 0.0;
    }
    let (ease, top) = ease_and_top(length);
    2.0 * top / glide_accel() + (length - 2.0 * ease) / top
}

/// Tiles covered `t` seconds into a glide of `length`. A longer glide
/// matches a shorter one up to where the shorter starts slowing down, so
/// extending a playing glide does not jolt it.
pub fn glide_distance(t: f32, length: f32) -> f32 {
    if length <= 0.0 {
        return 0.0;
    }
    let a = glide_accel();
    let (ease, top) = ease_and_top(length);
    let total = glide_secs(length);
    let speed_up = top / a;
    let t = t.clamp(0.0, total);
    if t < speed_up {
        0.5 * a * t * t
    } else if t > total - speed_up {
        length - 0.5 * a * (total - t) * (total - t)
    } else {
        ease + top * (t - speed_up)
    }
}

fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The yaw that turns a model facing south (+z) to face along `dir`, a grid
/// direction (x east, y south). `None` for no direction.
pub fn yaw_towards(dir: Vec2) -> Option<f32> {
    (dir.length_squared() > 1e-6).then(|| dir.x.atan2(dir.y))
}

/// Which way the beat's body faces `t` seconds in: along its path, at what
/// it strikes, or at what struck it.
fn heading(beat: &Beat, t: f32) -> Option<(Entity, f32)> {
    let (unit, dir) = match beat {
        Beat::Move { unit, cells } => {
            let length = path_length(cells);
            let s = glide_distance(t, length);
            let probe = 0.05;
            (*unit, path_point(cells, (s + probe).min(length)) - path_point(cells, (s - probe).max(0.0)))
        }
        Beat::Lunge { unit, from, to } => (*unit, cell(*to) - cell(*from)),
        Beat::Flinch { unit, at, from } => (*unit, cell(*from) - cell(*at)),
        _ => return None,
    };
    yaw_towards(dir).map(|yaw| (unit, yaw))
}

/// The way a creature's model turns to (`heading`).
#[derive(Component)]
pub struct Facing(pub f32);

fn floor(c: (i32, i32)) -> Vec3 {
    grid_to_world(c.0, c.1)
}

/// How far a death has faded, 0 to 1.
fn death_fade(t: f32) -> f32 {
    ((t / DEATH_SECS - DEATH_FALL) / (1.0 - DEATH_FALL)).clamp(0.0, 1.0)
}

/// Where the beat puts its body `t` seconds in.
fn body_pose(beat: &Beat, t: f32) -> Option<(Entity, Transform)> {
    let lift = Vec3::Y * EntityShape::Creature.lift();
    let wave = |secs: f32| (PI * (t / secs).clamp(0.0, 1.0)).sin();
    let pose = match beat {
        Beat::Move { unit, cells } => {
            let length = path_length(cells);
            let s = glide_distance(t, length);
            let p = path_point(cells, s);
            let steps = (cells.len() - 1) as f32;
            let hop = HOP_HEIGHT * (PI * s * steps / length.max(f32::EPSILON)).sin().abs();
            (*unit, Transform::from_translation(Vec3::new(p.x, 0.0, p.y) + lift + Vec3::Y * hop))
        }
        Beat::Lunge { unit, from, to } => {
            let dir = (floor(*to) - floor(*from)).normalize_or_zero();
            (*unit, Transform::from_translation(floor(*from) + lift + dir * LUNGE_REACH * wave(LUNGE_SECS)))
        }
        Beat::Flinch { unit, at, from } => {
            let dir = (floor(*at) - floor(*from)).normalize_or_zero();
            (*unit, Transform::from_translation(floor(*at) + lift + dir * FLINCH_REACH * wave(FLINCH_SECS)))
        }
        Beat::Death { unit, at } => {
            // Falls towards screen right: the camera looks along -x, -z.
            let fall = smoothstep(t / (DEATH_SECS * DEATH_FALL)) * PI / 2.0;
            let rotation = Quat::from_axis_angle(Vec3::new(-1.0, 0.0, -1.0).normalize(), fall);
            let lying = Vec3::Y * 0.3 * fall.sin();
            let sink = Vec3::Y * 0.4 * death_fade(t);
            let centre = floor(*at) + rotation * lift + lying - sink;
            (*unit, Transform::from_translation(centre).with_rotation(rotation))
        }
        _ => return None,
    };
    Some(pose)
}

pub struct PresentationPlugin;

impl Plugin for PresentationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Presentation>()
            .add_systems(OnEnter(RunState::PreRun), reset)
            .add_systems(OnEnter(RunState::NextLevel), reset)
            .add_systems(Update, skip_on_input.after(player::handle_player_input))
            .add_systems(
                PostUpdate,
                (play_beats, turn_models).chain().before(TransformSystems::Propagate).before(UiSystems::Prepare),
            );
    }
}

/// In combat, any key or click finishes what is playing. Runs after the map
/// input, which ignores input while busy, so the press is not also an order.
fn skip_on_input(
    mode: Res<Mode>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut presentation: ResMut<Presentation>,
) {
    let pressed = keys.get_just_pressed().next().is_some() || mouse.get_just_pressed().next().is_some();
    if pressed && matches!(*mode, Mode::Combat { .. }) && presentation.busy() {
        presentation.skip();
    }
}

/// A new level or game: nothing in flight carries over.
fn reset(
    mut commands: Commands,
    mut presentation: ResMut<Presentation>,
    leftovers: Query<Entity, Or<(With<Fallen>, With<FloaterText>)>>,
) {
    presentation.beats.clear();
    for entity in &leftovers {
        commands.entity(entity).despawn();
    }
}

/// What a body is made of, to copy onto a dead unit's stand-in.
pub type BodyParts<'w, 's> = Query<
    'w,
    's,
    (
        &'static Transform,
        &'static Visibility,
        Option<&'static Mesh3d>,
        Option<&'static MeshMaterial3d<StandardMaterial>>,
        Option<&'static Children>,
    ),
>;

/// The rules just removed `unit`: give it a stand-in body that keeps playing
/// its queued beats, then falls (`Beat::Death`) and is despawned. Call
/// before despawning `unit`.
pub fn fall(commands: &mut Commands, presentation: &mut Presentation, bodies: &BodyParts, unit: Entity, at: (i32, i32)) {
    let Ok((transform, visibility, mesh, material, children)) = bodies.get(unit) else {
        return;
    };
    let mut stand_in = commands.spawn((Fallen, *transform, *visibility));
    if let (Some(mesh), Some(material)) = (mesh, material) {
        stand_in.insert((mesh.clone(), material.clone()));
    }
    let stand_in = stand_in.id();
    // The model scene and a hero's torch go with the body.
    for &child in children.into_iter().flatten() {
        commands.entity(child).insert(ChildOf(stand_in));
    }
    presentation.rename(unit, stand_in);
    presentation.push(Beat::Death { unit: stand_in, at });
}

type Bodies<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static mut Transform, Option<&'static Position>, Option<&'static EntityShape>),
    Without<Camera3d>,
>;

fn place(bodies: &mut Bodies, beat: &Beat, t: f32) {
    if let Some((unit, pose)) = body_pose(beat, t) {
        if let Ok((_, mut transform, ..)) = bodies.get_mut(unit) {
            *transform = pose;
        }
    }
}

/// Plays the queue: ages and starts beats, sets every body's transform,
/// moves floaters, and rests creatures no beat names on their tile.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn play_beats(
    mut commands: Commands,
    time: Res<Time>,
    mode: Res<Mode>,
    font: Res<UiFont>,
    mut presentation: ResMut<Presentation>,
    mut effects: ResMut<Effects>,
    mut follow: ResMut<crate::camera::Camera>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut bodies: Bodies,
    mut skins: Query<&mut MeshMaterial3d<StandardMaterial>>,
    mut facings: Query<&mut Facing>,
    mut floaters: Query<(&mut Node, &mut TextColor), With<FloaterText>>,
    camera: Query<(&Camera, &Transform), With<Camera3d>>,
) {
    for done in presentation.tick(time.delta_secs()) {
        place(&mut bodies, &done.beat, done.beat.duration());
        if let Some(text) = done.spawned {
            commands.entity(text).try_despawn();
        }
        if let Beat::Death { unit, .. } = done.beat {
            commands.entity(unit).try_despawn();
        }
    }

    let view = camera.single().ok().map(|(camera, transform)| (camera, GlobalTransform::from(*transform)));
    for i in presentation.start_ready() {
        match presentation.beats[i].beat.clone() {
            Beat::Projectile { from, to } => effects.play(EffectKind::MagicMissile { from }, to.0, to.1),
            Beat::Flinch { at, .. } => effects.play(EffectKind::HitSpark, at.0, at.1),
            Beat::CameraFocus { at } => {
                let on_screen = view.as_ref().is_some_and(|(camera, rig)| {
                    let size = camera.logical_viewport_size().unwrap_or_default();
                    let margin = size * CAMERA_FOCUS_MARGIN;
                    camera.world_to_viewport(rig, floor(at)).is_ok_and(|p| p.cmpge(margin).all() && p.cmple(size - margin).all())
                });
                if on_screen {
                    presentation.beats[i].elapsed = Some(CAMERA_FOCUS_SECS);
                } else {
                    follow.look_at(Some(cell(at)));
                }
            }
            Beat::Floater { at, outcome, damage } => {
                presentation.beats[i].stack = free_floater_slot(&presentation.beats, at);
                let (text, colour) = floater_text(outcome, damage);
                let size = if outcome == Outcome::Crit { 26.0 } else { 20.0 };
                let entity = commands
                    .spawn((
                        FloaterText,
                        Text::new(text),
                        TextFont { font: font.0.clone().into(), font_size: FontSize::Px(size), ..default() },
                        TextColor(colour),
                        TextLayout::justify(Justify::Center),
                        Node { position_type: PositionType::Absolute, width: Val::Px(FLOATER_WIDTH), ..default() },
                        Visibility::Hidden,
                    ))
                    .id();
                presentation.beats[i].spawned = Some(entity);
            }
            Beat::Death { unit, .. } => {
                // Its own see-through copy of the shared colour material, to fade.
                if let Ok(mut skin) = skins.get_mut(unit) {
                    if let Some(mut fading) = materials.get(&skin.0).cloned() {
                        fading.alpha_mode = AlphaMode::Blend;
                        skin.0 = materials.add(fading);
                    }
                }
            }
            _ => {}
        }
    }

    for playing in &presentation.beats {
        let Some(t) = playing.elapsed else {
            continue;
        };
        place(&mut bodies, &playing.beat, t);
        if let Some((unit, yaw)) = heading(&playing.beat, t) {
            match facings.get_mut(unit) {
                Ok(mut facing) => facing.0 = yaw,
                Err(_) => {
                    commands.entity(unit).try_insert(Facing(yaw));
                }
            }
        }
        match &playing.beat {
            Beat::Death { unit, .. } => {
                if let Ok(skin) = skins.get(*unit) {
                    if let Some(mut material) = materials.get_mut(&skin.0) {
                        material.base_color.set_alpha(1.0 - death_fade(t));
                    }
                }
            }
            Beat::Floater { at, .. } => {
                let (Some(text), Some((camera, rig))) = (playing.spawned, &view) else {
                    continue;
                };
                let Ok((mut node, mut colour)) = floaters.get_mut(text) else {
                    continue;
                };
                let u = t / FLOATER_SECS;
                let point = floor(*at) + Vec3::Y * (1.7 + FLOATER_RISE * u + FLOATER_STACK * playing.stack as f32);
                if let Ok(screen) = camera.world_to_viewport(rig, point) {
                    node.left = Val::Px(screen.x - FLOATER_WIDTH / 2.0);
                    node.top = Val::Px(screen.y - 12.0);
                    commands.entity(text).insert(Visibility::Inherited);
                }
                colour.0.set_alpha(1.0 - smoothstep((u - 0.6) / 0.4));
            }
            _ => {}
        }
    }

    for (unit, mut transform, pos, shape) in &mut bodies {
        let (Some(&pos), Some(&EntityShape::Creature)) = (pos, shape) else {
            continue;
        };
        let rest = floor((pos.x, pos.y)) + Vec3::Y * EntityShape::Creature.lift();
        if transform.translation != rest && !presentation.involves(unit) {
            transform.translation = rest;
        }
    }

    if !presentation.busy() {
        follow.look_at(None);
    }
    presentation.explore = *mode == Mode::Explore;
}

/// Eases each creature's model scene towards its `Facing`.
fn turn_models(
    time: Res<Time>,
    units: Query<(&Facing, &Children)>,
    mut scenes: Query<&mut Transform, With<ModelScene>>,
) {
    let ease = 1.0 - (-time.delta_secs() / TURN_SECS).exp();
    for (facing, children) in &units {
        let target = Quat::from_rotation_y(facing.0);
        for &child in children {
            if let Ok(mut transform) = scenes.get_mut(child) {
                if transform.rotation != target {
                    transform.rotation = transform.rotation.slerp(target, ease);
                }
            }
        }
    }
}

/// The lowest stack slot over `at` no showing floater holds.
fn free_floater_slot(beats: &[Playing], at: (i32, i32)) -> usize {
    let taken = |slot| {
        beats.iter().any(|p| p.spawned.is_some() && p.stack == slot && matches!(p.beat, Beat::Floater { at: a, .. } if a == at))
    };
    (0..).find(|&slot| !taken(slot)).unwrap_or(0)
}

/// Floaters are centred in a box this wide, in logical pixels.
const FLOATER_WIDTH: f32 = 200.0;

#[cfg(test)]
mod tests {
    use super::*;

    fn units(n: usize) -> Vec<Entity> {
        let mut world = World::new();
        (0..n).map(|_| world.spawn_empty().id()).collect()
    }

    fn close(a: Vec2, b: Vec2) -> bool {
        a.distance(b) < 1e-4
    }

    #[test]
    fn a_path_is_exact_at_both_ends_and_straight_between() {
        let cells = [(0, 0), (1, 0), (2, 0)];
        assert_eq!(path_length(&cells), 2.0);
        assert_eq!(path_point(&cells, 0.0), Vec2::new(0.0, 0.0));
        assert_eq!(path_point(&cells, 2.0), Vec2::new(2.0, 0.0));
        assert!(close(path_point(&cells, 1.5), Vec2::new(1.5, 0.0)));
        assert_eq!(path_point(&cells, 9.0), Vec2::new(2.0, 0.0), "clamped past the end");
        assert_eq!(path_point(&[(3, 4)], 0.5), Vec2::new(3.0, 4.0));
        assert_eq!(path_point(&[(3, 4), (3, 4)], 0.0), Vec2::new(3.0, 4.0), "a step in place is not NaN");
        assert!((path_length(&[(0, 0), (1, 1)]) - 2f32.sqrt()).abs() < 1e-6);
    }

    #[test]
    fn a_path_rounds_its_corners_without_a_jump() {
        // East then south: the corner at (1, 0) is cut inside its radius.
        let cells = [(0, 0), (1, 0), (1, 1)];
        let at_corner = path_point(&cells, 1.0);
        assert!(close(at_corner, Vec2::new(0.925, 0.075)), "{at_corner}");
        let mut last = path_point(&cells, 0.0);
        for i in 1..=200 {
            let p = path_point(&cells, i as f32 / 100.0);
            assert!(p.distance(last) < 0.02, "a jump at s = {}", i as f32 / 100.0);
            last = p;
        }
        assert_eq!(last, Vec2::new(1.0, 1.0));
    }

    #[test]
    fn a_glide_eases_in_holds_its_speed_and_lands_exactly() {
        // One tile: a quarter tile up to speed, half a tile at 9/s, a quarter down.
        assert!((glide_secs(1.0) - 1.5 / GLIDE_SPEED).abs() < 1e-6);
        assert_eq!(glide_distance(0.0, 1.0), 0.0);
        assert_eq!(glide_distance(glide_secs(1.0), 1.0), 1.0);
        assert_eq!(glide_distance(5.0, 1.0), 1.0);
        // Ten tiles take ten tiles' time plus one ease's worth.
        assert!((glide_secs(10.0) - 10.5 / GLIDE_SPEED).abs() < 1e-5);
        // A longer glide matches a shorter one until the shorter slows down,
        // so a walk that extends a playing glide does not jolt it.
        let t = 0.1;
        assert!((glide_distance(t, 2.0) - glide_distance(t, 6.0)).abs() < 1e-6);
        // A very short glide still lands.
        assert!((glide_distance(glide_secs(0.2), 0.2) - 0.2).abs() < 1e-6);
    }

    #[test]
    fn beat_durations_and_holds() {
        let u = units(1)[0];
        assert_eq!(Beat::Lunge { unit: u, from: (0, 0), to: (1, 0) }.duration(), LUNGE_SECS);
        assert_eq!(Beat::Lunge { unit: u, from: (0, 0), to: (1, 0) }.hold(), LUNGE_SECS / 2.0);
        assert_eq!(Beat::Projectile { from: (0, 0), to: (5, 0) }.hold(), PROJECTILE_SECS);
        let floater = Beat::Floater { at: (0, 0), outcome: Outcome::Hit, damage: 3 };
        assert_eq!((floater.duration(), floater.hold()), (FLOATER_SECS, 0.0));
        assert_eq!(Beat::Death { unit: u, at: (0, 0) }.hold(), 0.0);
        assert_eq!(Beat::Wait { secs: 0.4 }.duration(), 0.4);
        assert!((Beat::Move { unit: u, cells: vec![(0, 0), (1, 0)] }.duration() - glide_secs(1.0)).abs() < 1e-6);
    }

    /// Plays the queue in steps of `dt` and lists which beats start at each step.
    fn run(p: &mut Presentation, dt: f32, steps: usize) -> Vec<Vec<String>> {
        (0..steps)
            .map(|_| {
                p.tick(dt);
                let started = p.start_ready();
                started.iter().map(|&i| name(&p.beats[i].beat)).collect()
            })
            .collect()
    }

    fn name(beat: &Beat) -> String {
        format!("{beat:?}").split([' ', '{']).next().unwrap().to_string()
    }

    #[test]
    fn an_attack_plays_in_order_and_floaters_overlap_what_follows() {
        let u = units(2);
        let mut p = Presentation::default();
        p.push(Beat::Move { unit: u[0], cells: vec![(0, 0), (1, 0)] });
        p.push(Beat::Lunge { unit: u[0], from: (1, 0), to: (2, 0) });
        p.push(Beat::Floater { at: (2, 0), outcome: Outcome::Hit, damage: 4 });
        p.push(Beat::Flinch { unit: u[1], at: (2, 0), from: (1, 0) });
        p.push(Beat::Death { unit: u[1], at: (2, 0) });
        p.push(Beat::Wait { secs: 0.1 });
        let starts = run(&mut p, 0.05, 20);
        let order: Vec<_> = starts.iter().filter(|s| !s.is_empty()).cloned().collect();
        assert_eq!(
            order,
            vec![
                vec!["Move".to_string()],
                vec!["Lunge".into()],
                // The blow lands half way through the lunge.
                vec!["Floater".into(), "Flinch".into()],
                // A death and whatever follows it start together.
                vec!["Death".into(), "Wait".into()],
            ]
        );
    }

    #[test]
    fn exploring_moves_of_different_units_overlap_and_a_walk_extends_its_glide() {
        let u = units(2);
        let mut p = Presentation { explore: true, ..default() };
        p.push_move(u[0], &[(0, 0), (1, 0)]);
        p.push_move(u[1], &[(0, 1), (0, 0)]);
        assert_eq!(run(&mut p, 0.01, 1), vec![vec!["Move".to_string(), "Move".into()]]);
        // The next step of each joins its glide.
        p.push_move(u[0], &[(1, 0), (2, 0)]);
        p.push_move(u[1], &[(0, 0), (1, 0)]);
        assert_eq!(p.beats.len(), 2);
        assert_eq!(p.beats[0].beat, Beat::Move { unit: u[0], cells: vec![(0, 0), (1, 0), (2, 0)] });
        assert!((p.glide_left(u[0]) - (2.0 - glide_distance(0.0, 2.0))).abs() < 1e-6);
        // A step that does not start where the glide ends queues behind it.
        p.push_move(u[0], &[(5, 5), (6, 5)]);
        assert_eq!(p.beats.len(), 3);
    }

    #[test]
    fn in_combat_moves_play_one_after_another() {
        let u = units(2);
        let mut p = Presentation::default();
        p.push_move(u[0], &[(0, 0), (1, 0)]);
        p.push_move(u[1], &[(5, 0), (4, 0)]);
        let starts = run(&mut p, 0.05, 10);
        // The first glide takes 0.167 s, so the second starts on the fifth step.
        let moved = vec!["Move".to_string()];
        assert_eq!(starts[0], moved);
        assert!(starts[1..4].iter().all(Vec::is_empty));
        assert_eq!(starts[4], moved);
    }

    #[test]
    fn busy_holds_input_until_the_blocking_beats_end_and_skip_finishes_them() {
        let u = units(1);
        let mut p = Presentation::default();
        assert!(!p.busy());
        p.push(Beat::Floater { at: (0, 0), outcome: Outcome::Miss, damage: 0 });
        assert!(!p.busy(), "floaters do not hold input");
        p.push(Beat::Projectile { from: (0, 0), to: (4, 0) });
        p.push_move(u[0], &[(0, 0), (0, 1)]);
        assert!(p.busy());
        p.start_ready();
        p.skip();
        p.tick(0.0);
        assert_eq!(p.beats.len(), 2, "the projectile finished; the floater and the move are left");
        p.start_ready();
        assert!(p.busy(), "the move started");
        p.skip();
        p.tick(0.0);
        assert!(!p.busy());
    }

    #[test]
    fn a_stand_in_inherits_the_dead_units_beats() {
        let u = units(2);
        let mut p = Presentation::default();
        p.push(Beat::Flinch { unit: u[0], at: (1, 1), from: (0, 1) });
        p.rename(u[0], u[1]);
        assert_eq!(p.beats[0].beat, Beat::Flinch { unit: u[1], at: (1, 1), from: (0, 1) });
    }

    #[test]
    fn floaters_over_one_tile_take_the_lowest_free_slot() {
        let text = units(1)[0];
        let floater = |at, stack| Playing {
            beat: Beat::Floater { at, outcome: Outcome::Hit, damage: 1 },
            elapsed: Some(0.0),
            together: false,
            spawned: Some(text),
            stack,
        };
        assert_eq!(free_floater_slot(&[], (1, 1)), 0);
        assert_eq!(free_floater_slot(&[floater((1, 1), 0)], (1, 1)), 1);
        assert_eq!(free_floater_slot(&[floater((1, 1), 1)], (1, 1)), 0, "the bottom one ended");
        assert_eq!(free_floater_slot(&[floater((2, 1), 0)], (1, 1)), 0, "another tile");
    }

    #[test]
    fn floater_text_for_each_outcome() {
        assert_eq!(floater_text(Outcome::Miss, 0).0, "Miss");
        assert_eq!(floater_text(Outcome::Hit, 5).0, "5");
        assert_eq!(floater_text(Outcome::Crit, 9).0, "Crit! 9");
        assert_eq!(floater_text(Outcome::Hit, 0).0, "No damage");
        assert_eq!(floater_text(Outcome::Crit, 0).0, "No damage");
    }

    #[test]
    fn a_body_rests_exactly_on_its_tiles_at_the_ends_of_its_beats() {
        let u = units(1)[0];
        let lift = EntityShape::Creature.lift();
        let walk = Beat::Move { unit: u, cells: vec![(0, 0), (1, 1), (2, 1)] };
        let end = body_pose(&walk, walk.duration()).unwrap().1.translation;
        assert!(end.distance(Vec3::new(2.0, lift, 1.0)) < 1e-4, "{end}");
        let lunge = Beat::Lunge { unit: u, from: (3, 3), to: (4, 3) };
        let peak = body_pose(&lunge, LUNGE_SECS / 2.0).unwrap().1.translation;
        assert!(peak.distance(Vec3::new(3.0 + LUNGE_REACH, lift, 3.0)) < 1e-4);
        let back = body_pose(&lunge, LUNGE_SECS).unwrap().1.translation;
        assert!(back.distance(Vec3::new(3.0, lift, 3.0)) < 1e-4);
    }

    #[test]
    fn a_body_faces_its_path_its_target_and_its_attacker() {
        let u = units(1)[0];
        let yaw = |beat: &Beat, t: f32| heading(beat, t).map(|(_, yaw)| yaw);
        let close = |a: Option<f32>, b: f32| a.is_some_and(|a| (a - b).abs() < 1e-3);
        // East is +x, a quarter turn; north (grid y down the screen, so -1) is a half turn.
        let east = Beat::Move { unit: u, cells: vec![(0, 0), (1, 0), (2, 0)] };
        assert!(close(yaw(&east, 0.0), PI / 2.0), "{:?}", yaw(&east, 0.0));
        assert!(close(yaw(&east, east.duration()), PI / 2.0));
        let north = Beat::Move { unit: u, cells: vec![(0, 2), (0, 1)] };
        assert!(close(yaw(&north, north.duration() / 2.0), PI));
        // Round a corner: east at the start, south (no turn) at the end.
        let corner = Beat::Move { unit: u, cells: vec![(0, 0), (1, 0), (1, 1), (1, 2)] };
        assert!(close(yaw(&corner, 0.0), PI / 2.0));
        assert!(close(yaw(&corner, corner.duration()), 0.0));
        // A lunge west faces west; a flinch faces back at the striker to its south-east.
        assert!(close(yaw(&Beat::Lunge { unit: u, from: (3, 3), to: (2, 3) }, 0.1), -PI / 2.0));
        assert!(close(yaw(&Beat::Flinch { unit: u, at: (3, 3), from: (4, 4) }, 0.1), PI / 4.0));
        // Standing still or a beat without a body: no heading.
        assert_eq!(yaw(&Beat::Move { unit: u, cells: vec![(5, 5)] }, 0.0), None);
        assert_eq!(heading(&Beat::Wait { secs: 1.0 }, 0.0), None);
        assert_eq!(yaw_towards(Vec2::ZERO), None);
    }
}
