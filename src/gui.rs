use bevy::input::keyboard::KeyboardInput;
use bevy::input::ButtonState;
use bevy::prelude::*;

use crate::components::{EntryTrigger, Item};
use crate::gamelog::GameLog;
use crate::map::{Map, Position, Revealed, RevealedState, Tile, TileType, FONT_SIZE, MAP_WIDTH};
use crate::map_builders;
use crate::monsters::Monster;
use crate::player::Player;
use crate::resources::{MenuBackground, UiFont};
use crate::rng::GameRng;
use crate::saveload;
use crate::spawner;
use crate::ui::{BuilderMenu, BuilderMenuText, MainMenu, MenuPage, ITEMS_PER_PAGE};
use crate::{MapGenBuilderName, MapGenHistory, MapGenSpawnData, RunState, SelectedBuilder};

pub struct GuiPlugin;

impl Plugin for GuiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MenuPage>()
            // Main menu
            .add_systems(OnEnter(RunState::MainMenu), (cleanup_game_entities, spawn_main_menu).chain())
            .add_systems(OnExit(RunState::MainMenu), despawn_main_menu)
            .add_systems(
                Update,
                handle_main_menu_input.run_if(in_state(RunState::MainMenu)),
            )
            // Builder selection menu
            .add_systems(OnEnter(RunState::MapBuilderSelect), reset_menu_page)
            .add_systems(OnExit(RunState::MapBuilderSelect), despawn_builder_menu)
            .add_systems(
                Update,
                (spawn_builder_menu, handle_builder_menu_input).chain().run_if(in_state(RunState::MapBuilderSelect)),
            );
    }
}

fn reset_menu_page(mut menu_page: ResMut<MenuPage>) {
    menu_page.0 = 0;
}

// ============================================================================
// Main Menu
// ============================================================================

fn cleanup_game_entities(
    mut commands: Commands,
    entities: Query<Entity, Or<(With<Player>, With<Monster>, With<Item>, With<Tile>)>>,
    mut map: ResMut<Map>,
    mut game_log: ResMut<GameLog>,
) {
    // Despawn all game entities
    for entity in &entities {
        commands.entity(entity).despawn_recursive();
    }

    // Reset map to default (will be regenerated on New Game)
    *map = Map::default();

    // Clear game log
    game_log.entries.clear();
}

fn spawn_new_game_immediate(
    commands: &mut Commands,
    map: &mut ResMut<Map>,
    rng: &mut ResMut<GameRng>,
    font: &Res<UiFont>,
) {
    // Generate new map using random builder
    let mut builder = map_builders::random_builder(1, rng);
    builder.build_map(rng);
    *map.as_mut() = builder.get_map();

    let text_font = TextFont {
        font: font.0.clone(),
        font_size: FONT_SIZE,
        ..default()
    };

    // Spawn map tiles
    let mut y = 0;
    let mut x = 0;
    for tile in map.tiles.iter() {
        match tile {
            TileType::Floor => {
                commands.spawn((
                    Tile,
                    Position { x, y },
                    Text2d::new("."),
                    text_font.clone(),
                    TextColor(Color::srgb(0.5, 0.5, 0.5)),
                    Revealed(RevealedState::Hidden),
                ));
            }
            TileType::Wall => {
                if map.is_adjacent_to_floor(x, y) {
                    let glyph = map.wall_glyph_at(x, y);
                    commands.spawn((
                        Tile,
                        Position { x, y },
                        glyph,
                        Text2d::new(glyph.to_char().to_string()),
                        text_font.clone(),
                        TextColor(Color::srgb(0.0, 1.0, 0.0)),
                        Revealed(RevealedState::Hidden),
                    ));
                }
            }
            TileType::DownStairs => {
                commands.spawn((
                    Tile,
                    Position { x, y },
                    Text2d::new(">"),
                    text_font.clone(),
                    TextColor(Color::srgb(0.0, 1.0, 1.0)),
                    Revealed(RevealedState::Hidden),
                ));
            }
        }

        x += 1;
        if x > MAP_WIDTH as i32 - 1 {
            x = 0;
            y += 1;
        }
    }

    // Spawn player at starting position
    let (player_x, player_y) = builder.get_starting_position();
    spawner::spawn_player(commands, &text_font, player_x, player_y);

    // Spawn monsters and items via builder
    builder.spawn_entities(commands, rng, &text_font);
}

fn spawn_main_menu(mut commands: Commands, font: Res<UiFont>, background: Res<MenuBackground>) {
    let has_save = saveload::has_save_file();

    let menu_text = if has_save {
        "Rust Roguelike\n\n(N) New Game\n(C) Continue\n(V) Map Visualizer\n(Q) Quit"
    } else {
        "Rust Roguelike\n\n(N) New Game\n(V) Map Visualizer\n(Q) Quit"
    };

    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            ImageNode::new(background.0.clone()),
            MainMenu,
        ))
        .with_children(|parent| {
            parent
                .spawn((
                    Node {
                        padding: UiRect::all(Val::Px(30.0)),
                        border: UiRect::all(Val::Px(2.0)),
                        ..default()
                    },
                    BorderColor(Color::WHITE),
                    BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)),
                ))
                .with_children(|menu| {
                    menu.spawn((
                        Text::new(menu_text),
                        TextFont {
                            font: font.0.clone(),
                            font_size: 20.0,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));
                });
        });
}

