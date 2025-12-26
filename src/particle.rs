use bevy::prelude::*;

use crate::map::{Position, FONT_SIZE, GRID_PX};
use crate::resources::UiFont;

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

pub fn particle_spawn_system(
    mut commands: Commands,
    mut builder: ResMut<ParticleBuilder>,
    font: Res<UiFont>,
    window: Query<&Window>,
) {
    let Ok(window) = window.get_single() else {
        return;
    };

    let half_width = window.width() / 2.0;
    let half_height = window.height() / 2.0;

    for request in builder.requests.drain(..) {
        // Calculate screen position directly to avoid frame delay
        let screen_x = (request.x as f32) * GRID_PX.x + (GRID_PX.x / 2.0) - half_width;
        let screen_y = (request.y as f32) * -GRID_PX.y - (GRID_PX.y / 2.0) + half_height;

        commands.spawn((
            ParticleLifetime {
                lifetime_ms: request.lifetime_ms,
            },
            Position {
                x: request.x,
                y: request.y,
            },
            Text2d::new(request.glyph),
            TextFont {
                font: font.0.clone(),
                font_size: FONT_SIZE,
                ..default()
            },
            TextColor(request.color),
            Transform::from_xyz(screen_x, screen_y, 10.0), // Above other entities
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
