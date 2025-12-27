use std::time::Duration;

use bevy::prelude::*;
use bevy::time::{Timer, TimerMode};
use map::{MapPlugin, GRID_PX, MAP_WIDTH};
use monsters::MonstersPlugin;
use player::PlayerPlugin;
use resources::ResourcesPlugin;
use viewshed::ViewshedPlugin;

mod combat;
mod components;
mod debug;
mod distance;
mod gamelog;
mod gui;
mod hunger;
mod inventory;
mod map;
mod map_builders;
mod map_indexing;
mod monsters;
mod particle;
mod pathfinding;
mod player;
mod resources;
mod rng;
mod saveload;
mod shapes;
mod spawner;
mod traps;
mod viewshed;

const SCREEN_HEIGHT: usize = 50;
const RESOLUTION: Vec2 = Vec2 {
    x: MAP_WIDTH as f32 * GRID_PX.x,
    y: SCREEN_HEIGHT as f32 * GRID_PX.y,
};

pub const SHOW_MAPGEN_VISUALIZER: bool = true;

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

#[derive(Resource, Default)]
pub struct MagicMapRevealRow(pub i32);

#[derive(Resource, Default)]
pub struct PendingMagicMap(pub bool);

#[derive(Resource, Default)]
pub struct TargetingInfo {
    pub range: i32,
    pub item: Option<Entity>,
}

// Map generation visualization resources
#[derive(Resource, Default)]
pub struct MapGenHistory(pub Vec<map::Map>);

#[derive(Resource, Default)]
pub struct MapGenIndex(pub usize);

#[derive(Resource)]
pub struct MapGenTimer(pub Timer);

impl Default for MapGenTimer {
    fn default() -> Self {
        Self(Timer::from_seconds(0.5, TimerMode::Repeating))
    }
}

#[derive(Resource, Default)]
pub struct MapGenSpawnData {
    pub starting_pos: (i32, i32),
    pub spawn_regions: Vec<shapes::Rect>,
    pub depth: i32,
    pub pending: bool,
}

#[derive(Resource, Default)]
pub struct MapGenBuilderName(pub String);

/// None = random builder, Some(index) = specific builder
#[derive(Resource, Default)]
pub struct SelectedBuilder(pub Option<usize>);

#[derive(Component)]
struct MapGenUI;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Rust Roguelike".into(),
                resolution: RESOLUTION.into(),
                ..default()
            }),
            ..default()
        }))
        .init_state::<RunState>()
        .init_resource::<gamelog::GameLog>()
        .init_resource::<rng::GameRng>()
        .init_resource::<TargetingInfo>()
        .init_resource::<MagicMapRevealRow>()
        .init_resource::<PendingMagicMap>()
        .init_resource::<particle::ParticleBuilder>()
        .init_resource::<MapGenHistory>()
        .init_resource::<MapGenIndex>()
        .init_resource::<MapGenTimer>()
        .init_resource::<MapGenSpawnData>()
        .init_resource::<MapGenBuilderName>()
        .init_resource::<SelectedBuilder>()
        .add_event::<AppExit>()
        .add_plugins((
            ResourcesPlugin,
            PlayerPlugin,
            ViewshedPlugin,
            MapPlugin,
            MonstersPlugin,
            gui::GuiPlugin,
            debug::DebugPlugin,
        ))
        .add_systems(Startup, setup)
        .add_systems(Update, handle_exit)
        .add_systems(
            Update,
            (
                map_indexing::map_indexing_system,
                particle::particle_spawn_system,
                particle::particle_cull_system,
                traps::reveal_hidden_system,
            )
                .run_if(not(in_state(RunState::MapGeneration))),
        )
        // MapGeneration: visualize map building
        .add_systems(OnEnter(RunState::MapGeneration), setup_mapgen_visualization)
        .add_systems(
            Update,
            (mapgen_visualization, mapgen_input).run_if(in_state(RunState::MapGeneration)),
        )
        .add_systems(OnExit(RunState::MapGeneration), (finalize_mapgen, cleanup_mapgen_ui))
        // PreRun: run systems then transition to AwaitingInput
        .add_systems(
            Update,
            transition_to_awaiting_input.run_if(in_state(RunState::PreRun)),
        )
        // PlayerTurn: run combat and item systems then transition to MonsterTurn
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
        // MonsterTurn: run monster AI then transition to AwaitingInput
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
        // NextLevel: generate new level and transition to PreRun
        .add_systems(
            Update,
            go_next_level.run_if(in_state(RunState::NextLevel)),
        )
        // MagicMapReveal: reveal map row by row
        .add_systems(
            OnEnter(RunState::MagicMapReveal),
            reset_magic_map_row,
        )
        .add_systems(
            Update,
            magic_map_reveal.run_if(in_state(RunState::MagicMapReveal)),
        )
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
}

