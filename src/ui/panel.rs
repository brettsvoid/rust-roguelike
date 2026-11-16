//! The shared modal panel. The inventory, drop and remove menus, the shop,
//! the ledger, game over and the targeting banner all draw one.
//!
//! A screen builds a `PanelSpec` from the game state (a pure function, so it
//! is tested) and writes it to its panel with `show`; `render_panels` redraws
//! a panel whose spec changed. Rows, tabs and footer hints answer the mouse,
//! and the arrow keys and Enter pick a row, by pressing the key they stand
//! for (`PanelKey`), so a click goes through the screen's own key handler.
//! A panel eases in when spawned and out after `close_panels`.

use bevy::input::keyboard::{Key, KeyboardInput, NativeKey};
use bevy::input::ButtonState;
use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform};
use bevy::window::PrimaryWindow;

use crate::picking::{BlocksPicking, HoveredTile};
use crate::resources::UiFont;
use crate::RunState;

use super::resources::{MenuPage, ITEMS_PER_PAGE};
use super::style::{self, font, under_cursor};

pub struct PanelPlugin;

impl Plugin for PanelPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PanelPress>()
            .add_systems(Update, (render_panels, steer_panels, press_panel_keys, ease_panels).chain());
    }
}

/// The keys that pick the rows of a page, in order.
pub const ROW_KEYS: [KeyCode; ITEMS_PER_PAGE] = [
    KeyCode::KeyA,
    KeyCode::KeyB,
    KeyCode::KeyC,
    KeyCode::KeyD,
    KeyCode::KeyE,
    KeyCode::KeyF,
    KeyCode::KeyG,
    KeyCode::KeyH,
    KeyCode::KeyI,
    KeyCode::KeyJ,
];

/// Where a panel sits: centred over the dimmed world, or a banner at the
/// top that leaves the world to click on.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Placement {
    Modal,
    Banner,
}

