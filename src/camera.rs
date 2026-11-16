use bevy::camera::{Camera as RenderCamera, ScalingMode};
use bevy::input::gestures::PinchGesture;
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use bevy::render::view::NoIndirectDrawing;

use crate::debug::DebugMode;
use crate::map::{Map, Position};
use crate::party::{ActiveUnit, PartyMember};
use crate::RunState;

// Screen dimensions in tiles (viewport)
pub const SCREEN_WIDTH: i32 = 80;
pub const SCREEN_HEIGHT: i32 = 43;

/// Pan speed in tiles per second at the default zoom. It scales with the view
/// height, so a pan crosses the same share of the screen at any zoom.
pub const PAN_SPEED: f32 = 14.0;
/// Time constant (seconds) of the pan's ease in and out.
pub const PAN_EASE_TIME: f32 = 0.08;
/// Time constant (seconds) of the glide to the active unit (follow, select, C).
pub const FOLLOW_TIME: f32 = 0.15;
/// Time constant (seconds) of the zoom's ease to its target height.
pub const ZOOM_EASE_TIME: f32 = 0.1;
/// Each zoom step shrinks the view height to this share (zooms in by a tenth).
pub const ZOOM_PER_STEP: f32 = 0.9;
/// Touchpad scroll pixels per zoom step.
pub const TOUCHPAD_PIXELS_PER_STEP: f32 = 100.0;
/// Touchpad scroll direction. With macOS natural scrolling, two fingers moving
/// up report negative pixels, so -1 makes that zoom in. Set to 1 to invert.
pub const TOUCHPAD_ZOOM_SIGN: f32 = -1.0;
/// Mouse wheel direction: 1 makes wheel up (positive lines) zoom in.
pub const WHEEL_ZOOM_SIGN: f32 = 1.0;
/// Zoom steps per unit of macOS pinch (a pinch out is positive and zooms in).
pub const PINCH_STEPS_PER_UNIT: f32 = 6.0;

/// The logical camera: the floor point the view centres on and the zoom. It
/// follows the active unit until panned, and glides back to following when
/// that unit moves, another is selected or the view is recentred (C).
/// `follow_iso_camera` turns it into the 3D rig.
#[derive(Resource)]
pub struct Camera {
    /// Grid coordinates of the view centre, fractional: world `(x, 0, y)`.
    focus: Vec2,
    /// Pan velocity in tiles per second.
    velocity: Vec2,
    /// The view height (tiles top to bottom) zoom is easing towards.
    zoom: f32,
    /// The view height on screen now.
    view_height: f32,
    panned: bool,
    /// Jump straight to the active unit on the next update (new game, new level).
    snap: bool,
    /// A grid point to glide to instead of the active unit (`look_at`).
    look_at: Option<Vec2>,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            focus: Vec2::ZERO,
            velocity: Vec2::ZERO,
            zoom: ISO_VIEW_HEIGHT,
            view_height: ISO_VIEW_HEIGHT,
            panned: false,
            snap: true,
            look_at: None,
        }
    }
}

impl Camera {
    /// Screen cell of grid cell `(x, y)` for the map visualiser's 80 by 43 glyph grid,
    /// centred on the focus cell.
    pub fn world_to_screen(&self, x: i32, y: i32) -> (i32, i32) {
        let centre = self.focus.round().as_ivec2();
        (x - centre.x + SCREEN_WIDTH / 2, y - centre.y + SCREEN_HEIGHT / 2)
    }

