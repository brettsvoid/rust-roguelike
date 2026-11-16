//! Mouse picking: the cursor becomes a grid cell by casting a ray from the
//! `Camera3d` onto the floor plane (`y = 0`). `update_hovered_tile` writes the
//! result to `HoveredTile`, which every mouse consumer reads (tooltips,
//! targeting, the debug tile info). `TileMarks` draws flat overlays on tiles
//! and upright cover shields on their sides.

use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform};
use bevy::window::PrimaryWindow;

use crate::cover::Cover;
use crate::map::{is_stairs, Map};
use crate::RunState;

pub struct PickingPlugin;

impl Plugin for PickingPlugin {
    fn build(&self, app: &mut App) {
        let in_gameplay = not(in_state(RunState::MainMenu))
            .and_then(not(in_state(RunState::GameOver)))
            .and_then(not(in_state(RunState::MapGeneration)))
            .and_then(not(in_state(RunState::MapBuilderSelect)));

        app.init_resource::<HoveredTile>()
            .add_systems(Startup, setup_tile_marks)
            .add_systems(
                Update,
                (
                    update_hovered_tile.run_if(in_gameplay.clone()),
                    clear_hovered_tile.run_if(not(in_gameplay)),
                    show_hover_highlight,
                )
                    .chain()
                    .after(crate::camera::follow_iso_camera),
            );
    }
}

/// What the mouse points at this frame. `tile` is `None` when the cursor is
/// off the window, over a `BlocksPicking` panel (the HUD), off the map, or
/// outside gameplay. `cursor` is in logical window pixels, for placing popups.
#[derive(Resource, Default, Clone, Copy, PartialEq, Debug)]
pub struct HoveredTile {
    pub cursor: Option<Vec2>,
    pub tile: Option<(i32, i32)>,
}

/// UI panels that hide the world underneath: the cursor over them picks nothing.
#[derive(Component)]
pub struct BlocksPicking;

/// Stands in for the real cursor (screenshot harness `--cursor`).
#[derive(Resource)]
pub struct FakeCursor(pub Vec2);

/// The grid cell a ray hits on the floor plane, or `None` if it never does.
/// Tile centres sit on integer coordinates, so the hit rounds to the nearest.
pub fn ray_to_tile(ray: Ray3d) -> Option<(i32, i32)> {
    let hit = ray.plane_intersection_point(Vec3::ZERO, InfinitePlane3d::new(Vec3::Y))?;
    Some(((hit.x + 0.5).floor() as i32, (hit.z + 0.5).floor() as i32))
}

pub fn update_hovered_tile(
    window: Single<&Window, With<PrimaryWindow>>,
    fake_cursor: Option<Res<FakeCursor>>,
    camera: Single<(&Camera, &Transform), With<Camera3d>>,
    blockers: Query<(&ComputedNode, &UiGlobalTransform, &InheritedVisibility), With<BlocksPicking>>,
    map: Res<Map>,
    mut hovered: ResMut<HoveredTile>,
) {
    let cursor = fake_cursor.map(|fake| fake.0).or_else(|| window.cursor_position());
    // UI nodes measure in physical pixels.
    let scale = camera.0.target_scaling_factor().unwrap_or(1.0);
    let over_ui = cursor.is_some_and(|c| {
        let point = c * scale;
        blockers
            .iter()
            .any(|(node, transform, visible)| visible.get() && node.contains_point(*transform, point))
    });
    // The rig has no parent, so its `Transform` is already this frame's global
    // transform; `GlobalTransform` would lag a frame behind the follow system.
    let (camera, transform) = *camera;
    let tile = cursor
        .filter(|_| !over_ui)
        .and_then(|c| camera.viewport_to_world(&GlobalTransform::from(*transform), c).ok())
        .and_then(ray_to_tile)
        .filter(|&(x, y)| x >= 0 && x < map.width && y >= 0 && y < map.height);
    hovered.set_if_neq(HoveredTile { cursor, tile });
}

fn clear_hovered_tile(mut hovered: ResMut<HoveredTile>) {
    hovered.set_if_neq(HoveredTile::default());
}

