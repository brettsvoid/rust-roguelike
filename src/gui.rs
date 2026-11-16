use bevy::input::keyboard::KeyboardInput;
use bevy::input::ButtonState;
use bevy::prelude::*;

use crate::components::Item;
use crate::economy::town::TownState;
use crate::gamelog::GameLog;
use crate::levels;
use crate::map::{Map, Tile};
use crate::map_builders;
use crate::mapgen_viz::{self, MapGenBuilderName, MapGenHistory, SelectedBuilder};
use crate::monsters::Monster;
use crate::party::PartyMember;
use crate::picking::HoveredTile;
use crate::resources::{MenuBackground, UiFont};
use crate::rng::GameRng;
use crate::saveload;
use crate::ui::style::{self, font, under_cursor};
use crate::ui::{BuilderMenu, BuilderMenuText, MainMenu, MenuPage, ITEMS_PER_PAGE};
use crate::RunState;

pub struct GuiPlugin;

impl Plugin for GuiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MenuPage>()
            .add_plugins(crate::menu_scene::MenuScenePlugin)
            // Main menu
            .add_systems(OnEnter(RunState::MainMenu), (cleanup_game_entities, spawn_main_menu).chain())
            .add_systems(OnExit(RunState::MainMenu), despawn_main_menu)
            .add_systems(
                Update,
                (handle_main_menu_input, style_main_menu).chain().run_if(in_state(RunState::MainMenu)),
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
    entities: Query<Entity, Or<(With<PartyMember>, With<Monster>, With<Item>, With<Tile>)>>,
    mut map: ResMut<Map>,
    mut game_log: ResMut<GameLog>,
) {
    // Despawn all game entities
    for entity in &entities {
        commands.entity(entity).despawn();
    }

    // Reset map to default (will be regenerated on New Game)
    *map = Map::default();

    // Clear game log
    game_log.entries.clear();
}

/// A main menu entry, top to bottom.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Entry {
    NewGame,
    Continue,
    Visualiser,
    Quit,
}

impl Entry {
    const ALL: [Entry; 4] = [Entry::NewGame, Entry::Continue, Entry::Visualiser, Entry::Quit];

    fn label(self) -> &'static str {
        match self {
            Entry::NewGame => "New Game",
            Entry::Continue => "Continue",
            Entry::Visualiser => "Map Visualiser",
            Entry::Quit => "Quit",
        }
    }

    fn key(self) -> KeyCode {
        match self {
            Entry::NewGame => KeyCode::KeyN,
            Entry::Continue => KeyCode::KeyC,
            Entry::Visualiser => KeyCode::KeyV,
            Entry::Quit => KeyCode::KeyQ,
        }
    }

    fn cap(self) -> &'static str {
        match self {
            Entry::NewGame => "N",
            Entry::Continue => "C",
            Entry::Visualiser => "V",
            Entry::Quit => "Q",
        }
    }

    fn enabled(self, has_save: bool) -> bool {
        self != Entry::Continue || has_save
    }
}

/// The entry the arrow keys move to, skipping disabled ones and wrapping.
fn step_entry(from: Entry, down: bool, has_save: bool) -> Entry {
    let at = Entry::ALL.iter().position(|&e| e == from).unwrap_or(0);
    let n = Entry::ALL.len();
    (1..=n)
        .map(|i| Entry::ALL[if down { (at + i) % n } else { (at + n - i) % n }])
        .find(|e| e.enabled(has_save))
        .unwrap_or(from)
}

/// The picked entry and whether there is a save, set on entering the menu.
#[derive(Resource)]
struct MenuChoice {
    selected: Entry,
    has_save: bool,
    /// Seconds since the menu opened, for the fade in.
    age: f32,
}

#[derive(Component)]
struct MenuEntry(Entry);
#[derive(Component)]
struct MenuEntryLabel(Entry);
#[derive(Component)]
struct MenuEntryCap(Entry);
#[derive(Component)]
struct MenuVeil;

/// A one-line notice under the entries, empty unless something sets its
/// `Text` (such as a damaged save set aside).
#[derive(Component)]
pub struct MainMenuNotice;

/// Seconds the menu takes to fade in from black.
const MENU_FADE: f32 = 1.2;

