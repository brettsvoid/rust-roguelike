//! The map generation visualizer (main menu -> Map Visualizer).
//!
//! Builders record a snapshot of the map after each significant step; this
//! module replays those snapshots like a flipbook so you can watch the
//! algorithm carve out the level. Purely a dev/curiosity tool - the game
//! itself never enters this state.

use bevy::input::keyboard::KeyboardInput;
use bevy::input::ButtonState;
use bevy::prelude::*;
use bevy::time::{Timer, TimerMode};

use crate::game_flow::RunState;
use crate::map::{self, Map, Tile, FONT_SIZE};
use crate::map_builders;
use crate::map_render::{spawn_map_tiles, TileReveal};
use crate::resources::UiFont;
use crate::rng::GameRng;

/// The snapshot flipbook recorded by the last builder run.
#[derive(Resource, Default)]
pub struct MapGenHistory(pub Vec<Map>);

/// Which snapshot the playback is currently showing.
#[derive(Resource, Default)]
pub struct MapGenIndex(pub usize);

/// Controls playback speed (one snapshot per tick).
#[derive(Resource)]
pub struct MapGenTimer(pub Timer);

impl Default for MapGenTimer {
    fn default() -> Self {
        Self(Timer::from_seconds(0.5, TimerMode::Repeating))
    }
}

/// Display name of the builder being visualized.
#[derive(Resource, Default)]
pub struct MapGenBuilderName(pub String);

/// None = random builder, Some(index) = specific builder from the menu.
#[derive(Resource, Default)]
pub struct SelectedBuilder(pub Option<usize>);

#[derive(Component)]
struct MapGenUI;

pub struct MapGenVizPlugin;

impl Plugin for MapGenVizPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MapGenHistory>()
            .init_resource::<MapGenIndex>()
            .init_resource::<MapGenTimer>()
            .init_resource::<MapGenBuilderName>()
            .init_resource::<SelectedBuilder>()
            .add_systems(OnEnter(RunState::MapGeneration), setup_visualization)
            .add_systems(
                Update,
                (playback, handle_input).run_if(in_state(RunState::MapGeneration)),
            )
            .add_systems(OnExit(RunState::MapGeneration), cleanup_ui);
    }
}

/// Run a builder and stash its output for playback. Called from the builder
/// menu (gui.rs) and again on Space to regenerate.
pub fn start_preview(
    map: &mut Map,
    rng: &mut GameRng,
    history: &mut MapGenHistory,
    builder_name: &mut MapGenBuilderName,
    selected: Option<usize>,
) {
    let mut builder = match selected {
        Some(idx) => map_builders::builder_by_index(idx, 1),
        None => map_builders::random_builder(1, rng),
    };
    builder_name.0 = builder.get_name().to_string();
    builder.build_map(rng);
    *map = builder.get_map();
    history.0 = builder.get_snapshot_history();
}

fn spawn_overlay_ui(commands: &mut Commands, font: &UiFont, builder_name: &str) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(10.0),
                left: Val::Px(10.0),
                padding: UiRect::all(Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.8)),
            MapGenUI,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new(format!(
                    "Builder: {}\n\nSpace: Regenerate\nEsc: Back",
                    builder_name
                )),
                TextFont {
                    font: font.0.clone(),
                    font_size: 16.0,
                    ..default()
                },
                TextColor(Color::srgb(1.0, 1.0, 0.0)),
            ));
        });
}

fn setup_visualization(
    mut commands: Commands,
    mut index: ResMut<MapGenIndex>,
    mut timer: ResMut<MapGenTimer>,
    builder_name: Res<MapGenBuilderName>,
    font: Res<UiFont>,
    // Despawn all text entities to ensure a clean slate
    text_query: Query<Entity, With<Text2d>>,
) {
    index.0 = 0;
    timer.0.reset();

    for entity in &text_query {
        commands.entity(entity).despawn();
    }

    spawn_overlay_ui(&mut commands, &font, &builder_name.0);
}

/// Advance the flipbook: every timer tick, despawn the current tiles and
/// draw the next snapshot.
fn playback(
    mut commands: Commands,
    time: Res<Time>,
    mut timer: ResMut<MapGenTimer>,
    mut index: ResMut<MapGenIndex>,
    history: Res<MapGenHistory>,
    font: Res<UiFont>,
    tile_query: Query<Entity, With<Tile>>,
) {
    timer.0.tick(time.delta());

    if !timer.0.just_finished() {
        return;
    }

    if index.0 >= history.0.len() {
        // Done - stay on the final frame
        return;
    }

    let snapshot = &history.0[index.0];

    // Skip snapshots with no floors (nothing interesting to show)
    let has_floors = snapshot
        .tiles.contains(&map::TileType::Floor);
    if !has_floors {
        index.0 += 1;
        return;
    }

    for entity in &tile_query {
        commands.entity(entity).despawn();
    }

    let text_font = TextFont {
        font: font.0.clone(),
        font_size: FONT_SIZE,
        ..default()
    };
    spawn_map_tiles(&mut commands, snapshot, &text_font, TileReveal::Preview);

    index.0 += 1;
}

fn handle_input(
    mut commands: Commands,
    mut evr_kbd: EventReader<KeyboardInput>,
    mut next_state: ResMut<NextState<RunState>>,
    mut map: ResMut<Map>,
    mut rng: ResMut<GameRng>,
    mut history: ResMut<MapGenHistory>,
    mut builder_name: ResMut<MapGenBuilderName>,
    mut index: ResMut<MapGenIndex>,
    mut timer: ResMut<MapGenTimer>,
    selected_builder: Res<SelectedBuilder>,
    font: Res<UiFont>,
    tile_query: Query<Entity, With<Tile>>,
    ui_query: Query<Entity, With<MapGenUI>>,
) {
    for ev in evr_kbd.read() {
        if ev.state != ButtonState::Pressed {
            continue;
        }

        match ev.key_code {
            KeyCode::Escape => {
                next_state.set(RunState::MapBuilderSelect);
            }
            KeyCode::Space => {
                // Regenerate with the same selection and restart playback
                for entity in &tile_query {
                    commands.entity(entity).despawn();
                }
                for entity in &ui_query {
                    commands.entity(entity).despawn_recursive();
                }

                start_preview(
                    &mut map,
                    &mut rng,
                    &mut history,
                    &mut builder_name,
                    selected_builder.0,
                );

                index.0 = 0;
                timer.0.reset();
                spawn_overlay_ui(&mut commands, &font, &builder_name.0);
            }
            _ => {}
        }
    }
}

fn cleanup_ui(mut commands: Commands, ui_query: Query<Entity, With<MapGenUI>>) {
    for entity in &ui_query {
        commands.entity(entity).despawn_recursive();
    }
}