fn handle_exit(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut exit: EventWriter<AppExit>,
    state: Res<State<RunState>>,
    map: Res<map::Map>,
    game_log: Res<gamelog::GameLog>,
    player_query: Query<
        (Entity, &map::Position, &components::Name, &combat::CombatStats, &viewshed::Viewshed, &components::HungerClock),
        With<player::Player>,
    >,
    monster_query: Query<
        (&map::Position, &components::Name, &combat::CombatStats, &viewshed::Viewshed, &Text2d, Option<&components::Confusion>),
        With<monsters::Monster>,
    >,
    item_query: Query<
        (
            Entity,
            &components::Name,
            &Text2d,
            &TextColor,
            Option<&map::Position>,
            Option<&components::InBackpack>,
            Option<&components::Consumable>,
            Option<&components::ProvidesHealing>,
            Option<&components::ProvidesFood>,
            Option<&components::Ranged>,
            Option<&components::InflictsDamage>,
            Option<&components::AreaOfEffect>,
            Option<&components::Targeting>,
            Option<&components::CausesConfusion>,
            Option<&components::MagicMapper>,
        ),
        With<components::Item>,
    >,
    trap_query: Query<
        (
            &map::Position,
            &components::Name,
            &Text2d,
            &TextColor,
            &components::InflictsDamage,
            Option<&components::Hidden>,
            Option<&components::SingleActivation>,
        ),
        With<components::EntryTrigger>,
    >,
) {
    if keyboard.just_released(KeyCode::KeyQ) {
        // Only save if we're in-game (not in MainMenu) and player is alive
        if *state.get() != RunState::MainMenu {
            let player_alive = player_query
                .get_single()
                .map(|(_, _, _, stats, _, _)| stats.hp > 0)
                .unwrap_or(false);

            if player_alive {
                saveload::save_game(
                    map,
                    game_log,
                    player_query,
                    monster_query,
                    item_query,
                    trap_query,
                );
            }
        }
        exit.send(AppExit::Success);
    }
}

fn transition_to_awaiting_input(
    mut next_state: ResMut<NextState<RunState>>,
    player_query: Query<&combat::CombatStats, With<player::Player>>,
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
    player_query: Query<&combat::CombatStats, With<player::Player>>,
) {
    // Don't transition if player is dead (GameOver state should take priority)
    if let Ok(stats) = player_query.get_single() {
        if stats.hp > 0 {
            // Check if magic map reveal is pending
            if pending_magic_map.0 {
                pending_magic_map.0 = false;
                next_state.set(RunState::MagicMapReveal);
            } else {
                next_state.set(RunState::MonsterTurn);
            }
        }
    }
}