fn spawn_main_menu(mut commands: Commands, font_handle: Res<UiFont>, mut game_log: ResMut<GameLog>) {
    // A save that cannot be loaded is set aside here; the reason goes on the notice line.
    let (has_save, report) = match saveload::save_status() {
        Ok(has_save) => (has_save, String::new()),
        Err(report) => {
            game_log.entries.push(report.clone());
            (false, report)
        }
    };
    commands.insert_resource(MenuChoice {
        selected: if has_save { Entry::Continue } else { Entry::NewGame },
        has_save,
        age: 0.0,
    });
    let f = &font_handle.0;
    // Dark on the left, where the menu sits, clearing over the room on the right.
    let shade = |alpha: f32| style::PANEL.with_alpha(alpha);
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                padding: UiRect::left(Val::Percent(7.0)),
                ..default()
            },
            BackgroundGradient::from(LinearGradient::to_right(vec![
                ColorStop::percent(shade(0.92), 0.0),
                ColorStop::percent(shade(0.7), 28.0),
                ColorStop::percent(shade(0.0), 58.0),
            ])),
            // Spawned in the same frame as the HUD, so without this their draw order is arbitrary.
            GlobalZIndex(1),
            MainMenu,
        ))
        .with_children(|menu| {
            menu.spawn((Text::new("The Abyss Below"), font(f, style::FONT_TITLE), TextColor(style::ACCENT)));
            menu.spawn((
                Node { margin: UiRect::new(Val::Px(4.0), Val::Px(0.0), Val::Px(6.0), Val::Px(40.0)), ..default() },
                Text::new("Twelve levels down lies the Heart. Bring it home."),
                font(f, style::FONT_ROW),
                TextColor(style::TEXT_DIM),
            ));
            for entry in Entry::ALL {
                menu.spawn((
                    Node {
                        width: Val::Px(340.0),
                        height: Val::Px(46.0),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::SpaceBetween,
                        padding: UiRect::axes(Val::Px(16.0), Val::Px(0.0)),
                        margin: UiRect::bottom(Val::Px(4.0)),
                        border: UiRect::left(Val::Px(3.0)),
                        border_radius: BorderRadius::all(Val::Px(style::RADIUS)),
                        ..default()
                    },
                    BackgroundColor(Color::NONE),
                    BorderColor::all(Color::NONE),
                    MenuEntry(entry),
                ))
                .with_children(|row| {
                    row.spawn((Text::new(entry.label()), font(f, style::FONT_MENU), TextColor(style::TEXT), MenuEntryLabel(entry)));
                    let caption = if entry.enabled(has_save) { entry.cap() } else { "No saved game" };
                    row.spawn((Text::new(caption), font(f, style::FONT_BODY), TextColor(style::TEXT_FAINT), MenuEntryCap(entry)));
                });
            }
            menu.spawn((
                Node { margin: UiRect::new(Val::Px(4.0), Val::Px(0.0), Val::Px(14.0), Val::Px(0.0)), min_height: Val::Px(18.0), ..default() },
                Text::new(report),
                font(f, style::FONT_ROW),
                TextColor(style::BADGE_HUNGRY),
                MainMenuNotice,
            ));
            menu.spawn((
                Node { margin: UiRect::new(Val::Px(4.0), Val::Px(0.0), Val::Px(24.0), Val::Px(0.0)), ..default() },
                Text::new("Up and Down to choose, Enter to select"),
                font(f, style::FONT_BODY),
                TextColor(style::TEXT_FAINT),
            ));
            // Over the whole menu, fading out as it opens.
            menu.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor(Color::BLACK),
                GlobalZIndex(3),
                MenuVeil,
            ));
        });
}

fn despawn_main_menu(mut commands: Commands, menu_query: Query<Entity, With<MainMenu>>) {
    for entity in &menu_query {
        commands.entity(entity).despawn();
    }
}

/// Lights the picked entry, dims a disabled one and fades the menu in.
fn style_main_menu(
    mut commands: Commands,
    time: Res<Time>,
    mut choice: ResMut<MenuChoice>,
    mut entries: Query<(&MenuEntry, &mut BackgroundColor, &mut BorderColor), Without<MenuVeil>>,
    mut labels: Query<(&MenuEntryLabel, &mut TextColor), Without<MenuEntryCap>>,
    mut caps: Query<(&MenuEntryCap, &mut TextColor), Without<MenuEntryLabel>>,
    mut veil: Query<(Entity, &mut BackgroundColor), With<MenuVeil>>,
) {
    choice.age += time.delta_secs();
    for (MenuEntry(entry), mut background, mut border) in &mut entries {
        let picked = *entry == choice.selected;
        background.set_if_neq(BackgroundColor(if picked { style::ROW_SELECTED } else { Color::NONE }));
        border.set_if_neq(BorderColor::all(if picked { style::ACCENT } else { Color::NONE }));
    }
    for (MenuEntryLabel(entry), mut colour) in &mut labels {
        let tint = match (entry.enabled(choice.has_save), *entry == choice.selected) {
            (false, _) => style::TEXT_FAINT,
            (true, true) => style::ACCENT,
            (true, false) => style::TEXT,
        };
        colour.set_if_neq(TextColor(tint));
    }
    for (MenuEntryCap(entry), mut colour) in &mut caps {
        let tint = if *entry == choice.selected { style::TEXT_DIM } else { style::TEXT_FAINT };
        colour.set_if_neq(TextColor(tint));
    }
    for (entity, mut colour) in &mut veil {
        let left = 1.0 - style::ease(choice.age / MENU_FADE);
        if left <= 0.0 {
            commands.entity(entity).despawn();
        } else {
            colour.0 = Color::BLACK.with_alpha(left);
        }
    }
}

