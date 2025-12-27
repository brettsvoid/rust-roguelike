use bevy::input::keyboard::KeyboardInput;
use bevy::input::ButtonState;
use bevy::prelude::*;

use crate::resources::UiFont;
use crate::RunState;

use super::resources::{MenuPage, ITEMS_PER_PAGE};

/// Marker component for modal menu container
#[derive(Component)]
pub struct ModalMenuContainer;

/// Marker component for the text element that can be updated for pagination
#[derive(Component)]
pub struct ModalMenuText;

/// Configuration for a modal menu
#[derive(Component, Clone)]
pub struct ModalMenu {
    pub title: String,
    pub items: Vec<MenuItem>,
    pub footer: Option<String>,
    pub empty_message: Option<String>,
    pub show_pagination: bool,
    pub cancel_state: Option<RunState>,
    pub any_key_state: Option<RunState>,
    pub style: MenuStyle,
}

/// A single item in a modal menu
#[derive(Clone)]
pub struct MenuItem {
    pub label: String,
    pub key: Option<char>,
    pub action: MenuAction,
}

/// Actions that can be triggered by menu items
#[derive(Clone)]
pub enum MenuAction {
    /// Transition to a different RunState
    StateTransition(RunState),
    /// Select an item by index (for inventory-style menus)
    SelectIndex(usize),
    /// No action (display only)
    None,
}

/// Visual style for the menu
#[derive(Clone)]
pub struct MenuStyle {
    pub background_color: Color,
    pub border_color: Color,
    pub text_color: Color,
    pub title_color: Option<Color>,
    pub padding: f32,
    pub font_size: f32,
    pub background_image: Option<Handle<Image>>,
}

impl Default for MenuStyle {
    fn default() -> Self {
        Self {
            background_color: Color::srgba(0.1, 0.1, 0.1, 0.9),
            border_color: Color::WHITE,
            text_color: Color::WHITE,
            title_color: None,
            padding: 20.0,
            font_size: 16.0,
            background_image: None,
        }
    }
}

/// Builder for creating modal menus
pub struct ModalMenuBuilder {
    menu: ModalMenu,
}

impl ModalMenuBuilder {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            menu: ModalMenu {
                title: title.into(),
                items: Vec::new(),
                footer: None,
                empty_message: None,
                show_pagination: false,
                cancel_state: None,
                any_key_state: None,
                style: MenuStyle::default(),
            },
        }
    }

    /// Add a single menu item
    pub fn item(mut self, label: impl Into<String>, key: char, action: MenuAction) -> Self {
        self.menu.items.push(MenuItem {
            label: label.into(),
            key: Some(key),
            action,
        });
        self
    }

    /// Add multiple items from an iterator
    pub fn items<I, S>(mut self, items: I) -> Self
    where
        I: IntoIterator<Item = (S, MenuAction)>,
        S: Into<String>,
    {
        for (i, (label, action)) in items.into_iter().enumerate() {
            let key = (b'a' + i as u8) as char;
            self.menu.items.push(MenuItem {
                label: label.into(),
                key: Some(key),
                action,
            });
        }
        self
    }

    /// Add items with automatic letter keys (a, b, c, ...)
    pub fn items_with_index<I, S>(mut self, items: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        for (i, label) in items.into_iter().enumerate() {
            let key = (b'a' + i as u8) as char;
            self.menu.items.push(MenuItem {
                label: label.into(),
                key: Some(key),
                action: MenuAction::SelectIndex(i),
            });
        }
        self
    }

    /// Enable pagination for long item lists
    pub fn paginated(mut self) -> Self {
        self.menu.show_pagination = true;
        self
    }

    /// Set footer text
    pub fn footer(mut self, text: impl Into<String>) -> Self {
        self.menu.footer = Some(text.into());
        self
    }

    /// Set message when items list is empty
    pub fn empty_message(mut self, text: impl Into<String>) -> Self {
        self.menu.empty_message = Some(text.into());
        self
    }

    /// Set state to transition to when Escape is pressed
    pub fn on_cancel(mut self, state: RunState) -> Self {
        self.menu.cancel_state = Some(state);
        self
    }

    /// Set state to transition to on any key press
    pub fn on_any_key(mut self, state: RunState) -> Self {
        self.menu.any_key_state = Some(state);
        self
    }

    /// Set custom style
    pub fn style(mut self, style: MenuStyle) -> Self {
        self.menu.style = style;
        self
    }

    /// Set background color
    pub fn background_color(mut self, color: Color) -> Self {
        self.menu.style.background_color = color;
        self
    }

    /// Set border color
    pub fn border_color(mut self, color: Color) -> Self {
        self.menu.style.border_color = color;
        self
    }

    /// Set text color
    pub fn text_color(mut self, color: Color) -> Self {
        self.menu.style.text_color = color;
        self
    }

    /// Set background image
    pub fn background_image(mut self, image: Handle<Image>) -> Self {
        self.menu.style.background_image = Some(image);
        self
    }

    /// Set padding
    pub fn padding(mut self, padding: f32) -> Self {
        self.menu.style.padding = padding;
        self
    }

    /// Set font size
    pub fn font_size(mut self, size: f32) -> Self {
        self.menu.style.font_size = size;
        self
    }

    /// Build and spawn the menu
    pub fn spawn(self, commands: &mut Commands, font: &UiFont, menu_page: &MenuPage) -> Entity {
        spawn_modal_menu(commands, font, &self.menu, menu_page)
    }

    /// Get the built menu configuration
    pub fn build(self) -> ModalMenu {
        self.menu
    }
}