/// What a panel shows.
#[derive(Component, Clone, PartialEq, Debug, Default)]
pub struct PanelSpec {
    pub title: String,
    /// Beside the title, dimmed: whose it is, the purse, the range.
    pub aside: String,
    /// The title in the hostile colour (game over).
    pub alarm: bool,
    pub width: f32,
    pub tabs: Vec<Tab>,
    /// The rows of the current page; row `i` is picked with `ROW_KEYS[i]`.
    pub rows: Vec<Row>,
    pub body: Vec<BodyLine>,
    /// Shown when there are neither rows nor body.
    pub empty: String,
    /// The last result (a trade), over the footer.
    pub message: String,
    /// The page (from 0) and the page count, when there is more than one.
    pub page: Option<(usize, usize)>,
    pub hints: Vec<Hint>,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Row {
    pub label: String,
    /// Right-aligned and dimmed: a price, what is left.
    pub detail: String,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Tab {
    pub label: String,
    pub active: bool,
    /// What a click presses to switch to it.
    pub keys: Vec<KeyCode>,
}

/// A footer hint: the key cap, what it does, and the key a click presses.
#[derive(Clone, PartialEq, Debug)]
pub struct Hint {
    pub cap: String,
    pub action: String,
    pub key: Option<KeyCode>,
}

impl Hint {
    pub fn new(cap: &str, action: &str, key: KeyCode) -> Self {
        Hint { cap: cap.into(), action: action.into(), key: Some(key) }
    }

    /// The `a-j` hint for the rows; a click on a row does it.
    pub fn rows(rows: usize, action: &str) -> Self {
        let last = (b'a' + rows.clamp(1, ITEMS_PER_PAGE) as u8 - 1) as char;
        let cap = if rows > 1 { format!("a-{last}") } else { "a".into() };
        Hint { cap, action: action.into(), key: None }
    }

    pub fn close() -> Self {
        Hint::new("Esc", "Close", KeyCode::Escape)
    }
}

#[derive(Clone, PartialEq, Debug)]
pub enum BodyLine {
    Heading(String),
    Text(String),
    /// A label, a progress bar filled to `fraction` and a dimmed detail.
    Meter { label: String, fraction: f32, detail: String },
}

/// The rows of `page` and, when there are several pages, the page and the
/// count. A page past the end shows the last.
pub fn page_of<T: Clone>(items: &[T], page: usize) -> (Vec<T>, Option<(usize, usize)>) {
    let pages = items.len().div_ceil(ITEMS_PER_PAGE).max(1);
    let page = page.min(pages - 1);
    let shown = items.iter().skip(page * ITEMS_PER_PAGE).take(ITEMS_PER_PAGE).cloned().collect();
    (shown, (pages > 1).then_some((page, pages)))
}

/// The footer hints for turning pages, when there are several.
pub fn page_hints(page: Option<(usize, usize)>) -> Vec<Hint> {
    match page {
        Some(_) => vec![Hint::new(",", "Previous", KeyCode::Comma), Hint::new(".", "Next", KeyCode::Period)],
        None => vec![],
    }
}

/// The item index a row key picks on the current page.
pub fn get_selected_index(key_code: KeyCode, menu_page: &MenuPage, total_items: usize) -> Option<usize> {
    let offset = ROW_KEYS.iter().position(|&k| k == key_code)?;
    let index = menu_page.0 * ITEMS_PER_PAGE + offset;
    (index < total_items).then_some(index)
}

/// Turns the page on `,` and `.`; true when it changed.
pub fn handle_pagination_input(key_code: KeyCode, menu_page: &mut MenuPage, total_items: usize) -> bool {
    let pages = total_items.div_ceil(ITEMS_PER_PAGE).max(1);
    match key_code {
        KeyCode::Comma if menu_page.0 > 0 => menu_page.0 -= 1,
        KeyCode::Period if menu_page.0 + 1 < pages => menu_page.0 += 1,
        _ => return false,
    }
    true
}

/// The row the arrow keys move to among `rows`, wrapping round.
pub fn step_selection(selected: Option<usize>, rows: usize, down: bool) -> Option<usize> {
    if rows == 0 {
        return None;
    }
    Some(match (selected.filter(|&i| i < rows), down) {
        (None, true) => 0,
        (None, false) => rows - 1,
        (Some(i), true) => (i + 1) % rows,
        (Some(i), false) => (i + rows - 1) % rows,
    })
}

/// An open or closing panel and its parts.
#[derive(Component)]
pub struct Panel {
    placement: Placement,
    /// 0 (hidden) to 1 (shown).
    shown: f32,
    closing: bool,
    selected: Option<usize>,
    card: Entity,
    content: Entity,
    veil: Entity,
}

/// A row, tab or hint: what a click on it presses, and its panel.
#[derive(Component)]
struct PanelKey {
    panel: Entity,
    keys: Vec<KeyCode>,
}

#[derive(Component)]
struct PanelRow(usize);

/// Keys the panels press this frame, written as key presses.
#[derive(Resource, Default)]
struct PanelPress(Vec<KeyCode>);

/// Spawns an empty panel; it draws once its spec is written.
pub fn spawn_panel(commands: &mut Commands, placement: Placement) -> Entity {
    let modal = placement == Placement::Modal;
    let content = commands.spawn(Node { flex_direction: FlexDirection::Column, ..default() }).id();
    let veil = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                top: Val::Px(0.0),
                bottom: Val::Px(0.0),
                border_radius: BorderRadius::all(Val::Px(style::RADIUS)),
                ..default()
            },
            BackgroundColor(style::PANEL),
        ))
        .id();
    let card = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(style::RADIUS)),
                overflow: Overflow::clip(),
                // Room for the bottom bar and a margin.
                max_height: Val::Vh(78.0),
                ..default()
            },
            BackgroundColor(style::PANEL),
            BorderColor::all(style::PANEL_EDGE),
            UiTransform::from_scale(Vec2::splat(style::PANEL_START_SCALE)),
            BlocksPicking,
        ))
        .add_children(&[content, veil])
        .id();
    let mut root = commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            align_items: if modal { AlignItems::Center } else { AlignItems::FlexStart },
            padding: UiRect::top(Val::Px(if modal { 0.0 } else { 14.0 })),
            ..default()
        },
        // Over the HUD.
        GlobalZIndex(2),
        BackgroundColor(Color::NONE),
        PanelSpec::default(),
    ));
    if modal {
        root.insert(BlocksPicking);
    }
    let root = root.add_child(card).id();
    commands.entity(root).insert(Panel {
        placement,
        shown: 0.0,
        closing: false,
        selected: None,
        card,
        content,
        veil,
    });
    root
}

