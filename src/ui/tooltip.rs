use bevy::prelude::*;

use crate::combat::{Arms, CombatStats, Refusal};
use crate::components::{HungerClock, HungerState, Name};
use crate::cover::Exposure;
use crate::game_flow::Mode;
use crate::map::{Map, Position};
use crate::monsters::Monster;
use crate::overwatch::Hunkered;
use crate::party::ActiveUnit;
use crate::picking::{update_hovered_tile, HoveredTile};
use crate::resources::UiFont;
use crate::RunState;

use super::combat_hud::{shot_preview, Mark, Shooter, ShotPreview};
use super::components::Tooltip;
use super::style::{self, font};

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

/// The panel's width, and how far it sits from the cursor, in logical pixels.
const WIDTH: f32 = 196.0;
const OFFSET: f32 = 18.0;
/// Room kept for the tallest panel (the shot preview) above the bottom bar.
const TALLEST: f32 = 104.0;

/// What one thing under the cursor shows.
enum Line {
    Name(String),
    /// A hostile the active hero could attack: the shot preview, or why not.
    Shot(String, Result<ShotPreview, Refusal>),
}

/// Names what is under the cursor. In combat a hostile the active hero can
/// act against shows the shot preview instead: hit and crit chance, damage
/// and its cover or a flank, from the numbers the attack rolls against.
#[allow(clippy::too_many_arguments)]
fn update_tooltip(
    mut commands: Commands,
    camera: Single<&Camera, With<Camera3d>>,
    hovered: Res<HoveredTile>,
    map: Res<Map>,
    font_handle: Res<UiFont>,
    (mode, active, arms): (Res<Mode>, Res<ActiveUnit>, Arms),
    entities_query: Query<(Entity, &Position, &Name, Option<&CombatStats>, Has<Monster>, Has<Hunkered>)>,
    hunger: Query<&HungerClock>,
    tooltip_query: Query<Entity, With<Tooltip>>,
) {
    for entity in &tooltip_query {
        commands.entity(entity).despawn();
    }

    let (Some(cursor), Some((map_x, map_y))) = (hovered.cursor, hovered.tile) else {
        return;
    };
    if !map.visible_tiles[map.xy_idx(map_x, map_y)] {
        return;
    }

    let shooter = active
        .0
        .filter(|&unit| matches!(*mode, Mode::Combat { .. }) && mode.can_act(unit))
        .and_then(|unit| {
            let (_, at, _, stats, ..) = entities_query.get(unit).ok()?;
            let stats = stats?;
            let well_fed = hunger.get(unit).is_ok_and(|h| h.state == HungerState::WellFed);
            Some((unit, Shooter { at: (at.x, at.y), aim: stats.aim, bonus: stats.power + i32::from(well_fed) }))
        });

    let lines: Vec<Line> = entities_query
        .iter()
        .filter(|(_, pos, ..)| pos.x == map_x && pos.y == map_y)
        .map(|(entity, pos, name, stats, monster, hunkered)| match (shooter, stats.filter(|_| monster)) {
            (Some((unit, shooter)), Some(stats)) => {
                let mark = Mark {
                    at: (pos.x, pos.y),
                    defence: stats.defence,
                    armour: stats.armour + arms.armour(entity),
                    hunkered,
                };
                Line::Shot(name.name.clone(), shot_preview(&map, &arms.weapon(unit), shooter, mark))
            }
            _ => Line::Name(name.name.clone()),
        })
        .collect();
    if lines.is_empty() {
        return;
    }

    // Beside the cursor, flipped left near the right edge, kept above the bar.
    let size = camera.logical_target_size().unwrap_or(Vec2::new(1600.0, 1000.0));
    let left = if cursor.x + OFFSET + WIDTH > size.x { cursor.x - OFFSET - WIDTH } else { cursor.x + OFFSET };
    let top = (cursor.y + OFFSET).min(size.y - style::BAR_HEIGHT - TALLEST).max(0.0);
    let f = &font_handle.0;
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(left.round()),
                top: Val::Px(top.round()),
                width: Val::Px(WIDTH),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                padding: UiRect::axes(Val::Px(10.0), Val::Px(8.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(style::RADIUS)),
                ..default()
            },
            BackgroundColor(style::PANEL),
            BorderColor::all(style::PANEL_EDGE),
            Tooltip,
        ))
        .with_children(|panel| {
            for line in lines {
                match line {
                    Line::Name(name) => {
                        panel.spawn((Text::new(name), font(f, style::FONT_NAME), TextColor(style::TEXT)));
                    }
                    Line::Shot(name, preview) => spawn_shot(panel, f, name, preview),
                }
            }
        });
}

fn spawn_shot(panel: &mut ChildSpawnerCommands, f: &Handle<Font>, name: String, preview: Result<ShotPreview, Refusal>) {
    panel.spawn((Text::new(name), font(f, style::FONT_NAME), TextColor(style::HOSTILE)));
    let preview = match preview {
        Ok(preview) => preview,
        Err(refusal) => {
            let why = match refusal {
                Refusal::OutOfRange => "Out of range",
                Refusal::NoLineOfSight => "No line of sight",
            };
            panel.spawn((Text::new(why), font(f, style::FONT_BODY), TextColor(style::TEXT_DIM)));
            return;
        }
    };
    panel
        .spawn(Node { align_items: AlignItems::Baseline, column_gap: Val::Px(6.0), ..default() })
        .with_children(|row| {
            row.spawn((Text::new(format!("{}%", preview.hit)), font(f, style::FONT_BIG), TextColor(style::ACCENT)));
            row.spawn((Text::new("to hit"), font(f, style::FONT_BODY), TextColor(style::TEXT_DIM)));
        });
    let (low, high) = preview.damage;
    panel.spawn((
        Text::new(format!("Crit {}%   Damage {low}-{high}", preview.crit)),
        font(f, style::FONT_BODY),
        TextColor(style::TEXT),
    ));
    if let Some(note) = preview.exposure.note() {
        let tint = match preview.exposure {
            Exposure::Flanked => style::HEALTH,
            _ => style::BADGE_HUNKERED,
        };
        let mut note = note.to_string();
        note[..1].make_ascii_uppercase();
        panel.spawn((Text::new(note), font(f, style::FONT_BODY), TextColor(tint)));
    }
}
