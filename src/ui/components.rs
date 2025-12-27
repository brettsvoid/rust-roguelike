use bevy::prelude::*;

// ============================================================================
// HUD Components
// ============================================================================

#[derive(Component)]
pub struct HealthText;

#[derive(Component)]
pub struct HealthBar;

#[derive(Component)]
pub struct DepthText;

#[derive(Component)]
pub struct HungerText;

#[derive(Component)]
pub struct GameLogText;

// ============================================================================
// Menu Components (used by gui.rs for MainMenu and BuilderMenu)
// ============================================================================

#[derive(Component)]
pub struct MainMenu;

#[derive(Component)]
pub struct BuilderMenu;

#[derive(Component)]
pub struct BuilderMenuText;

// ============================================================================
// Tooltip Components
// ============================================================================

#[derive(Component)]
pub struct Tooltip;

#[derive(Component)]
pub struct CursorHighlight;

// ============================================================================
// Targeting Components
// ============================================================================

#[derive(Component)]
pub struct TargetingMenu;

#[derive(Component)]
pub struct TargetHighlight;

#[derive(Component)]
pub struct TargetBorder;

#[derive(Component)]
pub struct RangeIndicator;