fn go_next_level(
    mut commands: Commands,
    mut map: ResMut<map::Map>,
    mut gamelog: ResMut<gamelog::GameLog>,
    mut next_state: ResMut<NextState<RunState>>,
    font: Res<resources::UiFont>,
    mut rng: ResMut<rng::GameRng>,
    mut player_query: Query<(Entity, &mut combat::CombatStats), With<player::Player>>,
    backpack_query: Query<(Entity, &components::InBackpack)>,
    entities_to_delete: Query<
        Entity,
        Or<(With<monsters::Monster>, With<map::Tile>, With<components::Item>, With<components::EntryTrigger>)>,
    >,
) {
    // Get player entity and items in their backpack
    let Ok((player_entity, mut player_stats)) = player_query.get_single_mut() else {
        return;
    };
    let player_items: Vec<Entity> = backpack_query
        .iter()
        .filter(|(_, backpack)| backpack.owner == player_entity)
        .map(|(entity, _)| entity)
        .collect();

    // Delete all entities except player and their backpack items
    for entity in &entities_to_delete {
        if entity != player_entity && !player_items.contains(&entity) {
            commands.entity(entity).despawn_recursive();
        }
    }

    // Generate new map with increased depth using builder
    let new_depth = map.depth + 1;
    let mut builder = map_builders::random_builder(new_depth, &mut rng);
    builder.build_map(&mut rng);
    *map = builder.get_map();

    // Spawn new map tiles
    let text_font = TextFont {
        font: font.0.clone(),
        font_size: map::FONT_SIZE,
        ..default()
    };

    let mut y = 0;
    let mut x = 0;
    for tile in map.tiles.iter() {
        match tile {
            map::TileType::Floor => {
                commands.spawn((
                    map::Tile,
                    map::Position { x, y },
                    Text2d::new("."),
                    text_font.clone(),
                    TextColor(Color::srgb(0.5, 0.5, 0.5)),
                    map::Revealed(map::RevealedState::Hidden),
                ));
            }
            map::TileType::Wall => {
                // Only spawn walls adjacent to floors (boundary walls)
                if map.is_adjacent_to_floor(x, y) {
                    let glyph = map.wall_glyph_at(x, y);
                    commands.spawn((
                        map::Tile,
                        map::Position { x, y },
                        glyph,
                        Text2d::new(glyph.to_char().to_string()),
                        text_font.clone(),
                        TextColor(Color::srgb(0.0, 1.0, 0.0)),
                        map::Revealed(map::RevealedState::Hidden),
                    ));
                }
            }
            map::TileType::DownStairs => {
                commands.spawn((
                    map::Tile,
                    map::Position { x, y },
                    Text2d::new(">"),
                    text_font.clone(),
                    TextColor(Color::srgb(0.0, 1.0, 1.0)),
                    map::Revealed(map::RevealedState::Hidden),
                ));
            }
        }

        x += 1;
        if x > map::MAP_WIDTH as i32 - 1 {
            x = 0;
            y += 1;
        }
    }

    // Spawn monsters and items via builder
    builder.spawn_entities(&mut commands, &mut rng, &text_font);

    // Move player to starting position
    let (player_x, player_y) = builder.get_starting_position();
    commands
        .entity(player_entity)
        .insert(map::Position { x: player_x, y: player_y });

    // Heal player (restore up to 50% of max HP)
    let heal_amount = player_stats.max_hp / 2;
    player_stats.hp = (player_stats.hp + heal_amount).min(player_stats.max_hp);

    // Mark player's viewshed as dirty to recalculate visibility
    commands
        .entity(player_entity)
        .insert(viewshed::Viewshed {
            range: 8,
            visible_tiles: Vec::new(),
            dirty: true,
        });

    gamelog.entries.push(format!(
        "You descend to level {}. You feel slightly rejuvenated.",
        new_depth
    ));

    next_state.set(RunState::PreRun);
}

fn reset_magic_map_row(mut reveal_row: ResMut<MagicMapRevealRow>) {
    reveal_row.0 = 0;
}

fn magic_map_reveal(
    mut reveal_row: ResMut<MagicMapRevealRow>,
    mut map: ResMut<map::Map>,
    mut next_state: ResMut<NextState<RunState>>,
    mut tile_query: Query<(&map::Position, &mut map::Revealed), With<map::Tile>>,
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
            *revealed = map::Revealed(map::RevealedState::Explored);
        }
    }

    reveal_row.0 += 1;
}

fn setup_mapgen_visualization(
    mut commands: Commands,
    mut index: ResMut<MapGenIndex>,
    mut timer: ResMut<MapGenTimer>,
    builder_name: Res<MapGenBuilderName>,
    font: Res<resources::UiFont>,
    // Despawn all text entities to ensure clean slate
    text_query: Query<Entity, With<Text2d>>,
) {
    index.0 = 0;
    timer.0.reset();

    // Despawn any existing text entities before visualization starts
    for entity in &text_query {
        commands.entity(entity).despawn();
    }

    // Spawn UI showing builder name and controls
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            left: Val::Px(10.0),
            padding: UiRect::all(Val::Px(10.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.8)),
        MapGenUI,
    )).with_children(|parent| {
        parent.spawn((
            Text::new(format!("Builder: {}\n\nSpace: Regenerate\nEsc: Back", builder_name.0)),
            TextFont {
                font: font.0.clone(),
                font_size: 16.0,
                ..default()
            },
            TextColor(Color::srgb(1.0, 1.0, 0.0)),
        ));
    });
}

