//! A small health bar over every hero and every monster the party sees.
//! Each bar is a UI node placed by projecting the unit's rendered
//! `GlobalTransform` through the `Camera3d` after transforms propagate and
//! before UI layout, so it follows a unit gliding between tiles in the frame
//! it is drawn.

use std::collections::HashMap;

use bevy::prelude::*;
use bevy::transform::TransformSystems;
use bevy::ui::UiSystems;

use crate::combat::CombatStats;
use crate::monsters::Monster;
use crate::party::PartyMember;

use super::style;

pub struct HealthBarsPlugin;

impl Plugin for HealthBarsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, sync_health_bars).add_systems(
            PostUpdate,
            place_health_bars.after(TransformSystems::Propagate).before(UiSystems::Layout),
        );
    }
}

/// How far above a unit's origin the bar floats, in world units (a creature
/// is about 1.4 tall and its origin sits at 0.7).
const BAR_LIFT: f32 = 0.95;

#[derive(Component)]
struct HealthBar {
    unit: Entity,
}

#[derive(Component)]
struct HealthFill;

/// One bar per unit: spawned for new units, despawned with theirs.
fn sync_health_bars(
    mut commands: Commands,
    units: Query<(Entity, Has<PartyMember>), (With<CombatStats>, Or<(With<Monster>, With<PartyMember>)>)>,
    mut bars: Local<HashMap<Entity, Entity>>,
) {
    bars.retain(|unit, bar| {
        let alive = units.contains(*unit);
        if !alive {
            commands.entity(*bar).despawn();
        }
        alive
    });
    for (unit, hero) in &units {
        bars.entry(unit).or_insert_with(|| {
            commands
                .spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        width: Val::Px(style::UNIT_BAR_WIDTH),
                        height: Val::Px(style::UNIT_BAR_HEIGHT),
                        border: UiRect::all(Val::Px(1.0)),
                        border_radius: BorderRadius::all(Val::Px(2.0)),
                        overflow: Overflow::clip(),
                        ..default()
                    },
                    BackgroundColor(style::TRACK),
                    BorderColor::all(Color::srgba(0.0, 0.0, 0.0, 0.8)),
                    // Under every other panel: the HUD, menus and the banner.
                    GlobalZIndex(-1),
                    Visibility::Hidden,
                    HealthBar { unit },
                ))
                .with_child((
                    Node { width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
                    BackgroundColor(if hero { style::HEALTH } else { style::HOSTILE }),
                    HealthFill,
                ))
                .id()
        });
    }
}

/// Puts each bar over its unit and sizes its fill. A bar hides with its unit
/// (a monster off the party's view) and when its unit is behind the camera.
fn place_health_bars(
    camera: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    units: Query<(&GlobalTransform, &CombatStats, &Visibility, Has<PartyMember>)>,
    mut bars: Query<(&HealthBar, &mut Node, &mut Visibility, &Children), Without<CombatStats>>,
    mut fills: Query<(&mut Node, &mut BackgroundColor), (With<HealthFill>, Without<HealthBar>)>,
) {
    let Ok((camera, eye)) = camera.single() else {
        return;
    };
    for (bar, mut node, mut visibility, children) in &mut bars {
        let Ok((at, stats, unit_visibility, hero)) = units.get(bar.unit) else {
            continue;
        };
        let shown = *unit_visibility != Visibility::Hidden;
        let point = camera.world_to_viewport(eye, at.translation() + Vec3::Y * BAR_LIFT).ok().filter(|_| shown);
        let Some(point) = point else {
            *visibility = Visibility::Hidden;
            continue;
        };
        *visibility = Visibility::Inherited;
        node.left = Val::Px((point.x - style::UNIT_BAR_WIDTH / 2.0).round());
        node.top = Val::Px((point.y - style::UNIT_BAR_HEIGHT / 2.0).round());
        let fraction = stats.hp.max(0) as f32 / stats.max_hp.max(1) as f32;
        for child in children {
            if let Ok((mut fill, mut colour)) = fills.get_mut(*child) {
                fill.width = Val::Percent(100.0 * fraction);
                colour.0 = match (hero, fraction <= 0.34) {
                    (true, true) => style::HEALTH_LOW,
                    (true, false) => style::HEALTH,
                    (false, _) => style::HOSTILE,
                };
            }
        }
    }
}