/// The whole tile, in tile-local coordinates (x along grid x, y along grid y).
pub const FULL_TILE: Rect = Rect {
    min: Vec2::splat(-0.5),
    max: Vec2::splat(0.5),
};

/// A strip `thickness` wide along the side of a tile that faces the
/// neighbour at `(dx, dy)` (one of the four cardinal steps).
pub fn tile_edge(dx: i32, dy: i32, thickness: f32) -> Rect {
    let centre = Vec2::new(dx as f32, dy as f32) * (0.5 - thickness / 2.0);
    let half = if dx != 0 {
        Vec2::new(thickness / 2.0, 0.5)
    } else {
        Vec2::new(0.5, thickness / 2.0)
    };
    Rect::from_center_half_size(centre, half)
}

/// Unlit, translucent flat quads laid on tiles, so they read in the dark.
#[derive(Resource)]
pub struct TileMarks {
    quad: Handle<Mesh>,
    pub hover: Handle<StandardMaterial>,
    pub target: Handle<StandardMaterial>,
    pub area: Handle<StandardMaterial>,
    pub range: Handle<StandardMaterial>,
    pub path: Handle<StandardMaterial>,
    /// The combat move range: tiles one AP away, and those only a dash reaches.
    pub move_one: Handle<StandardMaterial>,
    pub move_two: Handle<StandardMaterial>,
    /// Cover shields: half cover low and amber, full cover tall and blue.
    pub shield_half: Handle<StandardMaterial>,
    pub shield_full: Handle<StandardMaterial>,
}

impl TileMarks {
    /// A mark covering `area` of tile `(x, y)`. `lift` stacks marks that can
    /// overlap (range under target under hover) so their draw order is fixed.
    pub fn mark(
        &self,
        material: &Handle<StandardMaterial>,
        map: &Map,
        (x, y): (i32, i32),
        area: Rect,
        lift: f32,
    ) -> impl Bundle {
        (
            Mesh3d(self.quad.clone()),
            MeshMaterial3d(material.clone()),
            mark_transform(map, (x, y), area, lift),
        )
    }
}

impl TileMarks {
    /// An upright shield on the `side` (a cardinal step) of tile `(x, y)`,
    /// half as tall for half cover as for full.
    pub fn shield(&self, map: &Map, (x, y): (i32, i32), side: (i32, i32), cover: Cover) -> impl Bundle {
        let (material, height) = match cover {
            Cover::Full => (&self.shield_full, 0.9),
            _ => (&self.shield_half, 0.45),
        };
        let floor = mark_transform(map, (x, y), FULL_TILE, 0.0).translation.y;
        // The quad lies in x-z; stand it up to face along the side, turning it for east and west.
        let turn = if side.0 != 0 { std::f32::consts::FRAC_PI_2 } else { 0.0 };
        let transform = Transform::from_xyz(x as f32 + side.0 as f32 * 0.42, floor + height / 2.0, y as f32 + side.1 as f32 * 0.42)
            .with_rotation(Quat::from_rotation_y(turn) * Quat::from_rotation_x(std::f32::consts::FRAC_PI_2))
            .with_scale(Vec3::new(0.8, 1.0, height));
        (Mesh3d(self.quad.clone()), MeshMaterial3d(material.clone()), transform)
    }
}

fn mark_transform(map: &Map, (x, y): (i32, i32), area: Rect, lift: f32) -> Transform {
    // Sit on top of the stairs block (0.2 tall, see `map_render`), else on the floor.
    let floor = if is_stairs(map.tiles[map.xy_idx(x, y)]) { 0.2 } else { 0.0 };
    let centre = area.center();
    Transform::from_xyz(x as f32 + centre.x, floor + 0.01 + lift, y as f32 + centre.y)
        .with_scale(Vec3::new(area.width(), 1.0, area.height()))
}

fn translucent(materials: &mut Assets<StandardMaterial>, color: Color) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color: color,
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        ..default()
    })
}

/// Seen from both faces, so a shield shows whichever side the camera is on.
fn double_sided(materials: &mut Assets<StandardMaterial>, color: Color) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color: color,
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        double_sided: true,
        cull_mode: None,
        ..default()
    })
}

