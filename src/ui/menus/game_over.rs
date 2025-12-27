use bevy::input::keyboard::KeyboardInput;
use bevy::input::ButtonState;
use bevy::prelude::*;

use crate::resources::UiFont;
use crate::RunState;

use crate::ui::menu::{ModalMenuBuilder, ModalMenuContainer, MenuStyle};
use crate::ui::resources::MenuPage;

pub struct GameOverPlugin;

impl Plugin for GameOverPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(RunState::GameOver), spawn_game_over_menu)
            .add_systems(OnExit(RunState::GameOver), despawn_game_over_menu)
            .add_systems(
                Update,
                handle_game_over_input.run_if(in_state(RunState::GameOver)),
            );
    }
}

fn spawn_game_over_menu(mut commands: Commands, font: Res<UiFont>, menu_page: Res<MenuPage>) {
    ModalMenuBuilder::new("GAME OVER")
        .empty_message("You have died.")
        .footer("(Press any key to return to menu)")
        .on_any_key(RunState::MainMenu)
        .style(MenuStyle {
            background_color: Color::srgba(0.1, 0.0, 0.0, 0.9),
            border_color: Color::srgb(0.8, 0.0, 0.0),
            text_color: Color::srgb(1.0, 0.3, 0.3),
            title_color: None,
            padding: 30.0,
            font_size: 20.0,
            background_image: None,
        })
        .spawn(&mut commands, &font, &menu_page);
}

fn despawn_game_over_menu(
    mut commands: Commands,
    menu_query: Query<Entity, With<ModalMenuContainer>>,
) {
    for entity in &menu_query {
        commands.entity(entity).despawn_recursive();
    }
}

fn handle_game_over_input(
    mut evr_kbd: EventReader<KeyboardInput>,
    mut next_state: ResMut<NextState<RunState>>,
) {
    for ev in evr_kbd.read() {
        if ev.state == ButtonState::Pressed {
            next_state.set(RunState::MainMenu);
            return;
        }
    }
}
