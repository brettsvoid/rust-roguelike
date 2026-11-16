//! The HUD's shared colours and sizes, in one place so a restyle (P03)
//! changes them here and not in every panel.

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
