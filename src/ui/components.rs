use bevy::prelude::*;

// ============================================================================
// HUD Components
// ============================================================================

#[derive(Component)]
pub struct DepthText;

#[derive(Component)]
pub struct GoldText;

/// Shows the tactical mode: explore, or the combat phase and round.
#[derive(Component)]
pub struct ModeText;

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

// ============================================================================
// Targeting Components
// ============================================================================

#[derive(Component)]
pub struct TargetHighlight;

#[derive(Component)]
pub struct TargetBorder;

#[derive(Component)]
pub struct RangeIndicator;