fn finalize_mapgen(
    mut commands: Commands,
    mut spawn_data: ResMut<MapGenSpawnData>,
    mut rng: ResMut<rng::GameRng>,
    font: Res<resources::UiFont>,
    map: Res<map::Map>,
    tile_query: Query<Entity, With<map::Tile>>,
) {
    if !spawn_data.pending {
        return;
    }

    // Despawn visualization tiles
    for entity in &tile_query {
        commands.entity(entity).despawn();
    }

    // Spawn final map tiles with proper hidden state
    let text_font = TextFont {
        font: font.0.clone(),
        font_size: map::FONT_SIZE,
        ..default()
    };

    for y in 0..map::MAP_HEIGHT as i32 {
        for x in 0..map::MAP_WIDTH as i32 {
            let idx = map.xy_idx(x, y);
            let tile = map.tiles[idx];

            match tile {
                map::TileType::Floor => {
                    commands.spawn((
                        map::Tile,
                        map::Position { x, y },
                        Text2d::new("."),
                        text_font.clone(),
                        TextColor(Color::srgb(0.5, 0.5, 0.5)),
                        map::Revealed(map::RevealedState::Hidden),
                    ));
                }
                map::TileType::Wall => {
                    if map.is_adjacent_to_floor(x, y) {
                        let glyph = map.wall_glyph_at(x, y);
                        commands.spawn((
                            map::Tile,
                            map::Position { x, y },
                            glyph,
                            Text2d::new(glyph.to_char().to_string()),
                            text_font.clone(),
                            TextColor(Color::srgb(0.0, 1.0, 0.0)),
                            map::Revealed(map::RevealedState::Hidden),
                        ));
                    }
                }
                map::TileType::DownStairs => {
                    commands.spawn((
                        map::Tile,
                        map::Position { x, y },
                        Text2d::new(">"),
                        text_font.clone(),
                        TextColor(Color::srgb(0.0, 1.0, 1.0)),
                        map::Revealed(map::RevealedState::Hidden),
                    ));
                }
            }
        }
    }

    // Spawn player at starting position
    let (player_x, player_y) = spawn_data.starting_pos;
    spawner::spawn_player(&mut commands, &text_font, player_x, player_y);

    // Spawn monsters and items in rooms (skip first room - player spawn)
    let mut monster_id: usize = 0;
    for room in spawn_data.spawn_regions.iter().skip(1) {
        spawner::spawn_room(&mut commands, &mut rng, &text_font, room, &mut monster_id, spawn_data.depth);
    }

    spawn_data.pending = false;
}

fn mapgen_visualization(
    mut commands: Commands,
    time: Res<Time>,
    mut timer: ResMut<MapGenTimer>,
    mut index: ResMut<MapGenIndex>,
    history: Res<MapGenHistory>,
    font: Res<resources::UiFont>,
    tile_query: Query<Entity, With<map::Tile>>,
) {
    timer.0.tick(time.delta());

    if !timer.0.just_finished() {
        return;
    }

    if index.0 >= history.0.len() {
        // Done - just stay on final frame (press Q to exit)
        return;
    }

    // Get current snapshot
    let snapshot = &history.0[index.0];

    // Skip snapshots with no floors (nothing interesting to show)
    let has_floors = snapshot.tiles.iter().any(|t| *t == map::TileType::Floor);
    if !has_floors {
        index.0 += 1;
        return;
    }

    // Despawn existing tiles before spawning new ones
    for entity in &tile_query {
        commands.entity(entity).despawn();
    }

    // Spawn tiles for this snapshot
    let text_font = TextFont {
        font: font.0.clone(),
        font_size: map::FONT_SIZE,
        ..default()
    };

    // Show only floors and walls during visualization (no stairs - they're gameplay elements)
    for y in 0..map::MAP_HEIGHT as i32 {
        for x in 0..map::MAP_WIDTH as i32 {
            let idx = snapshot.xy_idx(x, y);
            match snapshot.tiles[idx] {
                map::TileType::Floor => {
                    commands.spawn((
                        map::Tile,
                        map::Position { x, y },
                        Text2d::new("."),
                        text_font.clone(),
                        TextColor(Color::srgb(0.5, 0.5, 0.5)),
                        map::Revealed(map::RevealedState::Visible),
                    ));
                }
                map::TileType::Wall => {
                    // Only show walls adjacent to floors
                    if snapshot.is_adjacent_to_floor(x, y) {
                        let glyph = snapshot.wall_glyph_at(x, y);
                        commands.spawn((
                            map::Tile,
                            map::Position { x, y },
                            glyph,
                            Text2d::new(glyph.to_char().to_string()),
                            text_font.clone(),
                            TextColor(Color::srgb(0.0, 1.0, 0.0)),
                            map::Revealed(map::RevealedState::Visible),
                        ));
                    }
                }
                map::TileType::DownStairs => {
                    // Don't show stairs during visualization - treat as floor visually
                    commands.spawn((
                        map::Tile,
                        map::Position { x, y },
                        Text2d::new("."),
                        text_font.clone(),
                        TextColor(Color::srgb(0.5, 0.5, 0.5)),
                        map::Revealed(map::RevealedState::Visible),
                    ));
                }
            }
        }
    }

    index.0 += 1;
}