fn despawn_main_menu(mut commands: Commands, menu_query: Query<Entity, With<MainMenu>>) {
    for entity in &menu_query {
        commands.entity(entity).despawn_recursive();
    }
}

fn handle_main_menu_input(
    mut commands: Commands,
    mut evr_kbd: EventReader<KeyboardInput>,
    mut next_state: ResMut<NextState<RunState>>,
    mut exit: EventWriter<AppExit>,
    // Resources needed for loading/new game
    entities_to_despawn: Query<Entity, Or<(With<Player>, With<Monster>, With<Item>, With<Tile>, With<EntryTrigger>)>>,
    mut map: ResMut<Map>,
    mut game_log: ResMut<GameLog>,
    mut rng: ResMut<crate::rng::GameRng>,
    font: Res<UiFont>,
    mut selected_builder: ResMut<SelectedBuilder>,
) {
    for ev in evr_kbd.read() {
        if ev.state != ButtonState::Pressed {
            continue;
        }

        match ev.key_code {
            KeyCode::KeyN => {
                // New Game - start game immediately (skip visualizer)
                selected_builder.0 = None;
                spawn_new_game_immediate(&mut commands, &mut map, &mut rng, &font);
                next_state.set(RunState::PreRun);
            }
            KeyCode::KeyC => {
                // Continue - load from save file
                if saveload::has_save_file() {
                    if saveload::load_game(
                        &mut commands,
                        &entities_to_despawn,
                        &mut map,
                        &mut game_log,
                        &font,
                    ) {
                        next_state.set(RunState::PreRun);
                    }
                }
            }
            KeyCode::KeyV => {
                // Map Visualizer - go to builder selection
                next_state.set(RunState::MapBuilderSelect);
            }
            KeyCode::KeyQ => {
                exit.send(AppExit::Success);
            }
            _ => {}
        }
    }
}

// ============================================================================
// Builder Selection Menu
// ============================================================================

fn build_builder_menu_text(menu_page: &MenuPage) -> String {
    let builder_names = map_builders::get_builder_names();
    let total_items = builder_names.len();
    let total_pages = (total_items + ITEMS_PER_PAGE - 1) / ITEMS_PER_PAGE;
    let current_page = menu_page.0.min(total_pages.saturating_sub(1));
    let start_idx = current_page * ITEMS_PER_PAGE;

    let items: Vec<String> = builder_names
        .iter()
        .enumerate()
        .skip(start_idx)
        .take(ITEMS_PER_PAGE)
        .map(|(i, name)| format!("({}) {}", (b'a' + (i - start_idx) as u8) as char, name))
        .collect();

    let page_info = if total_pages > 1 {
        format!("\n\nPage {}/{} (</> to navigate)", current_page + 1, total_pages)
    } else {
        String::new()
    };

    format!(
        "Select Map Builder\n\n{}{}\n\n(R) Random | (Esc) Back",
        items.join("\n"),
        page_info
    )
}

fn spawn_builder_menu(
    mut commands: Commands,
    font: Res<UiFont>,
    menu_page: Res<MenuPage>,
    background: Res<MenuBackground>,
    existing_menu: Query<Entity, With<BuilderMenu>>,
) {
    // Don't spawn if menu already exists
    if !existing_menu.is_empty() {
        return;
    }

    let menu_text = build_builder_menu_text(&menu_page);

    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            ImageNode::new(background.0.clone()),
            BuilderMenu,
        ))
        .with_children(|parent| {
            parent
                .spawn((
                    Node {
                        padding: UiRect::all(Val::Px(30.0)),
                        border: UiRect::all(Val::Px(2.0)),
                        ..default()
                    },
                    BorderColor(Color::WHITE),
                    BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)),
                ))
                .with_children(|menu| {
                    menu.spawn((
                        Text::new(menu_text),
                        TextFont {
                            font: font.0.clone(),
                            font_size: 16.0,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                        BuilderMenuText,
                    ));
                });
        });
}

fn despawn_builder_menu(mut commands: Commands, menu_query: Query<Entity, With<BuilderMenu>>) {
    for entity in &menu_query {
        commands.entity(entity).despawn_recursive();
    }
}

