//! A small health bar over every monster the party sees, and over a hero who
//! is hurt or under the cursor (the party cards always show the heroes'
//! health, so full-health heroes standing close stay uncluttered).
//! Each bar is a UI node placed by projecting a point just above the unit's
//! model through the `Camera3d` after transforms propagate and before UI
//! layout, so it follows a unit gliding between tiles in the frame it is
//! drawn. The height comes from the model's own bounds once it has loaded.

use std::collections::HashMap;

use bevy::prelude::*;
use bevy::transform::TransformSystems;
use bevy::ui::UiSystems;

use bevy::camera::primitives::Aabb;

use crate::combat::CombatStats;
use crate::map::Position;
use crate::monsters::Monster;
use crate::party::PartyMember;
use crate::picking::HoveredTile;

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

/// How far above a unit's origin the bar floats until its model has loaded,
/// in world units (the stand-in capsule is 1.4 tall, its origin at 0.7).
const BAR_LIFT: f32 = 0.95;
/// The gap between the top of a model and its bar, in world units.
const BAR_GAP: f32 = 0.2;

#[derive(Component)]
struct HealthBar {
    unit: Entity,
    /// Height of the bar above the unit's origin, once its meshes are in.
    lift: Option<f32>,
}

/// The top of a set of meshes (local bounds and world transforms), above `origin`.
fn top_above(origin: Vec3, meshes: impl IntoIterator<Item = (Aabb, GlobalTransform)>) -> Option<f32> {
    meshes
        .into_iter()
        .flat_map(|(aabb, at)| {
            let (min, max) = (Vec3::from(aabb.min()), Vec3::from(aabb.max()));
            [min.x, max.x]
                .into_iter()
                .flat_map(move |x| [min.y, max.y].into_iter().flat_map(move |y| [min.z, max.z].map(|z| Vec3::new(x, y, z))))
                .map(move |corner| at.transform_point(corner).y)
        })
        .reduce(f32::max)
        .map(|top| top - origin.y)
}

/// Whether a unit's bar shows: a monster's always (while the unit does), a
/// hero's only when hurt or hovered.
fn bar_shown(hero: bool, hp: i32, max_hp: i32, hovered: bool) -> bool {
    !hero || hp < max_hp || hovered
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
                    HealthBar { unit, lift: None },
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

/// Puts each bar just over its unit's model and sizes its fill. A bar hides
/// with its unit (a monster off the party's view), when its unit is behind
/// the camera, and for a hero at full health away from the cursor.
fn place_health_bars(
    camera: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    hovered: Res<HoveredTile>,
    units: Query<(&GlobalTransform, &CombatStats, &Visibility, &Position, Has<PartyMember>)>,
    children: Query<&Children>,
    meshes: Query<(&Aabb, &GlobalTransform)>,
    mut bars: Query<(&mut HealthBar, &mut Node, &mut Visibility, &Children), Without<CombatStats>>,
    mut fills: Query<(&mut Node, &mut BackgroundColor), (With<HealthFill>, Without<HealthBar>)>,
) {
    let Ok((camera, eye)) = camera.single() else {
        return;
    };
    for (mut bar, mut node, mut visibility, bar_children) in &mut bars {
        let Ok((at, stats, unit_visibility, pos, hero)) = units.get(bar.unit) else {
            continue;
        };
        if bar.lift.is_none() {
            let parts = std::iter::once(bar.unit).chain(children.iter_descendants(bar.unit));
            let found = parts.filter_map(|e| meshes.get(e).ok()).map(|(aabb, at)| (*aabb, *at));
            bar.lift = top_above(at.translation(), found).map(|top| top + BAR_GAP);
        }
        let hovered = hovered.tile == Some((pos.x, pos.y));
        let shown = *unit_visibility != Visibility::Hidden && bar_shown(hero, stats.hp, stats.max_hp, hovered);
        let lift = bar.lift.unwrap_or(BAR_LIFT);
        let point = camera.world_to_viewport(eye, at.translation() + Vec3::Y * lift).ok().filter(|_| shown);
        let Some(point) = point else {
            *visibility = Visibility::Hidden;
            continue;
        };
        *visibility = Visibility::Inherited;
        node.left = Val::Px((point.x - style::UNIT_BAR_WIDTH / 2.0).round());
        node.top = Val::Px((point.y - style::UNIT_BAR_HEIGHT).round());
        let fraction = stats.hp.max(0) as f32 / stats.max_hp.max(1) as f32;
        for child in bar_children {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bar_height_is_the_top_of_the_meshes_above_the_unit() {
        let origin = Vec3::new(3.0, 0.7, 2.0);
        let body = (Aabb::from_min_max(Vec3::new(-0.3, -0.7, -0.3), Vec3::new(0.3, 0.7, 0.3)), GlobalTransform::from_translation(origin));
        // A horn on a child, lifted and scaled.
        let horn = (
            Aabb::from_min_max(Vec3::ZERO, Vec3::new(0.1, 0.5, 0.1)),
            GlobalTransform::from(Transform::from_xyz(3.0, 1.6, 2.0).with_scale(Vec3::splat(2.0))),
        );
        assert_eq!(top_above(origin, [body]), Some(0.7));
        assert!((top_above(origin, [body, horn]).unwrap() - 1.9).abs() < 1e-5);
        assert_eq!(top_above(origin, []), None);
    }

    #[test]
    fn hero_bars_show_only_when_hurt_or_hovered() {
        assert!(bar_shown(false, 10, 10, false));
        assert!(!bar_shown(true, 10, 10, false));
        assert!(bar_shown(true, 9, 10, false));
        assert!(bar_shown(true, 10, 10, true));
    }
}
