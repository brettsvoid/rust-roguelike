//! The UI's shared colours, sizes and easing, in one place: the HUD, the
//! modal panels (`panel.rs`), the main menu and the tooltip all draw from it.

use bevy::prelude::*;

/// Panels over the world: the bottom bar, the shot preview, the banner.
/// Opaque, so unit health bars and the lit scene never show through.
pub const PANEL: Color = Color::srgb(0.055, 0.06, 0.075);
/// The hairline on a panel's edge.
pub const PANEL_EDGE: Color = Color::srgba(1.0, 1.0, 1.0, 0.09);
pub const CARD: Color = Color::srgba(1.0, 1.0, 1.0, 0.035);
pub const CARD_HOVER: Color = Color::srgba(1.0, 1.0, 1.0, 0.07);
pub const CARD_ACTIVE: Color = Color::srgba(0.95, 0.76, 0.32, 0.13);

/// The active hero, the banner, highlights.
pub const ACCENT: Color = Color::srgb(0.95, 0.76, 0.32);
pub const TEXT: Color = Color::srgb(0.91, 0.9, 0.86);
pub const TEXT_DIM: Color = Color::srgb(0.6, 0.61, 0.64);
pub const TEXT_FAINT: Color = Color::srgb(0.36, 0.37, 0.4);

pub const HEALTH: Color = Color::srgb(0.38, 0.8, 0.42);
pub const HEALTH_LOW: Color = Color::srgb(0.92, 0.33, 0.27);
pub const HOSTILE: Color = Color::srgb(0.86, 0.27, 0.22);
/// The empty part of a bar.
pub const TRACK: Color = Color::srgba(0.0, 0.0, 0.0, 0.55);
pub const AP: Color = Color::srgb(0.42, 0.76, 1.0);
pub const AP_SPENT: Color = Color::srgba(1.0, 1.0, 1.0, 0.13);

pub const BUTTON: Color = Color::srgba(1.0, 1.0, 1.0, 0.06);
pub const BUTTON_HOVER: Color = Color::srgba(0.95, 0.76, 0.32, 0.22);
pub const BUTTON_DISABLED: Color = Color::srgba(1.0, 1.0, 1.0, 0.025);
pub const BUTTON_EDGE: Color = Color::srgba(1.0, 1.0, 1.0, 0.14);

/// Status badges and cover notes.
pub const BADGE_OVERWATCH: Color = Color::srgb(0.42, 0.76, 1.0);
pub const BADGE_HUNKERED: Color = Color::srgb(0.95, 0.68, 0.28);
pub const BADGE_POISON: Color = Color::srgb(0.55, 0.85, 0.3);
pub const BADGE_CONFUSED: Color = Color::srgb(0.78, 0.55, 0.95);
pub const BADGE_FED: Color = Color::srgb(0.6, 0.78, 0.5);
pub const BADGE_HUNGRY: Color = Color::srgb(0.95, 0.68, 0.28);

pub const FONT_SMALL: f32 = 10.0;
pub const FONT_BODY: f32 = 12.0;
pub const FONT_NAME: f32 = 13.0;
pub const FONT_BIG: f32 = 22.0;
pub const FONT_BANNER: f32 = 34.0;
/// Rows and text in the modal panels.
pub const FONT_ROW: f32 = 14.0;
/// A panel's title.
pub const FONT_HEADING: f32 = 17.0;
/// Main menu entries.
pub const FONT_MENU: f32 = 22.0;
/// The game's name on the main menu.
pub const FONT_TITLE: f32 = 60.0;

/// Behind a modal panel: the world dimmed, still readable.
pub const BACKDROP: Color = Color::srgba(0.0, 0.0, 0.0, 0.55);
/// A panel's title bar and footer, a shade off the panel.
pub const PANEL_BAND: Color = Color::srgba(1.0, 1.0, 1.0, 0.03);
/// A row under the cursor or picked with the arrow keys.
pub const ROW_SELECTED: Color = Color::srgba(0.95, 0.76, 0.32, 0.12);
/// The key cap a row or hint is chosen with.
pub const KEY_CAP: Color = Color::srgba(1.0, 1.0, 1.0, 0.08);

/// The bottom bar: four card rows plus padding.
pub const BAR_HEIGHT: f32 = 132.0;
pub const BAR_PADDING: f32 = 8.0;
pub const GAP: f32 = 10.0;
pub const RADIUS: f32 = 4.0;
pub const CARD_WIDTH: f32 = 300.0;
pub const CARD_HEIGHT: f32 = 26.0;
pub const BUTTON_WIDTH: f32 = 88.0;
pub const BUTTON_GAP: f32 = 4.0;
pub const BUTTON_HEIGHT: f32 = 50.0;
/// The log and depth column: a share of the window, within these bounds.
pub const LOG_MIN_WIDTH: f32 = 150.0;
pub const LOG_MAX_WIDTH: f32 = 520.0;
/// Modal panels.
pub const PANEL_PADDING: f32 = 16.0;
pub const ROW_HEIGHT: f32 = 26.0;
pub const KEY_CAP_WIDTH: f32 = 20.0;
/// Seconds a panel takes to ease in or out.
pub const PANEL_EASE: f32 = 0.14;
/// How small a panel starts as it eases in.
pub const PANEL_START_SCALE: f32 = 0.96;
/// Health bars over units, in logical pixels.
pub const UNIT_BAR_WIDTH: f32 = 34.0;
pub const UNIT_BAR_HEIGHT: f32 = 5.0;

/// A `TextFont` at `size` in the HUD font.
pub fn font(handle: &Handle<Font>, size: f32) -> TextFont {
    TextFont {
        font: handle.clone().into(),
        font_size: bevy::text::FontSize::Px(size),
        ..default()
    }
}

/// 0 to 1 smoothed at both ends (smoothstep), for every UI ease.
pub fn ease(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Whether the cursor (logical pixels) is over a UI node (physical pixels of
/// the render target, which is an image at scale 1 in the harness).
pub fn under_cursor(
    cursor: Vec2,
    camera: &Camera,
    node: &bevy::ui::ComputedNode,
    transform: &bevy::ui::UiGlobalTransform,
) -> bool {
    node.contains_point(*transform, cursor * camera.target_scaling_factor().unwrap_or(1.0))
}
