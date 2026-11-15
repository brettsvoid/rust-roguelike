use bevy::prelude::*;

/// Marker component for friendly/neutral NPCs that don't attack.
/// Bumping into bystanders swaps positions instead of initiating combat.
#[derive(Component, Debug)]
pub struct Bystander;

pub struct BystanderPlugin;

impl Plugin for BystanderPlugin {
    fn build(&self, _app: &mut App) {
        // Bystanders don't need AI - they just stand around
        // Position swapping is handled in player.rs
    }
}
