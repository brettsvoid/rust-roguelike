use bevy::input::keyboard::KeyboardInput;
use bevy::input::ButtonState;
use bevy::prelude::*;

use crate::components::{EntryTrigger, Item};
use crate::gamelog::GameLog;
use crate::levels;
use crate::map::{Map, Tile};
use crate::map_builders;
use crate::mapgen_viz::{self, MapGenBuilderName, MapGenHistory, SelectedBuilder};
use crate::monsters::Monster;
use crate::player::Player;
use crate::resources::{MenuBackground, UiFont};
use crate::rng::GameRng;
use crate::saveload;
use crate::ui::{BuilderMenu, BuilderMenuText, MainMenu, MenuPage, ITEMS_PER_PAGE};
use crate::RunState;

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
                levels::spawn_new_game(&mut commands, &mut map, &mut rng, &font);
                next_state.set(RunState::PreRun);
            }
            KeyCode::KeyC => {
                // Continue - load from save file
                if saveload::has_save_file()
                    && saveload::load_game(
                        &mut commands,
                        &entities_to_despawn,
                        &mut map,
                        &mut game_log,
                        &font,
                    ) {
                        next_state.set(RunState::PreRun);
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
    let total_pages = total_items.div_ceil(ITEMS_PER_PAGE);
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

/// Map a menu letter key (a-j) to its slot on the current page.
fn menu_slot(key: KeyCode) -> Option<usize> {
    match key {
        KeyCode::KeyA => Some(0),
        KeyCode::KeyB => Some(1),
        KeyCode::KeyC => Some(2),
        KeyCode::KeyD => Some(3),
        KeyCode::KeyE => Some(4),
        KeyCode::KeyF => Some(5),
        KeyCode::KeyG => Some(6),
        KeyCode::KeyH => Some(7),
        KeyCode::KeyI => Some(8),
        KeyCode::KeyJ => Some(9),
        _ => None,
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
    mut builder_name: ResMut<MapGenBuilderName>,
    mut menu_text_query: Query<&mut Text, With<BuilderMenuText>>,
) {
    let total_items = map_builders::get_builder_names().len();
    let total_pages = total_items.div_ceil(ITEMS_PER_PAGE);
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
                mapgen_viz::start_preview(&mut map, &mut rng, &mut mapgen_history, &mut builder_name, None);
                next_state.set(RunState::MapGeneration);
            }
            key => {
                if let Some(slot) = menu_slot(key) {
                    let index = start_idx + slot;
                    if index < total_items {
                        selected_builder.0 = Some(index);
                        mapgen_viz::start_preview(
                            &mut map,
                            &mut rng,
                            &mut mapgen_history,
                            &mut builder_name,
                            Some(index),
                        );
                        next_state.set(RunState::MapGeneration);
                    }
                }
            }
        }
    }
}


