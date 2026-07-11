//! The turn-based state machine that drives the game.
//!
//! A normal turn cycles `AwaitingInput -> PlayerTurn -> MonsterTurn -> AwaitingInput`.
//! Everything else (menus, inventory screens, level transitions, the magic map
//! reveal) is a detour from that loop.

use bevy::prelude::*;

use crate::map::{self, Map, Position, Revealed, RevealedState, Tile};
use crate::player::Player;
use crate::{combat, hunger, inventory, levels, monsters, traps};

#[derive(States, Clone, Copy, Default, Eq, PartialEq, Debug, Hash)]
pub enum RunState {
    #[default]
    MainMenu,
    MapBuilderSelect,
    MapGeneration,
    PreRun,
    AwaitingInput,
    PlayerTurn,
    MonsterTurn,
    ShowInventory,
    ShowDropItem,
    ShowRemoveItem,
    ShowTargeting,
    NextLevel,
    MagicMapReveal,
    GameOver,
}

/// Set when a scroll of magic mapping is read; picked up at end of player turn.
#[derive(Resource, Default)]
pub struct PendingMagicMap(pub bool);

/// Which map row the magic map animation has revealed so far.
#[derive(Resource, Default)]
pub struct MagicMapRevealRow(pub i32);

pub struct GameFlowPlugin;

impl Plugin for GameFlowPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<RunState>()
            .init_resource::<PendingMagicMap>()
            .init_resource::<MagicMapRevealRow>()
            // PreRun: one frame of setup, then hand control to the player
            .add_systems(
                Update,
                transition_to_awaiting_input.run_if(in_state(RunState::PreRun)),
            )
            // PlayerTurn: resolve the player's action, then the monsters act
            .add_systems(
                Update,
                (
                    traps::trap_trigger_system,
                    inventory::item_collection_system,
                    inventory::item_use_system,
                    inventory::item_drop_system,
                    inventory::item_remove_system,
                    combat::melee_combat_system,
                    combat::damage_system,
                    combat::delete_the_dead,
                    hunger::hunger_system,
                    transition_to_monster_turn,
                )
                    .chain()
                    .run_if(in_state(RunState::PlayerTurn)),
            )
            // MonsterTurn: run monster AI, then wait for input again
            .add_systems(
                Update,
                (
                    monsters::monster_ai,
                    traps::trap_trigger_system,
                    combat::melee_combat_system,
                    combat::damage_system,
                    combat::delete_the_dead,
                    transition_to_awaiting_input,
                )
                    .chain()
                    .run_if(in_state(RunState::MonsterTurn)),
            )
            // NextLevel: tear down the old level and build the next one
            .add_systems(
                Update,
                levels::go_next_level.run_if(in_state(RunState::NextLevel)),
            )
            // MagicMapReveal: uncover the map one row per frame
            .add_systems(OnEnter(RunState::MagicMapReveal), reset_magic_map_row)
            .add_systems(
                Update,
                magic_map_reveal.run_if(in_state(RunState::MagicMapReveal)),
            );
    }
}

fn transition_to_awaiting_input(
    mut next_state: ResMut<NextState<RunState>>,
    player_query: Query<&combat::CombatStats, With<Player>>,
) {
    // Don't transition if player is dead (GameOver state should take priority)
    if let Ok(stats) = player_query.get_single() {
        if stats.hp > 0 {
            next_state.set(RunState::AwaitingInput);
        }
    }
}

fn transition_to_monster_turn(
    mut next_state: ResMut<NextState<RunState>>,
    mut pending_magic_map: ResMut<PendingMagicMap>,
    player_query: Query<&combat::CombatStats, With<Player>>,
) {
    // Don't transition if player is dead (GameOver state should take priority)
    if let Ok(stats) = player_query.get_single() {
        if stats.hp > 0 {
            if pending_magic_map.0 {
                pending_magic_map.0 = false;
                next_state.set(RunState::MagicMapReveal);
            } else {
                next_state.set(RunState::MonsterTurn);
            }
        }
    }
}

fn reset_magic_map_row(mut reveal_row: ResMut<MagicMapRevealRow>) {
    reveal_row.0 = 0;
}

fn magic_map_reveal(
    mut reveal_row: ResMut<MagicMapRevealRow>,
    mut map: ResMut<Map>,
    mut next_state: ResMut<NextState<RunState>>,
    mut tile_query: Query<(&Position, &mut Revealed), With<Tile>>,
) {
    let row = reveal_row.0;

    if row >= map::MAP_HEIGHT as i32 {
        // Done revealing, return to awaiting input
        next_state.set(RunState::AwaitingInput);
        return;
    }

    // Reveal all tiles in this row
    for x in 0..map::MAP_WIDTH as i32 {
        let idx = map.xy_idx(x, row);
        map.revealed_tiles[idx] = true;
    }

    // Update tile entities in this row
    for (pos, mut revealed) in &mut tile_query {
        if pos.y == row {
            *revealed = Revealed(RevealedState::Explored);
        }
    }

    reveal_row.0 += 1;
}