fn mapgen_input(
    mut commands: Commands,
    mut evr_kbd: EventReader<bevy::input::keyboard::KeyboardInput>,
    mut next_state: ResMut<NextState<RunState>>,
    mut map: ResMut<map::Map>,
    mut rng: ResMut<rng::GameRng>,
    mut mapgen_history: ResMut<MapGenHistory>,
    mut spawn_data: ResMut<MapGenSpawnData>,
    mut builder_name: ResMut<MapGenBuilderName>,
    mut index: ResMut<MapGenIndex>,
    mut timer: ResMut<MapGenTimer>,
    selected_builder: Res<SelectedBuilder>,
    font: Res<resources::UiFont>,
    tile_query: Query<Entity, With<map::Tile>>,
    ui_query: Query<Entity, With<MapGenUI>>,
) {
    use bevy::input::ButtonState;

    for ev in evr_kbd.read() {
        if ev.state != ButtonState::Pressed {
            continue;
        }

        match ev.key_code {
            KeyCode::Escape => {
                // Go back to builder selection menu
                spawn_data.pending = false;
                next_state.set(RunState::MapBuilderSelect);
            }
            KeyCode::Space => {
                // Regenerate map
                // Despawn existing tiles
                for entity in &tile_query {
                    commands.entity(entity).despawn();
                }

                // Despawn existing UI
                for entity in &ui_query {
                    commands.entity(entity).despawn_recursive();
                }

                // Generate new map using selected builder or random
                // Preserve pending state (true for new game, false for visualizer)
                let was_pending = spawn_data.pending;
                let mut builder = match selected_builder.0 {
                    Some(idx) => map_builders::builder_by_index(idx, 1),
                    None => map_builders::random_builder(1, &mut rng),
                };
                builder_name.0 = builder.get_name().to_string();
                builder.build_map(&mut rng);
                *map = builder.get_map();
                mapgen_history.0 = builder.get_snapshot_history();
                spawn_data.starting_pos = builder.get_starting_position();
                spawn_data.spawn_regions = builder.get_spawn_regions();
                spawn_data.depth = 1;
                spawn_data.pending = was_pending;

                // Reset visualization
                index.0 = 0;
                timer.0.reset();

                // Respawn UI with new builder name
                commands.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        top: Val::Px(10.0),
                        left: Val::Px(10.0),
                        padding: UiRect::all(Val::Px(10.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.8)),
                    MapGenUI,
                )).with_children(|parent| {
                    parent.spawn((
                        Text::new(format!("Builder: {}\n\nSpace: Regenerate\nEsc: Back", builder_name.0)),
                        TextFont {
                            font: font.0.clone(),
                            font_size: 16.0,
                            ..default()
                        },
                        TextColor(Color::srgb(1.0, 1.0, 0.0)),
                    ));
                });
            }
            _ => {}
        }
    }
}

fn cleanup_mapgen_ui(
    mut commands: Commands,
    ui_query: Query<Entity, With<MapGenUI>>,
) {
    for entity in &ui_query {
        commands.entity(entity).despawn_recursive();
    }
}

fn run_loop(mut app: App) -> AppExit {
    //let mut exit_event_reader = app.world().resource_mut::<Events<AppExit>>().get_cursor();

    loop {
        //let run_state = app.world().resource::<State<RunState>>();
        //if run_state.get() == &RunState::Running {
        app.update();
        //}

        // Check if we got an exit event, etc...
        if let Some(exit) = app.should_exit() {
            return exit;
        }

        // Check if we should exit
        {
            //let exit_events = app.world().resource::<Events<AppExit>>();
            //for exit in exit_event_reader.read(exit_events) {
            //    return exit.clone();
            //}

            //if !exit_events.is_empty() {
            //    // Return the first exit event found.
            //    if let Some(exit) = exit_events.iter().next() {
            //        return exit.clone();
            //    }
            //}
        }

        // Small sleep to avoid busy-looping (adjust as needed)
        std::thread::sleep(Duration::from_millis(16));
    }
}