/// N, C, V and Q take their entry at once; the arrow keys and the mouse move
/// the selection, and Enter or a click takes it.
#[allow(clippy::too_many_arguments)]
fn handle_main_menu_input(
    mut commands: Commands,
    mut evr_kbd: MessageReader<KeyboardInput>,
    mut next_state: ResMut<NextState<RunState>>,
    mut exit: MessageWriter<AppExit>,
    (mut map, mut rng, font, mut selected_builder, mut town): (
        ResMut<Map>,
        ResMut<GameRng>,
        Res<UiFont>,
        ResMut<SelectedBuilder>,
        ResMut<TownState>,
    ),
    (mouse, hovered, camera, mut choice): (
        Res<ButtonInput<MouseButton>>,
        Res<HoveredTile>,
        Single<&Camera, With<Camera3d>>,
        ResMut<MenuChoice>,
    ),
    rows: Query<(&MenuEntry, &bevy::ui::ComputedNode, &bevy::ui::UiGlobalTransform)>,
) {
    let has_save = choice.has_save;
    let mut taken = None;
    let under = hovered.cursor.and_then(|c| {
        rows.iter().find(|(_, node, transform)| under_cursor(c, &camera, node, transform)).map(|(e, ..)| e.0)
    });
    if let Some(entry) = under.filter(|e| e.enabled(has_save)) {
        choice.selected = entry;
        if mouse.just_pressed(MouseButton::Left) {
            taken = Some(entry);
        }
    }
    for ev in evr_kbd.read() {
        if ev.state != ButtonState::Pressed {
            continue;
        }
        match ev.key_code {
            KeyCode::ArrowDown => choice.selected = step_entry(choice.selected, true, has_save),
            KeyCode::ArrowUp => choice.selected = step_entry(choice.selected, false, has_save),
            KeyCode::Enter | KeyCode::NumpadEnter => taken = Some(choice.selected),
            key => taken = Entry::ALL.into_iter().find(|e| e.key() == key).or(taken),
        }
    }
    match taken {
        Some(Entry::NewGame) => {
            // New Game - start game immediately (skip visualizer)
            selected_builder.0 = None;
            levels::spawn_new_game(&mut commands, &mut map, &mut rng, &font, &mut town, levels::TOWN_DEPTH);
            next_state.set_if_different(RunState::PreRun);
        }
        Some(Entry::Continue) => {
            if saveload::has_save_file() {
                commands.queue(saveload::continue_game);
            }
        }
        Some(Entry::Visualiser) => next_state.set_if_different(RunState::MapBuilderSelect),
        Some(Entry::Quit) => {
            exit.write(AppExit::Success);
        }
        None => {}
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
            ImageNode::new(background.0.clone()).with_mode(NodeImageMode::Stretch),
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
                    BorderColor::all(Color::WHITE),
                    BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)),
                ))
                .with_children(|menu| {
                    menu.spawn((
                        Text::new(menu_text),
                        TextFont {
                            font: font.0.clone().into(),
                            font_size: FontSize::Px(16.0),
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
        commands.entity(entity).despawn();
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
    mut evr_kbd: MessageReader<KeyboardInput>,
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
                next_state.set_if_different(RunState::MainMenu);
            }
            KeyCode::Comma => {
                if menu_page.0 > 0 {
                    menu_page.0 -= 1;
                    // Update text in place
                    if let Ok(mut text) = menu_text_query.single_mut() {
                        **text = build_builder_menu_text(&menu_page);
                    }
                }
            }
            KeyCode::Period => {
                if menu_page.0 < total_pages.saturating_sub(1) {
                    menu_page.0 += 1;
                    // Update text in place
                    if let Ok(mut text) = menu_text_query.single_mut() {
                        **text = build_builder_menu_text(&menu_page);
                    }
                }
            }
            KeyCode::KeyR => {
                // Random builder
                selected_builder.0 = None;
                mapgen_viz::start_preview(&mut map, &mut rng, &mut mapgen_history, &mut builder_name, None);
                next_state.set_if_different(RunState::MapGeneration);
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
                        next_state.set_if_different(RunState::MapGeneration);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_arrows_skip_continue_without_a_save_and_wrap() {
        assert_eq!(step_entry(Entry::NewGame, true, false), Entry::Visualiser);
        assert_eq!(step_entry(Entry::NewGame, true, true), Entry::Continue);
        assert_eq!(step_entry(Entry::Quit, true, false), Entry::NewGame);
        assert_eq!(step_entry(Entry::NewGame, false, false), Entry::Quit);
        assert_eq!(step_entry(Entry::Visualiser, false, false), Entry::NewGame);
    }
}
