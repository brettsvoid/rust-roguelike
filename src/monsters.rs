use bevy::{color::palettes, prelude::*};
use rand::Rng;

use crate::{
    map::{Map, Position, FONT_SIZE},
    player::Player,
    resources::UiFont,
    viewshed::Viewshed,
    RunState,
};

#[derive(Component, Debug)]
struct Monster;

pub struct MonstersPlugin;
impl Plugin for MonstersPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_monsters).add_systems(
            Update,
            (
                update_monsters,
                monster_ai.run_if(in_state(RunState::Running)),
            ),
        );
    }
}

fn setup_monsters(mut commands: Commands, font: Res<UiFont>, map: Res<Map>) {
    let text_font = TextFont {
        font: font.0.clone(),
        font_size: FONT_SIZE,
        ..default()
    };

    let mut rng = rand::thread_rng();
    let roll = rng.gen_range(0..=1);
    let monster_type = match roll {
        0 => "o",
        _ => "g",
    };

    // Skip the first room because that's where the player starts
    for room in map.rooms.iter().skip(1) {
        let (x, y) = room.center();
        commands.spawn((
            Monster,
            Position { x, y, z: 1 },
            Text2d::new(monster_type),
            text_font.clone(),
            Viewshed {
                range: 8,
                ..default()
            },
            TextColor(palettes::basic::RED.into()),
            BackgroundColor(palettes::basic::BLACK.into()),
            Visibility::Hidden,
        ));
    }
}

fn update_monsters(
    map: Res<Map>,
    mut monster_query: Query<(&Position, &mut Visibility), With<Monster>>,
) {
    for (pos, mut visibility) in &mut monster_query {
        let idx = map.xy_idx(pos.x, pos.y);
        if map.visible_tiles[idx] && matches!(*visibility, Visibility::Hidden) {
            visibility.toggle_visible_hidden();
        } else if matches!(*visibility, Visibility::Visible) {
            visibility.toggle_visible_hidden();
        }
    }
}

fn monster_ai(
    mut monster_query: Query<(&Position, &mut Viewshed), With<Monster>>,
    player_position: Single<&Position, With<Player>>,
) {
    let player_pos = &player_position;

    for (pos, viewshed) in &monster_query {
        if viewshed
            .visible_tiles
            .contains(&(player_pos.x, player_pos.y))
        {
            println!("Monster shouts insults");
        }
    }
}
