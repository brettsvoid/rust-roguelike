//! Combat and spell effects: short-lived glowing meshes that fly, swell and
//! fade on a code tween, then despawn.
//!
//! Gameplay asks for one with `Effects::play(kind, x, y)`. `EffectKind::style`
//! is the one table of how each kind looks (mesh, colour, lifetime, piece
//! count, motion, light) and `piece_transform` places every piece as a pure
//! function of the effect's progress. Effects age on `Time`, which the
//! screenshot harness steps by a fixed amount per frame, so captures stay
//! byte-identical.

use std::collections::HashMap;
use std::f32::consts::{PI, TAU};

use bevy::light::NotShadowCaster;
use bevy::prelude::*;

use crate::map::grid_to_world;
use crate::RunState;

pub struct EffectsPlugin;

impl Plugin for EffectsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Effects>().add_systems(
            Update,
            (spawn_effects, animate_effects)
                .chain()
                .run_if(not(in_state(RunState::MapGeneration))),
        );
    }
}

/// What to play. The tile passed to `Effects::play` is the target.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EffectKind {
    /// Sparks off a melee or trap hit.
    HitSpark,
    /// A fire burst filling a blast `radius` in tiles.
    Fireball { radius: f32 },
    /// A bolt flying from the caster's tile to the target.
    MagicMissile { from: (i32, i32) },
    /// Green motes spiralling up the body.
    Healing,
    /// Orbs circling above the head.
    Confusion,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum EffectMesh {
    Shard,
    Ball,
    Mote,
}