fn handle_builder_menu_input(
    mut evr_kbd: EventReader<KeyboardInput>,
    mut next_state: ResMut<NextState<RunState>>,
    mut menu_page: ResMut<MenuPage>,
    mut selected_builder: ResMut<SelectedBuilder>,
    mut map: ResMut<Map>,
    mut rng: ResMut<GameRng>,
    mut mapgen_history: ResMut<MapGenHistory>,
    mut spawn_data: ResMut<MapGenSpawnData>,
    mut builder_name: ResMut<MapGenBuilderName>,
    mut menu_text_query: Query<&mut Text, With<BuilderMenuText>>,
) {
    let builder_names = map_builders::get_builder_names();
    let total_items = builder_names.len();
    let total_pages = (total_items + ITEMS_PER_PAGE - 1) / ITEMS_PER_PAGE;
    let current_page = menu_page.0.min(total_pages.saturating_sub(1));
    let start_idx = current_page * ITEMS_PER_PAGE;

    for ev in evr_kbd.read() {
        if ev.state != ButtonState::Pressed {
            continue;
        }

        match ev.key_code {
            KeyCode::Escape => {
                next_state.set(RunState::MainMenu);
            }
            KeyCode::Comma => {
                if menu_page.0 > 0 {
                    menu_page.0 -= 1;
                    // Update text in place
                    if let Ok(mut text) = menu_text_query.get_single_mut() {
                        **text = build_builder_menu_text(&menu_page);
                    }
                }
            }
            KeyCode::Period => {
                if menu_page.0 < total_pages.saturating_sub(1) {
                    menu_page.0 += 1;
                    // Update text in place
                    if let Ok(mut text) = menu_text_query.get_single_mut() {
                        **text = build_builder_menu_text(&menu_page);
                    }
                }
            }
            KeyCode::KeyR => {
                // Random builder
                selected_builder.0 = None;
                start_visualizer(&mut map, &mut rng, &mut mapgen_history, &mut spawn_data, &mut builder_name, None);
                next_state.set(RunState::MapGeneration);
            }
            KeyCode::KeyA => try_select_builder(start_idx + 0, total_items, &mut selected_builder, &mut map, &mut rng, &mut mapgen_history, &mut spawn_data, &mut builder_name, &mut next_state),
            KeyCode::KeyB => try_select_builder(start_idx + 1, total_items, &mut selected_builder, &mut map, &mut rng, &mut mapgen_history, &mut spawn_data, &mut builder_name, &mut next_state),
            KeyCode::KeyC => try_select_builder(start_idx + 2, total_items, &mut selected_builder, &mut map, &mut rng, &mut mapgen_history, &mut spawn_data, &mut builder_name, &mut next_state),
            KeyCode::KeyD => try_select_builder(start_idx + 3, total_items, &mut selected_builder, &mut map, &mut rng, &mut mapgen_history, &mut spawn_data, &mut builder_name, &mut next_state),
            KeyCode::KeyE => try_select_builder(start_idx + 4, total_items, &mut selected_builder, &mut map, &mut rng, &mut mapgen_history, &mut spawn_data, &mut builder_name, &mut next_state),
            KeyCode::KeyF => try_select_builder(start_idx + 5, total_items, &mut selected_builder, &mut map, &mut rng, &mut mapgen_history, &mut spawn_data, &mut builder_name, &mut next_state),
            KeyCode::KeyG => try_select_builder(start_idx + 6, total_items, &mut selected_builder, &mut map, &mut rng, &mut mapgen_history, &mut spawn_data, &mut builder_name, &mut next_state),
            KeyCode::KeyH => try_select_builder(start_idx + 7, total_items, &mut selected_builder, &mut map, &mut rng, &mut mapgen_history, &mut spawn_data, &mut builder_name, &mut next_state),
            KeyCode::KeyI => try_select_builder(start_idx + 8, total_items, &mut selected_builder, &mut map, &mut rng, &mut mapgen_history, &mut spawn_data, &mut builder_name, &mut next_state),
            KeyCode::KeyJ => try_select_builder(start_idx + 9, total_items, &mut selected_builder, &mut map, &mut rng, &mut mapgen_history, &mut spawn_data, &mut builder_name, &mut next_state),
            _ => {}
        }
    }
}

fn try_select_builder(
    index: usize,
    total_items: usize,
    selected_builder: &mut ResMut<SelectedBuilder>,
    map: &mut ResMut<Map>,
    rng: &mut ResMut<GameRng>,
    mapgen_history: &mut ResMut<MapGenHistory>,
    spawn_data: &mut ResMut<MapGenSpawnData>,
    builder_name: &mut ResMut<MapGenBuilderName>,
    next_state: &mut ResMut<NextState<RunState>>,
) {
    if index < total_items {
        selected_builder.0 = Some(index);
        start_visualizer(map, rng, mapgen_history, spawn_data, builder_name, Some(index));
        next_state.set(RunState::MapGeneration);
    }
}

fn start_visualizer(
    map: &mut ResMut<Map>,
    rng: &mut ResMut<GameRng>,
    mapgen_history: &mut ResMut<MapGenHistory>,
    spawn_data: &mut ResMut<MapGenSpawnData>,
    builder_name: &mut ResMut<MapGenBuilderName>,
    builder_index: Option<usize>,
) {
    let mut builder = match builder_index {
        Some(idx) => map_builders::builder_by_index(idx, 1),
        None => map_builders::random_builder(1, rng),
    };
    builder_name.0 = builder.get_name().to_string();
    builder.build_map(rng);
    *map.as_mut() = builder.get_map();
    mapgen_history.0 = builder.get_snapshot_history();
    spawn_data.starting_pos = builder.get_starting_position();
    spawn_data.spawn_regions = builder.get_spawn_regions();
    spawn_data.depth = 1;
    spawn_data.pending = false; // Don't spawn entities in visualizer mode
}


