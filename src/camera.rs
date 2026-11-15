use bevy::camera::{Camera as RenderCamera, ScalingMode};
use bevy::input::keyboard::KeyboardInput;
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::input::ButtonState;
use bevy::prelude::*;
use bevy::render::view::NoIndirectDrawing;

use crate::debug::DebugMode;
use crate::map::{grid_to_world, Map, Position};
use crate::party::{ActiveUnit, PartyMember};
use crate::RunState;

// Screen dimensions in tiles (viewport)
pub const SCREEN_WIDTH: i32 = 80;
pub const SCREEN_HEIGHT: i32 = 43;

/// The grid cell the view centres on. It follows the active unit until panned,
/// and goes back to following when that unit moves, another is selected or
/// the view is recentred.
#[derive(Resource, Default)]
pub struct Camera {
    pub x: i32,
    pub y: i32,
    pub panned: bool,
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
/// Zoom limits, in tiles top to bottom.
const MIN_VIEW_HEIGHT: f32 = 8.0;
const MAX_VIEW_HEIGHT: f32 = 48.0;

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Camera>()
            .add_systems(Startup, spawn_cameras)
            .add_systems(
                Update,
                (
                    (pan_camera, zoom_camera)
                        .run_if(in_state(RunState::AwaitingInput).or_else(in_state(RunState::ShowTargeting))),
                    update_camera,
                    follow_iso_camera,
                )
                    .chain(),
            );
    }
}

/// The 3D camera draws the world first and clears the screen. The 2D camera
/// draws on top: it owns the UI and the remaining `Text2d`
/// glyphs (the map visualiser).
fn spawn_cameras(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        RenderCamera {
            order: 0,
            ..default()
        },
        Projection::from(iso_projection()),
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
            // The 3D camera is HDR, so this camera has its own texture, which
            // Bevy alpha-blends over the world. Clearing it to transparent
            // stops old frames (the main menu) showing through.
            clear_color: ClearColorConfig::Custom(Color::NONE),
            ..default()
        },
        IsDefaultUiCamera,
    ));
}

pub(crate) fn iso_projection() -> OrthographicProjection {
    OrthographicProjection {
        scaling_mode: ScalingMode::FixedVertical {
            viewport_height: ISO_VIEW_HEIGHT,
        },
        ..OrthographicProjection::default_3d()
    }
}

pub(crate) fn iso_camera_transform(target: Vec3) -> Transform {
    let offset = Vec3::new(
        ISO_YAW.sin() * ISO_PITCH.cos(),
        ISO_PITCH.sin(),
        ISO_YAW.cos() * ISO_PITCH.cos(),
    ) * ISO_DISTANCE;
    Transform::from_translation(target + offset).looking_at(target, Vec3::Y)
}

fn update_camera(active: Res<ActiveUnit>, party: Query<Ref<Position>, With<PartyMember>>, mut camera: ResMut<Camera>) {
    if let Some(unit_pos) = active.0.and_then(|unit| party.get(unit).ok()) {
        if unit_pos.is_changed() || active.is_changed() {
            camera.panned = false;
        }
        if !camera.panned {
            camera.x = unit_pos.x;
            camera.y = unit_pos.y;
        }
    }
}

/// The grid step that moves the view one tile along the screen for a WASD
/// key. The rig looks towards -x and -z, so screen up is grid (-1, -1).
fn pan_step(key: KeyCode) -> Option<IVec2> {
    match key {
        KeyCode::KeyW => Some(IVec2::new(-1, -1)),
        KeyCode::KeyS => Some(IVec2::new(1, 1)),
        KeyCode::KeyA => Some(IVec2::new(-1, 1)),
        KeyCode::KeyD => Some(IVec2::new(1, -1)),
        _ => None,
    }
}

/// WASD pans one step per press and per key repeat; C recentres on the active unit.
fn pan_camera(
    mut evr_kbd: MessageReader<KeyboardInput>,
    debug_mode: Res<DebugMode>,
    map: Res<Map>,
    mut camera: ResMut<Camera>,
) {
    for ev in evr_kbd.read() {
        if ev.state == ButtonState::Released || debug_mode.show_console {
            continue;
        }
        if ev.key_code == KeyCode::KeyC {
            camera.panned = false;
        } else if let Some(step) = pan_step(ev.key_code) {
            camera.x = (camera.x + step.x).clamp(0, map.width - 1);
            camera.y = (camera.y + step.y).clamp(0, map.height - 1);
            camera.panned = true;
        }
    }
}

/// The view height after scrolling `lines` notches: each notch up zooms in
/// by a tenth, within the zoom limits.
fn zoomed_view_height(height: f32, lines: f32) -> f32 {
    (height * 0.9_f32.powf(lines)).clamp(MIN_VIEW_HEIGHT, MAX_VIEW_HEIGHT)
}

fn zoom_camera(mut evr_wheel: MessageReader<MouseWheel>, mut projection: Single<&mut Projection, With<Camera3d>>) {
    // shortcut: 40 pixels count as one notch for trackpads, tune when someone tries one.
    let lines: f32 = evr_wheel
        .read()
        .map(|ev| match ev.unit {
            MouseScrollUnit::Line => ev.y,
            MouseScrollUnit::Pixel => ev.y / 40.0,
        })
        .sum();
    if lines == 0.0 {
        return;
    }
    if let Projection::Orthographic(ortho) = &mut **projection {
        if let ScalingMode::FixedVertical { viewport_height } = &mut ortho.scaling_mode {
            *viewport_height = zoomed_view_height(*viewport_height, lines);
        }
    }
}

/// The 3D camera follows the logical `Camera` target, so panning (F05) only
/// has to move that.
pub(crate) fn follow_iso_camera(camera: Res<Camera>, mut rig: Single<&mut Transform, With<Camera3d>>) {
    **rig = iso_camera_transform(grid_to_world(camera.x, camera.y));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wasd_pans_along_the_screen_axes() {
        let rig = iso_camera_transform(Vec3::ZERO);
        let on_floor = |v: Vec3| Vec2::new(v.x, v.z).normalize();
        let grid = |key| pan_step(key).unwrap().as_vec2().normalize();
        let close = |a: Vec2, b: Vec2| (a - b).length() < 1e-5;
        assert!(close(grid(KeyCode::KeyW), on_floor(*rig.up())));
        assert!(close(grid(KeyCode::KeyS), -on_floor(*rig.up())));
        assert!(close(grid(KeyCode::KeyD), on_floor(*rig.right())));
        assert!(close(grid(KeyCode::KeyA), -on_floor(*rig.right())));
        assert_eq!(pan_step(KeyCode::KeyQ), None);
    }

    #[test]
    fn scrolling_zooms_by_a_tenth_a_notch_within_limits() {
        assert!((zoomed_view_height(24.0, 1.0) - 21.6).abs() < 1e-4);
        assert!((zoomed_view_height(24.0, -1.0) - 24.0 / 0.9).abs() < 1e-4);
        assert_eq!(zoomed_view_height(24.0, 0.0), 24.0);
        assert_eq!(zoomed_view_height(9.0, 5.0), MIN_VIEW_HEIGHT);
        assert_eq!(zoomed_view_height(45.0, -5.0), MAX_VIEW_HEIGHT);
    }
}
