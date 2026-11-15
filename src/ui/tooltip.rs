use bevy::prelude::*;

use crate::combat::{aim_at, Arms, CombatStats};
use crate::components::Name;
use crate::game_flow::Mode;
use crate::map::{Map, Position};
use crate::monsters::Monster;
use crate::party::ActiveUnit;
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

/// Names what is under the cursor. In combat a hostile the active hero can
/// attack also shows the chance to hit it.
#[allow(clippy::too_many_arguments)]
fn update_tooltip(
    mut commands: Commands,
    window: Query<&Window>,
    hovered: Res<HoveredTile>,
    map: Res<Map>,
    font: Res<UiFont>,
    (mode, active, arms): (Res<Mode>, Res<ActiveUnit>, Arms),
    entities_query: Query<(&Position, &Name, Option<&CombatStats>, Has<Monster>)>,
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

    let attacker = active
        .0
        .filter(|&unit| matches!(*mode, Mode::Combat { .. }) && mode.can_act(unit))
        .and_then(|unit| entities_query.get(unit).ok().and_then(|(pos, _, stats, _)| Some((unit, pos, stats?))));
    let chance = |target: &Position, defence: i32| {
        let (unit, from, stats) = attacker?;
        aim_at(&map, &arms.weapon(unit), stats.aim, (from.x, from.y), defence, (target.x, target.y)).ok()
    };

    // Find entities at this position (using world coordinates)
    let mut tooltip_names: Vec<String> = Vec::new();
    for (pos, name, stats, monster) in &entities_query {
        if pos.x == map_x && pos.y == map_y {
            match stats.filter(|_| monster).and_then(|s| chance(pos, s.defence)) {
                Some(chance) => tooltip_names.push(format!("{} ({chance}% to hit)", name.name)),
                None => tooltip_names.push(name.name.clone()),
            }
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