/// Build the text content for a modal menu
pub fn build_menu_text(menu: &ModalMenu, menu_page: &MenuPage) -> String {
    let mut text = menu.title.clone();
    text.push_str("\n\n");

    if menu.items.is_empty() {
        if let Some(ref empty_msg) = menu.empty_message {
            text.push_str(empty_msg);
        }
    } else {
        let total_items = menu.items.len();
        let total_pages = (total_items + ITEMS_PER_PAGE - 1) / ITEMS_PER_PAGE;
        let current_page = menu_page.0.min(total_pages.saturating_sub(1));
        let start_idx = current_page * ITEMS_PER_PAGE;

        // Add visible items
        for (display_idx, item) in menu.items.iter().enumerate().skip(start_idx).take(ITEMS_PER_PAGE) {
            if let Some(key) = item.key {
                // Recalculate key based on position within page
                let page_idx = display_idx - start_idx;
                let display_key = (b'a' + page_idx as u8) as char;
                text.push_str(&format!("({}) {}\n", display_key, item.label));
            } else {
                text.push_str(&format!("{}\n", item.label));
            }
        }

        // Add pagination info
        if menu.show_pagination && total_pages > 1 {
            text.push_str(&format!("\nPage {}/{} (</> to navigate)", current_page + 1, total_pages));
        }
    }

    // Add footer
    if let Some(ref footer) = menu.footer {
        text.push_str("\n\n");
        text.push_str(footer);
    }

    text
}

/// Spawn a modal menu entity
pub fn spawn_modal_menu(
    commands: &mut Commands,
    font: &UiFont,
    menu: &ModalMenu,
    menu_page: &MenuPage,
) -> Entity {
    let text_content = build_menu_text(menu, menu_page);
    let style = &menu.style;

    let container = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            ModalMenuContainer,
            menu.clone(),
        ));

    let container_id = container.id();

    // Add background image if present
    commands.entity(container_id).insert(
        if let Some(ref image) = style.background_image {
            ImageNode::new(image.clone())
        } else {
            ImageNode::default()
        }
    );

    commands.entity(container_id).with_children(|parent| {
        parent
            .spawn((
                Node {
                    padding: UiRect::all(Val::Px(style.padding)),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BorderColor(style.border_color),
                BackgroundColor(style.background_color),
            ))
            .with_children(|inner| {
                inner.spawn((
                    Text::new(text_content),
                    TextFont {
                        font: font.0.clone(),
                        font_size: style.font_size,
                        ..default()
                    },
                    TextColor(style.text_color),
                    ModalMenuText,
                ));
            });
    });

    container_id
}

/// Despawn all modal menu entities
pub fn despawn_modal_menu(
    commands: &mut Commands,
    menu_query: &Query<Entity, With<ModalMenuContainer>>,
) {
    for entity in menu_query.iter() {
        commands.entity(entity).despawn_recursive();
    }
}

/// Get the selected item index based on key press and current page
pub fn get_selected_index(key_code: KeyCode, menu_page: &MenuPage, total_items: usize) -> Option<usize> {
    let key_offset = match key_code {
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
    }?;

    let start_idx = menu_page.0 * ITEMS_PER_PAGE;
    let index = start_idx + key_offset;

    if index < total_items {
        Some(index)
    } else {
        None
    }
}

/// Handle pagination input, returns true if page changed
pub fn handle_pagination_input(
    key_code: KeyCode,
    menu_page: &mut MenuPage,
    total_items: usize,
) -> bool {
    let total_pages = if total_items == 0 { 1 } else { (total_items + ITEMS_PER_PAGE - 1) / ITEMS_PER_PAGE };

    match key_code {
        KeyCode::Comma => {
            if menu_page.0 > 0 {
                menu_page.0 -= 1;
                return true;
            }
        }
        KeyCode::Period => {
            if menu_page.0 < total_pages.saturating_sub(1) {
                menu_page.0 += 1;
                return true;
            }
        }
        _ => {}
    }
    false
}

/// System that handles common menu input patterns
pub fn handle_modal_menu_input(
    mut evr_kbd: EventReader<KeyboardInput>,
    mut next_state: ResMut<NextState<RunState>>,
    menu_query: Query<&ModalMenu, With<ModalMenuContainer>>,
) {
    let Ok(menu) = menu_query.get_single() else {
        return;
    };

    for ev in evr_kbd.read() {
        if ev.state != ButtonState::Pressed {
            continue;
        }

        // Handle any-key transition
        if let Some(state) = menu.any_key_state {
            next_state.set(state);
            return;
        }

        // Handle escape/cancel
        if ev.key_code == KeyCode::Escape {
            if let Some(state) = menu.cancel_state {
                next_state.set(state);
                return;
            }
        }
    }
}