#[derive(Component)]
struct HoverHighlight;

fn setup_tile_marks(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let marks = TileMarks {
        quad: meshes.add(Plane3d::default().mesh().size(1.0, 1.0)),
        hover: translucent(&mut materials, Color::srgba(1.0, 0.0, 1.0, 0.35)),
        target: translucent(&mut materials, Color::srgba(0.0, 1.0, 1.0, 0.6)),
        area: translucent(&mut materials, Color::srgba(1.0, 0.5, 0.0, 0.4)),
        range: translucent(&mut materials, Color::srgba(0.7, 0.7, 0.7, 0.7)),
        path: translucent(&mut materials, Color::srgba(1.0, 0.85, 0.3, 0.35)),
        move_one: translucent(&mut materials, Color::srgba(0.2, 0.55, 1.0, 0.4)),
        move_two: translucent(&mut materials, Color::srgba(0.1, 1.0, 0.45, 0.25)),
        shield_half: double_sided(&mut materials, Color::srgba(1.0, 0.75, 0.1, 0.8)),
        shield_full: double_sided(&mut materials, Color::srgba(0.3, 0.7, 1.0, 0.8)),
    };
    commands.spawn((
        HoverHighlight,
        Mesh3d(marks.quad.clone()),
        MeshMaterial3d(marks.hover.clone()),
        Transform::default(),
        Visibility::Hidden,
    ));
    commands.insert_resource(marks);
}

/// Only on tiles the player has seen, so the highlight cannot trace out
/// unexplored rooms.
fn show_hover_highlight(
    hovered: Res<HoveredTile>,
    map: Res<Map>,
    mut highlight: Single<(&mut Transform, &mut Visibility), With<HoverHighlight>>,
) {
    let (transform, visibility) = &mut *highlight;
    match hovered.tile.filter(|&(x, y)| map.revealed_tiles[map.xy_idx(x, y)]) {
        Some(tile) => {
            **transform = mark_transform(&map, tile, FULL_TILE, 0.02);
            **visibility = Visibility::Visible;
        }
        None => **visibility = Visibility::Hidden,
    }
}

#[cfg(test)]
mod tests {
    use bevy::camera::{CameraProjection, RenderTargetInfo};

    use super::*;
    use crate::camera::{iso_camera_transform, iso_projection};
    use crate::map::grid_to_world;

    fn ray(origin: Vec3, direction: Vec3) -> Ray3d {
        Ray3d::new(origin, Dir3::new(direction).unwrap())
    }

    /// A ray from the real rig, parked over `target`, that lands on `hit`.
    fn rig_ray(target: Vec3, hit: Vec3) -> Ray3d {
        let rig = iso_camera_transform(target);
        let forward = *rig.forward();
        Ray3d::new(hit - forward * 50.0, rig.forward())
    }

    #[test]
    fn straight_down_hits_the_tile_below() {
        assert_eq!(ray_to_tile(ray(Vec3::new(3.2, 10.0, 7.7), Vec3::NEG_Y)), Some((3, 8)));
        assert_eq!(ray_to_tile(ray(Vec3::new(0.0, 1.0, 0.0), Vec3::NEG_Y)), Some((0, 0)));
    }

    #[test]
    fn rounds_to_the_nearest_centre_either_side_of_an_edge() {
        let down = Vec3::NEG_Y;
        assert_eq!(ray_to_tile(ray(Vec3::new(5.49, 4.0, 9.0), down)), Some((5, 9)));
        assert_eq!(ray_to_tile(ray(Vec3::new(5.51, 4.0, 9.0), down)), Some((6, 9)));
        assert_eq!(ray_to_tile(ray(Vec3::new(2.0, 4.0, 8.49), down)), Some((2, 8)));
        assert_eq!(ray_to_tile(ray(Vec3::new(2.0, 4.0, 8.51), down)), Some((2, 9)));
        assert_eq!(ray_to_tile(ray(Vec3::new(-0.4, 4.0, -0.6), down)), Some((0, -1)));
    }