/// Shows `spec` on the open panels (there is one per screen).
pub fn show(panels: &mut Query<(&Panel, &mut PanelSpec)>, spec: PanelSpec) {
    for (panel, mut shown) in panels {
        if !panel.closing {
            shown.set_if_neq(spec.clone());
        }
    }
}

/// Eases every open panel out; each despawns once hidden. Every screen runs
/// it on leaving its state.
pub fn close_panels(mut commands: Commands, mut panels: Query<(Entity, &mut Panel)>) {
    for (entity, mut panel) in &mut panels {
        panel.closing = true;
        commands.entity(entity).remove::<BlocksPicking>();
        commands.entity(panel.card).remove::<BlocksPicking>();
    }
}

/// Escape goes back to play: for screens with nothing else to press.
pub fn escape_to_play(mut keys: MessageReader<KeyboardInput>, mut next_state: ResMut<NextState<RunState>>) {
    if keys.read().any(|ev| ev.state == ButtonState::Pressed && ev.key_code == KeyCode::Escape) {
        next_state.set_if_different(RunState::AwaitingInput);
    }
}

fn render_panels(
    mut commands: Commands,
    font_handle: Res<UiFont>,
    mut panels: Query<(Entity, &mut Panel, &PanelSpec), Changed<PanelSpec>>,
    mut cards: Query<&mut Node, Without<Panel>>,
) {
    let f = &font_handle.0;
    for (entity, mut panel, spec) in &mut panels {
        if spec.width > 0.0 {
            if let Ok(mut card) = cards.get_mut(panel.card) {
                card.width = Val::Px(spec.width);
            }
        }
        panel.selected = panel.selected.filter(|&i| i < spec.rows.len());
        let selected = panel.selected;
        commands.entity(panel.content).despawn_children().with_children(|content| {
            title_bar(content, f, spec);
            if !spec.tabs.is_empty() {
                tabs(content, f, entity, &spec.tabs);
            }
            let has_list = !spec.rows.is_empty() || !spec.body.is_empty();
            content
                .spawn(Node {
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::axes(Val::Px(style::PANEL_PADDING - 6.0), Val::Px(10.0)),
                    row_gap: Val::Px(2.0),
                    flex_shrink: 1.0,
                    min_height: Val::Px(0.0),
                    overflow: Overflow::clip(),
                    ..default()
                })
                .with_children(|list| {
                    for (i, row) in spec.rows.iter().enumerate() {
                        spawn_row(list, f, entity, i, row, selected == Some(i));
                    }
                    for line in &spec.body {
                        spawn_body_line(list, f, line);
                    }
                    if !has_list && !spec.empty.is_empty() {
                        list.spawn((
                            Node { padding: UiRect::axes(Val::Px(6.0), Val::Px(6.0)), ..default() },
                            Text::new(spec.empty.clone()),
                            font(f, style::FONT_ROW),
                            TextColor(style::TEXT_DIM),
                        ));
                    }
                });
            if !spec.message.is_empty() {
                content.spawn((
                    Node { padding: UiRect::new(Val::Px(style::PANEL_PADDING), Val::Px(style::PANEL_PADDING), Val::Px(0.0), Val::Px(10.0)), ..default() },
                    Text::new(spec.message.clone()),
                    font(f, style::FONT_BODY),
                    TextColor(style::ACCENT),
                ));
            }
            if !spec.hints.is_empty() || spec.page.is_some() {
                footer(content, f, entity, spec);
            }
        });
    }
}

