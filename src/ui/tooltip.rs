use bevy::prelude::*;

use crate::components::Name;
use crate::map::{Map, Position};
use crate::picking::{update_hovered_tile, HoveredTile};
use crate::resources::UiFont;
use crate::RunState;

use super::components::Tooltip;

pub struct TooltipPlugin;

impl Plugin for TooltipPlugin {
    fn build(&self, app: &mut App) {
        let in_gameplay = not(in_state(RunState::MainMenu))
            .and_then(not(in_state(RunState::GameOver)))
            .and_then(not(in_state(RunState::MapGeneration)))
            .and_then(not(in_state(RunState::MapBuilderSelect)));

        app.add_systems(Update, update_tooltip.after(update_hovered_tile).run_if(in_gameplay));
    }
}

fn update_tooltip(
    mut commands: Commands,
    window: Query<&Window>,
    hovered: Res<HoveredTile>,
    map: Res<Map>,
    font: Res<UiFont>,
    entities_query: Query<(&Position, &Name)>,
    tooltip_query: Query<Entity, With<Tooltip>>,
) {
    for entity in &tooltip_query {
        commands.entity(entity).despawn();
    }

    let Ok(window) = window.single() else {
        return;
    };
    let (Some(cursor_position), Some((map_x, map_y))) = (hovered.cursor, hovered.tile) else {
        return;
    };

    // Check if tile is visible
    let idx = map.xy_idx(map_x, map_y);
    if !map.visible_tiles[idx] {
        return;
    }

    // Find entities at this position (using world coordinates)
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
            font: font.0.clone().into(),
            font_size: FontSize::Px(14.0),
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
