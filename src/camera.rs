use bevy::camera::{Camera as RenderCamera, ScalingMode};
use bevy::prelude::*;
use bevy::render::view::NoIndirectDrawing;

use crate::map::{grid_to_world, Position};
use crate::player::Player;

// Screen dimensions in tiles (viewport)
pub const SCREEN_WIDTH: i32 = 80;
pub const SCREEN_HEIGHT: i32 = 43;

#[derive(Resource, Default)]
pub struct Camera {
    pub x: i32,
    pub y: i32,
}

impl Camera {
    /// Get visible screen bounds based on camera position
    /// Returns (min_x, max_x, min_y, max_y)
    pub fn get_screen_bounds(&self) -> (i32, i32, i32, i32) {
        let min_x = self.x - (SCREEN_WIDTH / 2);
        let max_x = min_x + SCREEN_WIDTH;
        let min_y = self.y - (SCREEN_HEIGHT / 2);
        let max_y = min_y + SCREEN_HEIGHT;
        (min_x, max_x, min_y, max_y)
    }

    /// Convert world coordinates to screen coordinates
    pub fn world_to_screen(&self, x: i32, y: i32) -> (i32, i32) {
        let (min_x, _, min_y, _) = self.get_screen_bounds();
        (x - min_x, y - min_y)
    }

    /// Convert screen coordinates to world coordinates
    pub fn screen_to_world(&self, screen_x: i32, screen_y: i32) -> (i32, i32) {
        let (min_x, _, min_y, _) = self.get_screen_bounds();
        (screen_x + min_x, screen_y + min_y)
    }

    /// Check if world coordinates are within camera bounds
    #[allow(dead_code)]
    pub fn in_bounds(&self, x: i32, y: i32) -> bool {
        let (min_x, max_x, min_y, max_y) = self.get_screen_bounds();
        x >= min_x && x < max_x && y >= min_y && y < max_y
    }
}

/// Isometric rig: the camera looks down at the follow target from 45 degrees
/// round (from the +x, +z side) and 35 degrees above the floor.
const ISO_YAW: f32 = 45.0_f32.to_radians();
const ISO_PITCH: f32 = 35.0_f32.to_radians();
/// Far enough back that the tallest geometry stays in front of the near plane.
const ISO_DISTANCE: f32 = 50.0;
/// How many world units (tiles) fit top to bottom on screen.
const ISO_VIEW_HEIGHT: f32 = 24.0;

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Camera>()
            .add_systems(Startup, spawn_cameras)
            .add_systems(Update, (update_camera, follow_iso_camera).chain());
    }
}

/// The 3D camera draws the world first and clears the screen. The 2D camera
/// draws on top without clearing: it owns the UI and the remaining `Text2d`
/// glyphs (entities until F03, the map visualiser).
fn spawn_cameras(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        RenderCamera {
            order: 0,
            ..default()
        },
        Projection::from(OrthographicProjection {
            scaling_mode: ScalingMode::FixedVertical {
                viewport_height: ISO_VIEW_HEIGHT,
            },
            ..OrthographicProjection::default_3d()
        }),
        AmbientLight {
            brightness: 300.0,
            ..default()
        },
        iso_camera_transform(Vec3::ZERO),
        // GPU-driven drawing orders instances differently each run, so pixels
        // where neighbouring tiles tie on depth flicker between screenshots.
        // CPU-built draws keep screenshots byte-identical. Revisit in F10.
        NoIndirectDrawing,
    ));
    commands.spawn((
        Camera2d,
        RenderCamera {
            order: 1,
            clear_color: ClearColorConfig::None,
            ..default()
        },
        IsDefaultUiCamera,
    ));
}

fn iso_camera_transform(target: Vec3) -> Transform {
    let offset = Vec3::new(
        ISO_YAW.sin() * ISO_PITCH.cos(),
        ISO_PITCH.sin(),
        ISO_YAW.cos() * ISO_PITCH.cos(),
    ) * ISO_DISTANCE;
    Transform::from_translation(target + offset).looking_at(target, Vec3::Y)
}

fn update_camera(player_query: Query<&Position, With<Player>>, mut camera: ResMut<Camera>) {
    if let Ok(player_pos) = player_query.single() {
        camera.x = player_pos.x;
        camera.y = player_pos.y;
    }
}

/// The 3D camera follows the logical `Camera` target, so panning (F05) only
/// has to move that.
fn follow_iso_camera(camera: Res<Camera>, mut rig: Single<&mut Transform, With<Camera3d>>) {
    **rig = iso_camera_transform(grid_to_world(camera.x, camera.y));
}