    #[test]
    fn real_rig_angle_through_centres_and_near_edges() {
        let target = grid_to_world(40, 21);
        assert_eq!(ray_to_tile(rig_ray(target, grid_to_world(40, 21))), Some((40, 21)));
        assert_eq!(ray_to_tile(rig_ray(target, grid_to_world(31, 27))), Some((31, 27)));
        assert_eq!(ray_to_tile(rig_ray(target, Vec3::new(12.45, 0.0, 3.55))), Some((12, 4)));
        assert_eq!(ray_to_tile(rig_ray(target, Vec3::new(12.55, 0.0, 3.45))), Some((13, 3)));
    }

    #[test]
    fn other_camera_angles() {
        // 60 degrees down, looking along -x.
        let steep = Vec3::new(-0.5, -(3.0_f32).sqrt() / 2.0, 0.0);
        assert_eq!(ray_to_tile(ray(Vec3::new(10.0, 3.0_f32.sqrt(), 4.0), steep)), Some((9, 4)));
        // Shallow, 45 degrees down along +z from height 10.
        assert_eq!(ray_to_tile(ray(Vec3::new(1.0, 10.0, 0.0), Vec3::new(0.0, -1.0, 1.0))), Some((1, 10)));
    }

    #[test]
    fn rays_that_never_reach_the_floor() {
        assert_eq!(ray_to_tile(ray(Vec3::new(0.0, 1.0, 0.0), Vec3::X)), None);
        assert_eq!(ray_to_tile(ray(Vec3::new(0.0, 5.0, 0.0), Vec3::Y)), None);
        assert_eq!(ray_to_tile(ray(Vec3::new(0.0, 5.0, 0.0), Vec3::new(1.0, 0.2, 1.0))), None);
        assert_eq!(ray_to_tile(ray(Vec3::new(0.0, -1.0, 0.0), Vec3::NEG_Y)), None);
    }

    #[test]
    fn tile_edges_face_their_neighbour() {
        let north = tile_edge(0, -1, 0.1);
        assert!((north.min - Vec2::new(-0.5, -0.5)).length() < 1e-6);
        assert!((north.max - Vec2::new(0.5, -0.4)).length() < 1e-6);
        let east = tile_edge(1, 0, 0.1);
        assert!((east.min - Vec2::new(0.4, -0.5)).length() < 1e-6);
        assert!((east.max - Vec2::new(0.5, 0.5)).length() < 1e-6);
    }

    /// The real camera rig on the game's 1280x800 window, without a window.
    fn rig_camera() -> Camera {
        let mut projection = iso_projection();
        projection.update(1280.0, 800.0);
        let mut camera = Camera::default();
        camera.computed.clip_from_view = projection.get_clip_from_view();
        camera.computed.target_info = Some(RenderTargetInfo {
            physical_size: UVec2::new(1280, 800),
            scale_factor: 1.0,
        });
        camera
    }

    #[test]
    fn projecting_a_cell_to_the_screen_and_picking_it_back_round_trips() {
        let camera = rig_camera();
        for (cx, cy) in [(40, 21), (10, 30), (70, 5)] {
            let rig = GlobalTransform::from(iso_camera_transform(grid_to_world(cx, cy)));
            for x in cx - 8..=cx + 8 {
                for y in cy - 8..=cy + 8 {
                    // Centre, and just inside two corners of the cell.
                    for nudge in [Vec3::ZERO, Vec3::new(0.45, 0.0, -0.45), Vec3::new(-0.45, 0.0, 0.45)] {
                        let screen = camera.world_to_viewport(&rig, grid_to_world(x, y) + nudge).unwrap();
                        let picked = camera.viewport_to_world(&rig, screen).ok().and_then(ray_to_tile);
                        assert_eq!(picked, Some((x, y)), "camera on ({cx}, {cy}), screen {screen}");
                    }
                }
            }
        }
    }

    #[test]
    fn the_screen_centre_picks_the_followed_tile() {
        let camera = rig_camera();
        let rig = GlobalTransform::from(iso_camera_transform(grid_to_world(40, 21)));
        let picked = camera.viewport_to_world(&rig, Vec2::new(640.0, 400.0)).ok().and_then(ray_to_tile);
        assert_eq!(picked, Some((40, 21)));
    }
}