    /// Glide to grid point `at` (action off-screen) instead of the active
    /// unit, overriding a pan; `None` glides back to following the unit.
    pub fn look_at(&mut self, at: Option<Vec2>) {
        self.look_at = at;
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
            .add_systems(OnExit(RunState::PreRun), |mut camera: ResMut<Camera>| camera.snap = true)
            .add_systems(Update, (update_camera, follow_iso_camera).chain());
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

/// Reads WASD, C, the wheel and pinches (only while the game waits for input),
/// then moves the focus and eases the zoom by the frame time.
fn update_camera(
    time: Res<Time>,
    state: Res<State<RunState>>,
    debug_mode: Res<DebugMode>,
    keys: Res<ButtonInput<KeyCode>>,
    mut wheel: MessageReader<MouseWheel>,
    mut pinch: MessageReader<PinchGesture>,
    map: Res<Map>,
    active: Res<ActiveUnit>,
    party: Query<Ref<Position>, With<PartyMember>>,
    mut camera: ResMut<Camera>,
) {
    let dt = time.delta_secs();
    let steering =
        matches!(state.get(), RunState::AwaitingInput | RunState::ShowTargeting) && !debug_mode.show_console;
    let steps: f32 = wheel
        .read()
        .map(|ev| scroll_zoom_steps(ev.unit, ev.y))
        .chain(pinch.read().map(|ev| ev.0 * PINCH_STEPS_PER_UNIT))
        .sum();
    let camera = &mut *camera;
    let held = keys.get_pressed().copied().filter(|_| steering);
    let target_velocity = pan_velocity(held, camera.view_height);
    if steering {
        camera.zoom = zoomed_view_height(camera.zoom, steps);
        if keys.just_pressed(KeyCode::KeyC) {
            camera.panned = false;
        }
        if target_velocity != Vec2::ZERO {
            camera.panned = true;
        }
    }
    camera.view_height = glide(camera.view_height, camera.zoom, dt, ZOOM_EASE_TIME);
    camera.velocity = glide_vec(camera.velocity, target_velocity, dt, PAN_EASE_TIME);

    if let Some(unit_pos) = active.0.and_then(|unit| party.get(unit).ok()) {
        let target = Vec2::new(unit_pos.x as f32, unit_pos.y as f32);
        if camera.snap {
            camera.snap = false;
            camera.panned = false;
            camera.focus = target;
        }
        if unit_pos.is_changed() || active.is_changed() || camera.look_at.is_some() {
            camera.panned = false;
        }
        if !camera.panned {
            camera.velocity = Vec2::ZERO;
            camera.focus = glide_vec(camera.focus, camera.look_at.unwrap_or(target), dt, FOLLOW_TIME);
            return;
        }
    }
    camera.focus = clamp_focus(camera.focus + camera.velocity * dt, map.width, map.height);
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

/// The pan velocity (grid tiles per second) for the held keys: `PAN_SPEED`
/// along the screen direction of the WASD keys, the same speed on diagonals,
/// scaled by how far the view is zoomed out.
fn pan_velocity(held: impl IntoIterator<Item = KeyCode>, view_height: f32) -> Vec2 {
    let direction: IVec2 = held.into_iter().filter_map(pan_step).sum();
    direction.as_vec2().normalize_or_zero() * PAN_SPEED * view_height / ISO_VIEW_HEIGHT
}

/// One frame of exponential smoothing from `current` towards `target` with
/// time constant `time_constant`: never overshoots, and the same over a
/// second whatever the frame rate. Lands exactly once within a thousandth.
fn glide(current: f32, target: f32, dt: f32, time_constant: f32) -> f32 {
    let next = current + (target - current) * (1.0 - (-dt / time_constant).exp());
    if (target - next).abs() < 1e-3 { target } else { next }
}

fn glide_vec(current: Vec2, target: Vec2, dt: f32, time_constant: f32) -> Vec2 {
    Vec2::new(glide(current.x, target.x, dt, time_constant), glide(current.y, target.y, dt, time_constant))
}

/// Keeps the view centre on the map.
fn clamp_focus(focus: Vec2, width: i32, height: i32) -> Vec2 {
    focus.clamp(Vec2::ZERO, Vec2::new((width - 1) as f32, (height - 1) as f32))
}

/// Zoom steps (positive zooms in) for one scroll event: a wheel line is a step
/// in `WHEEL_ZOOM_SIGN`'s direction, touchpad pixels count
/// `TOUCHPAD_PIXELS_PER_STEP` to a step in `TOUCHPAD_ZOOM_SIGN`'s direction.
fn scroll_zoom_steps(unit: MouseScrollUnit, y: f32) -> f32 {
    match unit {
        MouseScrollUnit::Line => y * WHEEL_ZOOM_SIGN,
        MouseScrollUnit::Pixel => y * TOUCHPAD_ZOOM_SIGN / TOUCHPAD_PIXELS_PER_STEP,
    }
}

/// The view height after `steps` zoom steps, within the zoom limits.
fn zoomed_view_height(height: f32, steps: f32) -> f32 {
    (height * ZOOM_PER_STEP.powf(steps)).clamp(MIN_VIEW_HEIGHT, MAX_VIEW_HEIGHT)
}

/// Places the 3D rig and its zoom from the logical `Camera`.
pub(crate) fn follow_iso_camera(camera: Res<Camera>, rig: Single<(&mut Transform, &mut Projection), With<Camera3d>>) {
    let (mut transform, mut projection) = rig.into_inner();
    *transform = iso_camera_transform(Vec3::new(camera.focus.x, 0.0, camera.focus.y));
    if let Projection::Orthographic(ortho) = &mut *projection {
        // Only on a change, so a still camera doesn't recompute its projection each frame.
        if !matches!(ortho.scaling_mode, ScalingMode::FixedVertical { viewport_height } if viewport_height == camera.view_height) {
            ortho.scaling_mode = ScalingMode::FixedVertical { viewport_height: camera.view_height };
        }
    }
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
    fn holding_keys_pans_at_a_steady_speed_scaled_by_zoom() {
        let close = |a: Vec2, b: Vec2| (a - b).length() < 1e-4;
        let w = PAN_SPEED / 2.0_f32.sqrt();
        assert!(close(pan_velocity([KeyCode::KeyW], 24.0), Vec2::new(-w, -w)));
        // Two keys go diagonally at the same speed, opposite keys cancel.
        assert!(close(pan_velocity([KeyCode::KeyW, KeyCode::KeyD], 24.0), Vec2::new(0.0, -PAN_SPEED)));
        assert_eq!(pan_velocity([KeyCode::KeyW, KeyCode::KeyS], 24.0), Vec2::ZERO);
        assert_eq!(pan_velocity([KeyCode::KeyQ], 24.0), Vec2::ZERO);
        // Zoomed out to twice the view height, the pan is twice as fast.
        assert!(close(pan_velocity([KeyCode::KeyS], 48.0), Vec2::new(2.0 * w, 2.0 * w)));
    }

    #[test]
    fn glide_converges_without_overshooting_at_any_frame_rate() {
        let run = |fps: u32| {
            let mut x = 0.0;
            let mut samples = vec![];
            for frame in 1..=fps {
                x = glide(x, 10.0, 1.0 / fps as f32, 0.15);
                assert!(x <= 10.0, "overshot at frame {frame}: {x}");
                if frame * 10 % fps == 0 {
                    samples.push(x);
                }
            }
            samples
        };
        let (at30, at60, at120) = (run(30), run(60), run(120));
        // After 0.1 s, one minus e^(-0.1 / 0.15) of the way: 4.866.
        assert!((at60[0] - 4.866).abs() < 1e-3, "{}", at60[0]);
        for i in 0..10 {
            assert!((at30[i] - at60[i]).abs() < 1e-3 && (at120[i] - at60[i]).abs() < 1e-3, "{i}");
        }
        // Ten tiles settle exactly within one and a half seconds.
        assert!(at60[9] < 10.0);
        assert_eq!((0..90).fold(0.0, |x, _| glide(x, 10.0, 1.0 / 60.0, 0.15)), 10.0);
        assert_eq!(glide(3.0, 3.0, 1.0 / 60.0, 0.15), 3.0);
    }

    #[test]
    fn focus_stays_on_the_map() {
        assert_eq!(clamp_focus(Vec2::new(-0.4, 12.5), 80, 50), Vec2::new(0.0, 12.5));
        assert_eq!(clamp_focus(Vec2::new(85.0, 49.5), 80, 50), Vec2::new(79.0, 49.0));
    }

    #[test]
    fn wheel_up_and_touchpad_fingers_up_zoom_in() {
        // Wheel up is +1 line; macOS natural scrolling reports fingers up as negative pixels.
        assert_eq!(scroll_zoom_steps(MouseScrollUnit::Line, 1.0), 1.0);
        assert_eq!(scroll_zoom_steps(MouseScrollUnit::Line, -2.0), -2.0);
        assert_eq!(scroll_zoom_steps(MouseScrollUnit::Pixel, -100.0), 1.0);
        assert_eq!(scroll_zoom_steps(MouseScrollUnit::Pixel, 250.0), -2.5);
        assert!(zoomed_view_height(24.0, scroll_zoom_steps(MouseScrollUnit::Pixel, -100.0)) < 24.0);
    }

    #[test]
    fn the_visualiser_grid_centres_on_the_rounded_focus() {
        let camera = Camera { focus: Vec2::new(10.4, 20.6), ..default() };
        assert_eq!(camera.world_to_screen(10, 21), (40, 21));
        assert_eq!(camera.world_to_screen(0, 0), (30, 0));
    }

    #[test]
    fn each_zoom_step_is_a_tenth_within_limits() {
        assert!((zoomed_view_height(24.0, 1.0) - 21.6).abs() < 1e-4);
        assert!((zoomed_view_height(24.0, -1.0) - 24.0 / 0.9).abs() < 1e-4);
        assert_eq!(zoomed_view_height(24.0, 0.0), 24.0);
        assert_eq!(zoomed_view_height(9.0, 5.0), MIN_VIEW_HEIGHT);
        assert_eq!(zoomed_view_height(45.0, -5.0), MAX_VIEW_HEIGHT);
    }
}