impl EffectMesh {
    fn mesh(self) -> Mesh {
        match self {
            // Long along z, so `looking_to` points it along its flight.
            EffectMesh::Shard => Cuboid::new(0.07, 0.07, 0.5).into(),
            // Unit radius, scaled per piece.
            EffectMesh::Ball => Sphere::new(1.0).mesh().ico(3).unwrap(),
            EffectMesh::Mote => Sphere::new(0.13).mesh().ico(1).unwrap(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Motion {
    /// Pieces fly out from chest height in a fan, shortening as they go.
    Burst { reach: f32 },
    /// Nested balls swell from the floor to `size` tiles across. Additive
    /// blending stacks them into a white-hot core with a darker rim.
    Swell { size: f32 },
    /// Pieces fly from the caster to the target in an arc, each trailing the
    /// one ahead and smaller.
    Bolt,
    /// Pieces spiral up around the body.
    Rise { height: f32 },
    /// Pieces circle above the head.
    Orbit { radius: f32 },
}

struct EffectStyle {
    mesh: EffectMesh,
    /// Linear HDR colour; channels far above 1 bloom.
    colour: LinearRgba,
    /// Seconds.
    lifetime: f32,
    pieces: u32,
    motion: Motion,
    /// Peak lumens of a point light riding the first piece; 0 for none.
    light: f32,
}

impl EffectKind {
    fn style(self) -> EffectStyle {
        match self {
            EffectKind::HitSpark => EffectStyle {
                mesh: EffectMesh::Shard,
                colour: LinearRgba::rgb(6.0, 2.2, 0.4),
                lifetime: 0.3,
                pieces: 9,
                motion: Motion::Burst { reach: 1.0 },
                light: 0.0,
            },
            EffectKind::Fireball { radius } => EffectStyle {
                mesh: EffectMesh::Ball,
                colour: LinearRgba::rgb(1.6, 0.12, 0.0),
                lifetime: 0.7,
                pieces: 3,
                motion: Motion::Swell { size: radius + 0.5 },
                light: 300_000.0,
            },
            EffectKind::MagicMissile { .. } => EffectStyle {
                mesh: EffectMesh::Ball,
                colour: LinearRgba::rgb(1.5, 4.0, 12.0),
                lifetime: 0.45,
                pieces: 5,
                motion: Motion::Bolt,
                light: 60_000.0,
            },
            EffectKind::Healing => EffectStyle {
                mesh: EffectMesh::Mote,
                colour: LinearRgba::rgb(1.5, 10.0, 2.5),
                lifetime: 0.9,
                pieces: 10,
                motion: Motion::Rise { height: 1.8 },
                light: 0.0,
            },
            EffectKind::Confusion => EffectStyle {
                mesh: EffectMesh::Mote,
                colour: LinearRgba::rgb(9.0, 1.5, 9.0),
                lifetime: 1.0,
                pieces: 3,
                motion: Motion::Orbit { radius: 0.45 },
                light: 0.0,
            },
        }
    }
}

/// Brightness over an effect's life `t` (0 to 1): a quick flash in, then an
/// ease out to nothing.
fn envelope(t: f32) -> f32 {
    ((t / 0.08).min(1.0) * (1.0 - t * t)).clamp(0.0, 1.0)
}

fn ease_out(t: f32) -> f32 {
    1.0 - (1.0 - t) * (1.0 - t)
}

/// Where piece `i` of `n` sits at progress `t` (0 to 1). `from` and `to` are
/// floor points: the caster and the target (the same tile for most kinds).
fn piece_transform(motion: Motion, i: u32, n: u32, t: f32, from: Vec3, to: Vec3) -> Transform {
    let fi = i as f32;
    let fraction = fi / n as f32;
    match motion {
        Motion::Burst { reach } => {
            // The golden angle spreads any count evenly round the circle.
            let angle = fi * 2.399_963;
            let rise = 0.15 + 0.7 * (fi * 0.618_034).fract();
            let dir = Vec3::new(angle.cos(), rise, angle.sin()).normalize();
            Transform::from_translation(to + Vec3::Y * 1.0 + dir * reach * ease_out(t))
                .looking_to(dir, Vec3::Y)
                .with_scale(Vec3::splat(1.0 - t))
        }
        Motion::Swell { size } => {
            let s = (size * ease_out(t) * (1.0 - fraction)).max(0.15);
            Transform::from_translation(to + Vec3::Y * 0.3).with_scale(Vec3::new(s, s * 0.6, s))
        }
        Motion::Bolt => {
            let u = (t * 1.3 - fi * 0.06).clamp(0.0, 1.0);
            let arc = (PI * u).sin() * 0.5;
            let dir = (to - from).normalize_or(Vec3::X);
            let s = 0.22 * (1.0 - 0.7 * fraction);
            Transform::from_translation(from.lerp(to, u) + Vec3::Y * (1.0 + arc))
                .looking_to(dir, Vec3::Y)
                .with_scale(Vec3::new(s, s, s * 2.5))
        }
        Motion::Rise { height } => {
            let angle = fraction * TAU + t * TAU * 0.75;
            let y = 0.2 + height * (0.8 * t + 0.35 * fraction);
            Transform::from_translation(to + Vec3::new(angle.cos() * 0.5, y, angle.sin() * 0.5))
                .with_scale(Vec3::splat(1.0 - 0.5 * t))
        }
        Motion::Orbit { radius } => {
            let angle = fraction * TAU + t * TAU * 1.5;
            let bob = (angle * 2.0).sin() * 0.06;
            Transform::from_translation(
                to + Vec3::new(angle.cos() * radius, 1.85 + bob, angle.sin() * radius),
            )
        }
    }
}

/// Effects asked for this frame; `spawn_effects` plays them.
#[derive(Resource, Default)]
pub struct Effects {
    requests: Vec<(EffectKind, i32, i32)>,
}

impl Effects {
    pub fn play(&mut self, kind: EffectKind, x: i32, y: i32) {
        self.requests.push((kind, x, y));
    }
}

#[derive(Component)]
struct Effect {
    kind: EffectKind,
    from: Vec3,
    to: Vec3,
    age: f32,
    /// One additive material per effect, shared by its pieces, so it fades as one.
    material: Handle<StandardMaterial>,
}

#[derive(Component)]
struct Piece(u32);

#[derive(Component)]
struct EffectLight;

/// Effects are not on a tile for gameplay, so they have no `Position` and
/// show through the fog of war like the light they give off.
fn spawn_effects(
    mut commands: Commands,
    mut effects: ResMut<Effects>,
    mut mesh_cache: Local<HashMap<EffectMesh, Handle<Mesh>>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (kind, x, y) in effects.requests.drain(..) {
        let style = kind.style();
        let to = grid_to_world(x, y);
        let from = match kind {
            EffectKind::MagicMissile { from: (fx, fy) } => grid_to_world(fx, fy),
            _ => to,
        };
        let mesh = mesh_cache
            .entry(style.mesh)
            .or_insert_with(|| meshes.add(style.mesh.mesh()))
            .clone();
        let material = materials.add(StandardMaterial {
            base_color: style.colour.with_alpha(0.0).into(),
            unlit: true,
            alpha_mode: AlphaMode::Add,
            ..default()
        });
        let mut effect = commands.spawn((
            Effect {
                kind,
                from,
                to,
                age: 0.0,
                material: material.clone(),
            },
            Transform::default(),
            Visibility::default(),
        ));
        for i in 0..style.pieces {
            effect.with_child((
                Piece(i),
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material.clone()),
                piece_transform(style.motion, i, style.pieces, 0.0, from, to),
                NotShadowCaster,
            ));
        }
        if style.light > 0.0 {
            let c = style.colour;
            let peak = c.red.max(c.green).max(c.blue);
            effect.with_child((
                EffectLight,
                PointLight {
                    color: LinearRgba::rgb(c.red / peak, c.green / peak, c.blue / peak).into(),
                    intensity: 0.0,
                    range: 8.0,
                    ..default()
                },
                piece_transform(style.motion, 0, style.pieces, 0.0, from, to),
            ));
        }
    }
}

/// Ages every effect, moves its pieces and light, fades it and despawns it
/// at the end of its life.
#[allow(clippy::type_complexity)]
fn animate_effects(
    mut commands: Commands,
    time: Res<Time>,
    mut effects: Query<(Entity, &mut Effect, &Children)>,
    mut pieces: Query<(&Piece, &mut Transform), Without<EffectLight>>,
    mut lights: Query<(&mut PointLight, &mut Transform), With<EffectLight>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (entity, mut effect, children) in &mut effects {
        let style = effect.kind.style();
        effect.age += time.delta_secs();
        let t = effect.age / style.lifetime;
        if t >= 1.0 {
            commands.entity(entity).despawn();
            continue;
        }
        let brightness = envelope(t);
        if let Some(mut material) = materials.get_mut(&effect.material) {
            material.base_color.set_alpha(brightness);
        }
        let place = |i| piece_transform(style.motion, i, style.pieces, t, effect.from, effect.to);
        for &child in children {
            if let Ok((piece, mut transform)) = pieces.get_mut(child) {
                *transform = place(piece.0);
            } else if let Ok((mut light, mut transform)) = lights.get_mut(child) {
                light.intensity = style.light * brightness;
                transform.translation = place(0).translation;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_flashes_in_and_fades_out() {
        assert_eq!(envelope(0.0), 0.0);
        assert!((envelope(0.04) - 0.4992).abs() < 1e-5);
        assert_eq!(envelope(0.5), 0.75);
        assert_eq!(envelope(1.0), 0.0);
    }

    #[test]
    fn missile_flies_from_caster_to_target() {
        let from = Vec3::new(2.0, 0.0, 3.0);
        let to = Vec3::new(6.0, 0.0, 3.0);
        let start = piece_transform(Motion::Bolt, 0, 5, 0.0, from, to);
        let end = piece_transform(Motion::Bolt, 0, 5, 0.9, from, to);
        assert_eq!(start.translation, Vec3::new(2.0, 1.0, 3.0));
        assert!(end.translation.abs_diff_eq(Vec3::new(6.0, 1.0, 3.0), 1e-5));
    }

    #[test]
    fn fireball_fills_its_blast_radius() {
        let EffectStyle { motion, .. } = EffectKind::Fireball { radius: 3.0 }.style();
        let end = piece_transform(motion, 0, 3, 1.0, Vec3::ZERO, Vec3::ZERO);
        assert_eq!(end.scale.x, 3.5);
    }
}
