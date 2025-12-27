use bevy::prelude::*;

use crate::components::Name;
use crate::map::{Map, Position, GRID_PX, MAP_HEIGHT, MAP_WIDTH};
use crate::resources::UiFont;
use crate::RunState;

use super::components::{CursorHighlight, Tooltip};

pub struct TooltipPlugin;

impl Plugin for TooltipPlugin {
    fn build(&self, app: &mut App) {
        let in_gameplay = not(in_state(RunState::MainMenu))
            .and(not(in_state(RunState::GameOver)))
            .and(not(in_state(RunState::MapGeneration)))
            .and(not(in_state(RunState::MapBuilderSelect)));

        app.add_systems(Update, update_tooltip.run_if(in_gameplay));
    }
}

fn update_tooltip(
    mut commands: Commands,
    window: Query<&Window>,
    camera_query: Query<(&Camera, &GlobalTransform)>,
    map: Res<Map>,
    font: Res<UiFont>,
    entities_query: Query<(&Position, &Name)>,
    tooltip_query: Query<Entity, With<Tooltip>>,
    highlight_query: Query<Entity, With<CursorHighlight>>,
) {
    // Remove existing tooltip and highlight
    for entity in &tooltip_query {
        commands.entity(entity).despawn();
    }
    for entity in &highlight_query {
        commands.entity(entity).despawn();
    }

    let Ok(window) = window.get_single() else {
        return;
    };

    let Some(cursor_position) = window.cursor_position() else {
        return;
    };

    // Convert screen position to world position
    let Ok((camera, camera_transform)) = camera_query.get_single() else {
        return;
    };

    let Ok(world_pos) = camera.viewport_to_world_2d(camera_transform, cursor_position) else {
        return;
    };

    // Convert world position to map coordinates
    let half_width = window.width() / 2.0;
    let half_height = window.height() / 2.0;

    let map_x = ((world_pos.x + half_width) / GRID_PX.x).floor() as i32;
    let map_y = ((-world_pos.y + half_height) / GRID_PX.y).floor() as i32;

    // Check bounds
    if map_x < 0 || map_x >= MAP_WIDTH as i32 || map_y < 0 || map_y >= MAP_HEIGHT as i32 {
        return;
    }

    // Spawn cursor highlight (magenta background on the tile)
    commands.spawn((
        Sprite {
            color: Color::srgba(1.0, 0.0, 1.0, 0.3), // Magenta with transparency
            custom_size: Some(Vec2::new(GRID_PX.x, GRID_PX.y)),
            ..default()
        },
        Transform::from_xyz(
            (map_x as f32) * GRID_PX.x + (GRID_PX.x / 2.0) - half_width,
            (map_y as f32) * -GRID_PX.y - (GRID_PX.y / 2.0) + half_height,
            0.5, // Between tiles and entities
        ),
        CursorHighlight,
    ));

    // Check if tile is visible
    let idx = map.xy_idx(map_x, map_y);
    if !map.visible_tiles[idx] {
        return;
    }

    // Find entities at this position
    let mut tooltip_names: Vec<String> = Vec::new();
    for (pos, name) in &entities_query {
        if pos.x == map_x && pos.y == map_y {
            tooltip_names.push(name.name.clone());
        }
    }

    if tooltip_names.is_empty() {
        return;
    }

    // Spawn tooltip
    let tooltip_text = tooltip_names.join(", ");
    let on_right_side = cursor_position.x > window.width() / 2.0;

    commands.spawn((
        Text::new(if on_right_side {
            format!("{} <-", tooltip_text)
        } else {
            format!("-> {}", tooltip_text)
        }),
        TextFont {
            font: font.0.clone(),
            font_size: 14.0,
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            left: if on_right_side {
                Val::Px(cursor_position.x - 150.0)
            } else {
                Val::Px(cursor_position.x + 15.0)
            },
            top: Val::Px(cursor_position.y),
            ..default()
        },
        BackgroundColor(Color::srgb(0.3, 0.3, 0.3)),
        Tooltip,
    ));
}
