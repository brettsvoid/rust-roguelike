use bevy::prelude::*;

use crate::entity_render::{entity_transform, EntityShape, Renderable};
use crate::map::Position;

#[derive(Component)]
pub struct ParticleLifetime {
    pub lifetime_ms: f32,
}

#[derive(Clone)]
pub struct ParticleRequest {
    pub x: i32,
    pub y: i32,
    pub glyph: String,
    pub color: Color,
    pub lifetime_ms: f32,
}

#[derive(Resource, Default)]
pub struct ParticleBuilder {
    requests: Vec<ParticleRequest>,
}

impl ParticleBuilder {
    pub fn request(&mut self, x: i32, y: i32, glyph: &str, color: Color, lifetime_ms: f32) {
        self.requests.push(ParticleRequest {
            x,
            y,
            glyph: glyph.to_string(),
            color,
            lifetime_ms,
        });
    }
}

/// A particle is a small glowing cube over its tile. Proper effects are A07.
pub fn particle_spawn_system(mut commands: Commands, mut builder: ResMut<ParticleBuilder>) {
    for request in builder.requests.drain(..) {
        // No `Position`: particles are not on the tile for gameplay.
        let pos = Position {
            x: request.x,
            y: request.y,
        };
        commands.spawn((
            ParticleLifetime {
                lifetime_ms: request.lifetime_ms,
            },
            entity_transform(EntityShape::Particle, pos),
            EntityShape::Particle,
            Renderable {
                glyph: request.glyph,
                fg: request.color,
            },
        ));
    }
}

pub fn particle_cull_system(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut ParticleLifetime)>,
) {
    let delta_ms = time.delta_secs() * 1000.0;
    for (entity, mut lifetime) in &mut query {
        lifetime.lifetime_ms -= delta_ms;
        if lifetime.lifetime_ms <= 0.0 {
            commands.entity(entity).despawn();
        }
    }
}