fn title_bar(content: &mut ChildSpawnerCommands, f: &Handle<Font>, spec: &PanelSpec) {
    content
        .spawn((
            Node {
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Baseline,
                column_gap: Val::Px(24.0),
                padding: UiRect::axes(Val::Px(style::PANEL_PADDING), Val::Px(11.0)),
                border: UiRect::bottom(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(style::PANEL_BAND),
            BorderColor::all(style::PANEL_EDGE),
        ))
        .with_children(|bar| {
            let tint = if spec.alarm { style::HOSTILE } else { style::ACCENT };
            bar.spawn((Text::new(spec.title.clone()), font(f, style::FONT_HEADING), TextColor(tint)));
            if !spec.aside.is_empty() {
                bar.spawn((Text::new(spec.aside.clone()), font(f, style::FONT_BODY), TextColor(style::TEXT_DIM)));
            }
        });
}

fn tabs(content: &mut ChildSpawnerCommands, f: &Handle<Font>, panel: Entity, tabs: &[Tab]) {
    content
        .spawn((
            Node {
                column_gap: Val::Px(4.0),
                padding: UiRect::new(Val::Px(style::PANEL_PADDING - 6.0), Val::Px(style::PANEL_PADDING), Val::Px(8.0), Val::Px(0.0)),
                border: UiRect::bottom(Val::Px(1.0)),
                ..default()
            },
            BorderColor::all(style::PANEL_EDGE),
        ))
        .with_children(|bar| {
            for tab in tabs {
                bar.spawn((
                    Node {
                        padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                        border: UiRect::bottom(Val::Px(2.0)),
                        border_radius: BorderRadius::top(Val::Px(style::RADIUS)),
                        ..default()
                    },
                    BackgroundColor(Color::NONE),
                    BorderColor::all(if tab.active { style::ACCENT } else { Color::NONE }),
                    PanelKey { panel, keys: tab.keys.clone() },
                ))
                .with_child((
                    Text::new(tab.label.clone()),
                    font(f, style::FONT_ROW),
                    TextColor(if tab.active { style::TEXT } else { style::TEXT_DIM }),
                ));
            }
        });
}

fn key_cap(parent: &mut ChildSpawnerCommands, f: &Handle<Font>, cap: &str) {
    parent
        .spawn((
            Node {
                min_width: Val::Px(style::KEY_CAP_WIDTH),
                height: Val::Px(18.0),
                padding: UiRect::horizontal(Val::Px(4.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                flex_shrink: 0.0,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(style::KEY_CAP),
            BorderColor::all(style::BUTTON_EDGE),
        ))
        .with_child((Text::new(cap), font(f, style::FONT_SMALL), TextColor(style::TEXT_DIM)));
}

fn spawn_row(list: &mut ChildSpawnerCommands, f: &Handle<Font>, panel: Entity, i: usize, row: &Row, selected: bool) {
    list.spawn((
        Node {
            min_height: Val::Px(style::ROW_HEIGHT),
            align_items: AlignItems::Center,
            column_gap: Val::Px(10.0),
            padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)),
            border: UiRect::left(Val::Px(2.0)),
            border_radius: BorderRadius::all(Val::Px(style::RADIUS)),
            ..default()
        },
        BackgroundColor(if selected { style::ROW_SELECTED } else { Color::NONE }),
        BorderColor::all(if selected { style::ACCENT } else { Color::NONE }),
        PanelKey { panel, keys: vec![ROW_KEYS[i]] },
        PanelRow(i),
    ))
    .with_children(|r| {
        key_cap(r, f, &((b'a' + i as u8) as char).to_string());
        r.spawn((
            Node { flex_grow: 1.0, ..default() },
            Text::new(row.label.clone()),
            font(f, style::FONT_ROW),
            TextColor(style::TEXT),
        ));
        if !row.detail.is_empty() {
            r.spawn((Text::new(row.detail.clone()), font(f, style::FONT_BODY), TextColor(style::TEXT_DIM)));
        }
    });
}

fn spawn_body_line(list: &mut ChildSpawnerCommands, f: &Handle<Font>, line: &BodyLine) {
    let pad = UiRect::horizontal(Val::Px(6.0));
    match line {
        BodyLine::Heading(text) => {
            list.spawn((
                Node { padding: UiRect::new(Val::Px(6.0), Val::Px(6.0), Val::Px(10.0), Val::Px(4.0)), ..default() },
                Text::new(text.to_uppercase()),
                font(f, style::FONT_SMALL),
                TextColor(style::ACCENT.with_alpha(0.8)),
            ));
        }
        BodyLine::Text(text) => {
            list.spawn((
                Node { padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)), ..default() },
                Text::new(text.clone()),
                font(f, style::FONT_BODY),
                TextColor(style::TEXT),
            ));
        }
        BodyLine::Meter { label, fraction, detail } => {
            list.spawn(Node { padding: pad, column_gap: Val::Px(12.0), align_items: AlignItems::Center, min_height: Val::Px(22.0), ..default() })
                .with_children(|row| {
                    row.spawn((
                        Node { width: Val::Px(150.0), flex_shrink: 0.0, ..default() },
                        Text::new(label.clone()),
                        font(f, style::FONT_ROW),
                        TextColor(style::TEXT),
                    ));
                    row.spawn((
                        Node {
                            width: Val::Px(120.0),
                            height: Val::Px(6.0),
                            flex_shrink: 0.0,
                            border_radius: BorderRadius::all(Val::Px(3.0)),
                            overflow: Overflow::clip(),
                            ..default()
                        },
                        BackgroundColor(style::TRACK),
                    ))
                    .with_child((
                        Node { width: Val::Percent(100.0 * fraction.clamp(0.0, 1.0)), height: Val::Percent(100.0), ..default() },
                        BackgroundColor(style::ACCENT),
                    ));
                    row.spawn((Text::new(detail.clone()), font(f, style::FONT_BODY), TextColor(style::TEXT_DIM)));
                });
        }
    }
}

fn footer(content: &mut ChildSpawnerCommands, f: &Handle<Font>, panel: Entity, spec: &PanelSpec) {
    content
        .spawn((
            Node {
                column_gap: Val::Px(14.0),
                align_items: AlignItems::Center,
                flex_wrap: FlexWrap::Wrap,
                row_gap: Val::Px(6.0),
                padding: UiRect::axes(Val::Px(style::PANEL_PADDING), Val::Px(9.0)),
                border: UiRect::top(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(style::PANEL_BAND),
            BorderColor::all(style::PANEL_EDGE),
        ))
        .with_children(|bar| {
            for hint in &spec.hints {
                let mut node = bar.spawn((
                    Node {
                        column_gap: Val::Px(6.0),
                        align_items: AlignItems::Center,
                        padding: UiRect::axes(Val::Px(4.0), Val::Px(2.0)),
                        border_radius: BorderRadius::all(Val::Px(style::RADIUS)),
                        ..default()
                    },
                    BackgroundColor(Color::NONE),
                ));
                if let Some(key) = hint.key {
                    node.insert(PanelKey { panel, keys: vec![key] });
                }
                node.with_children(|h| {
                    key_cap(h, f, &hint.cap);
                    h.spawn((Text::new(hint.action.clone()), font(f, style::FONT_BODY), TextColor(style::TEXT_DIM)));
                });
            }
            if let Some((page, pages)) = spec.page {
                bar.spawn((
                    Node { margin: UiRect::left(Val::Auto), ..default() },
                    Text::new(format!("Page {} of {pages}", page + 1)),
                    font(f, style::FONT_BODY),
                    TextColor(style::TEXT_FAINT),
                ));
            }
        });
}

/// Hover and the arrow keys pick a row; a click on a row, tab or hint, or
/// Enter on the picked row, presses its key. Rows light when picked, tabs
/// and hints under the cursor.
fn steer_panels(
    mut keys: MessageReader<KeyboardInput>,
    mouse: Res<ButtonInput<MouseButton>>,
    hovered: Res<HoveredTile>,
    camera: Single<&Camera, With<Camera3d>>,
    mut panels: Query<(&mut Panel, &PanelSpec)>,
    mut nodes: Query<(&PanelKey, Option<&PanelRow>, &ComputedNode, &UiGlobalTransform, &mut BackgroundColor, &mut BorderColor)>,
    mut press: ResMut<PanelPress>,
    mut last_hover: Local<Option<(Entity, usize)>>,
) {
    let mut hover = None;
    let over = |node: &ComputedNode, transform: &UiGlobalTransform| {
        hovered.cursor.is_some_and(|c| under_cursor(c, &camera, node, transform))
    };
    let pressed: Vec<KeyCode> = keys.read().filter(|ev| ev.state == ButtonState::Pressed).map(|ev| ev.key_code).collect();
    for (key, row, node, transform, ..) in &nodes {
        let Ok((mut panel, _)) = panels.get_mut(key.panel) else { continue };
        if panel.closing || !over(node, transform) {
            continue;
        }
        // A row picks itself when the cursor comes onto it, not while it rests there.
        if let Some(PanelRow(i)) = row {
            hover = Some((key.panel, *i));
            if hover != *last_hover {
                panel.selected = Some(*i);
            }
        }
        if mouse.just_pressed(MouseButton::Left) {
            press.0.extend(&key.keys);
        }
    }
    *last_hover = hover;
    for (mut panel, spec) in &mut panels {
        if panel.closing {
            continue;
        }
        for key in &pressed {
            match key {
                KeyCode::ArrowDown | KeyCode::ArrowUp => {
                    panel.selected = step_selection(panel.selected, spec.rows.len(), *key == KeyCode::ArrowDown);
                }
                KeyCode::Enter | KeyCode::NumpadEnter => {
                    if let Some(i) = panel.selected.filter(|&i| i < spec.rows.len()) {
                        press.0.push(ROW_KEYS[i]);
                    }
                }
                _ => {}
            }
        }
    }
    for (key, row, node, transform, mut background, mut border) in &mut nodes {
        let Ok((panel, _)) = panels.get(key.panel) else { continue };
        let colour = match row {
            Some(PanelRow(i)) if panel.selected == Some(*i) => style::ROW_SELECTED,
            Some(_) => Color::NONE,
            None if !panel.closing && over(node, transform) => style::CARD_HOVER,
            None => Color::NONE,
        };
        background.set_if_neq(BackgroundColor(colour));
        if row.is_some() {
            let edge = if colour == style::ROW_SELECTED { style::ACCENT } else { Color::NONE };
            border.set_if_neq(BorderColor::all(edge));
        }
    }
}

/// Writes the keys the panels pressed as key presses, read by the screens'
/// own handlers like a real key.
fn press_panel_keys(
    mut press: ResMut<PanelPress>,
    window: Option<Single<Entity, With<PrimaryWindow>>>,
    mut keyboard: MessageWriter<KeyboardInput>,
) {
    let Some(window) = window else {
        press.0.clear();
        return;
    };
    for key_code in press.0.drain(..) {
        for state in [ButtonState::Pressed, ButtonState::Released] {
            keyboard.write(KeyboardInput {
                key_code,
                logical_key: Key::Unidentified(NativeKey::Unidentified),
                state,
                text: None,
                repeat: false,
                window: *window,
            });
        }
    }
}

/// Fades the backdrop and the contents in and scales the card up, or the
/// reverse while closing; a closed panel despawns.
fn ease_panels(
    mut commands: Commands,
    time: Res<Time>,
    mut panels: Query<(Entity, &mut Panel, &mut BackgroundColor)>,
    mut cards: Query<&mut UiTransform>,
    mut veils: Query<&mut BackgroundColor, Without<Panel>>,
) {
    let step = time.delta_secs() / style::PANEL_EASE;
    for (entity, mut panel, mut backdrop) in &mut panels {
        let target = if panel.closing { 0.0 } else { 1.0 };
        if panel.shown == target {
            if panel.closing {
                commands.entity(entity).despawn();
            }
            continue;
        }
        panel.shown = if panel.closing { (panel.shown - step).max(0.0) } else { (panel.shown + step).min(1.0) };
        let shown = style::ease(panel.shown);
        if panel.placement == Placement::Modal {
            backdrop.0 = style::BACKDROP.with_alpha(style::BACKDROP.alpha() * shown);
        }
        if let Ok(mut transform) = cards.get_mut(panel.card) {
            transform.scale = Vec2::splat(style::PANEL_START_SCALE + (1.0 - style::PANEL_START_SCALE) * shown);
        }
        if let Ok(mut veil) = veils.get_mut(panel.veil) {
            veil.0 = style::PANEL.with_alpha(1.0 - shown);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pages_hold_ten_rows_and_clamp_to_the_last() {
        let items: Vec<u32> = (0..23).collect();
        assert_eq!(page_of(&items, 0), ((0..10).collect(), Some((0, 3))));
        assert_eq!(page_of(&items, 2), ((20..23).collect(), Some((2, 3))));
        assert_eq!(page_of(&items, 9), ((20..23).collect(), Some((2, 3))));
        assert_eq!(page_of(&items[..4], 0), ((0..4).collect(), None));
        assert_eq!(page_of::<u32>(&[], 0), (vec![], None));
    }

    #[test]
    fn row_keys_pick_on_the_current_page() {
        assert_eq!(get_selected_index(KeyCode::KeyA, &MenuPage(0), 3), Some(0));
        assert_eq!(get_selected_index(KeyCode::KeyC, &MenuPage(0), 3), Some(2));
        assert_eq!(get_selected_index(KeyCode::KeyD, &MenuPage(0), 3), None);
        assert_eq!(get_selected_index(KeyCode::KeyB, &MenuPage(1), 15), Some(11));
        assert_eq!(get_selected_index(KeyCode::KeyK, &MenuPage(0), 30), None);
    }

    #[test]
    fn pages_turn_within_bounds() {
        let mut page = MenuPage(0);
        assert!(!handle_pagination_input(KeyCode::Comma, &mut page, 25));
        assert!(handle_pagination_input(KeyCode::Period, &mut page, 25));
        assert!(handle_pagination_input(KeyCode::Period, &mut page, 25));
        assert!(!handle_pagination_input(KeyCode::Period, &mut page, 25));
        assert_eq!(page.0, 2);
        assert!(!handle_pagination_input(KeyCode::Period, &mut MenuPage(0), 0));
    }

    #[test]
    fn arrows_wrap_round_the_rows() {
        assert_eq!(step_selection(None, 3, true), Some(0));
        assert_eq!(step_selection(None, 3, false), Some(2));
        assert_eq!(step_selection(Some(2), 3, true), Some(0));
        assert_eq!(step_selection(Some(0), 3, false), Some(2));
        assert_eq!(step_selection(Some(5), 3, true), Some(0));
        assert_eq!(step_selection(Some(1), 0, true), None);
    }

    #[test]
    fn the_row_hint_names_the_letters_in_use() {
        assert_eq!(Hint::rows(1, "Use").cap, "a");
        assert_eq!(Hint::rows(4, "Use").cap, "a-d");
        assert_eq!(Hint::rows(30, "Use").cap, "a-j");
    }
}
