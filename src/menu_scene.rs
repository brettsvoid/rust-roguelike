//! The lit scene behind the main menu: a fixed dungeon corner built from the
//! game's own tile, prop and hero models, two wall torches, the party's torch
//! and a cold glow from the stairs down, under a camera that drifts slowly on
//! `Time`. Spawned on entering the main menu and despawned on leaving it.

use bevy::camera::ScalingMode;
use bevy::prelude::*;

use crate::entity_render::EntityShape;
use crate::lighting::{flame, flame_material, spawn_light, wall_offset, LightSource};
use crate::map::{grid_to_world, Map, Position, TileType};
use crate::models::{model_yaw, tile_model_names, AvailableModels};
use crate::RunState;

pub struct MenuScenePlugin;

impl Plugin for MenuScenePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(RunState::MainMenu), spawn_menu_scene)
            .add_systems(OnExit(RunState::MainMenu), despawn_menu_scene)
            .add_systems(
                Update,
                (drift_camera.after(crate::camera::follow_iso_camera), cast_shadows)
                    .run_if(in_state(RunState::MainMenu)),
            );
    }
}

/// The room: walls along the north and west, open towards the camera.
const WIDTH: usize = 12;
const HEIGHT: usize = 10;
const STAIRS: (i32, i32) = (9, 7);

/// Props by model name, on their cells; those on a wall turn to the room.
const PROPS: [(&str, i32, i32); 8] = [
    ("wall_torch", 4, 0),
    ("wall_torch", 0, 5),
    ("keg", 1, 1),
    ("keg", 2, 1),
    ("weapon_rack", 7, 1),
    ("armour_stand", 9, 1),
    ("table", 3, 5),
    ("chair", 3, 6),
];

/// Who stands in the room, where, and the cell they face.
const FIGURES: [(&str, f32, f32, (f32, f32)); 5] = [
    ("hero_fighter", 6.0, 5.0, (9.0, 7.0)),
    ("hero_ranger", 5.0, 6.2, (9.0, 7.0)),
    ("hero_mage", 6.6, 3.6, (9.0, 7.0)),
    ("hero_cleric", 4.6, 4.2, (9.0, 7.0)),
    ("skeleton", 10.4, 4.6, (6.0, 5.0)),
];

/// The party's torch: warm, held at head height among them.
const PARTY_LIGHT: LightSource = LightSource {
    colour: Color::srgb(1.0, 0.62, 0.32),
    radius: 8.0,
    flicker: 0.15,
    height: 1.9,
};

/// The raws Wall Torch light.
const TORCH: LightSource = LightSource {
    colour: Color::srgb(1.0, 0.6, 0.25),
    radius: 7.0,
    flicker: 0.3,
    height: 1.5,
};

/// Something cold below the stairs.
const ABYSS_GLOW: LightSource = LightSource {
    colour: Color::srgb(0.35, 0.45, 1.0),
    radius: 4.5,
    flicker: 0.25,
    height: 0.4,
};

/// Where the camera looks, and how far it sways. The look point sits left of
/// the room so the room fills the right of the screen, clear of the menu.
const FOCUS: Vec2 = Vec2::new(4.2, 7.4);
const VIEW_HEIGHT: f32 = 10.5;
const YAW: f32 = 45.0;
const PITCH: f32 = 35.0;
const SWAY_YAW: f32 = 5.0;
const SWAY_FOCUS: f32 = 0.35;
/// Seconds for one full sway; slow enough to read as drift.
const SWAY_PERIOD: f32 = 48.0;

#[derive(Component)]
struct MenuScene;

/// The room as a `Map`, so walls take the same pieces as in a level.
fn room() -> Map {
    let mut map = Map::new(WIDTH, HEIGHT, 1);
    for y in 1..HEIGHT as i32 {
        for x in 1..WIDTH as i32 {
            let idx = map.xy_idx(x, y);
            map.tiles[idx] = TileType::Floor;
        }
    }
    let stairs = map.xy_idx(STAIRS.0, STAIRS.1);
    map.tiles[stairs] = TileType::DownStairs;
    map
}

fn spawn_menu_scene(
    mut commands: Commands,
    models: Res<AvailableModels>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let map = room();
    let mut root = commands.spawn((MenuScene, Transform::default(), Visibility::default()));
    root.with_children(|scene| {
        for y in 0..map.height {
            for x in 0..map.width {
                if let Some(model) = models.tile_scene(&tile_model_names(&map, x, y), x, y) {
                    scene.spawn((model, Transform::from_translation(grid_to_world(x, y))));
                }
            }
        }
        for (name, x, y) in PROPS {
            let Some(model) = models.scene(name) else { continue };
            let yaw = model_yaw(&map, Position { x, y }, EntityShape::Prop);
            scene.spawn((model, Transform::from_translation(grid_to_world(x, y)).with_rotation(Quat::from_rotation_y(yaw))));
        }
        for (name, x, y, (to_x, to_y)) in FIGURES {
            let Some(model) = models.scene(name) else { continue };
            // Models face +z; turn each to the cell it watches.
            let yaw = (to_x - x).atan2(to_y - y);
            scene.spawn((model, Transform::from_xyz(x, 0.0, y).with_rotation(Quat::from_rotation_y(yaw))));
        }
    });

    let flame_mesh = meshes.add(flame());
    let mut light = |source: &LightSource, at: Vec3, phase: f32, with_flame: bool| {
        let flame = with_flame.then(|| (flame_mesh.clone(), materials.add(flame_material(source))));
        root.with_children(|scene| {
            let mut holder = scene.spawn((Transform::from_translation(at), Visibility::default()));
            spawn_light(&mut holder, source, Vec3::Y * source.height, phase, flame);
        });
    };
    for (i, &(_, x, y)) in PROPS.iter().enumerate().filter(|(_, p)| p.0 == "wall_torch") {
        let off = wall_offset(&map, Position { x, y });
        light(&TORCH, grid_to_world(x, y) + Vec3::new(off.x, 0.0, off.y), i as f32 * 2.1, true);
    }
    light(&PARTY_LIGHT, Vec3::new(5.9, 0.0, 5.3), 0.7, false);
    light(&ABYSS_GLOW, grid_to_world(STAIRS.0, STAIRS.1), 3.3, false);
}

fn despawn_menu_scene(mut commands: Commands, scene: Query<Entity, With<MenuScene>>) {
    for entity in &scene {
        commands.entity(entity).despawn();
    }
}

/// Only the menu's few lights exist here, so all of them cast shadows.
fn cast_shadows(mut lights: Query<&mut PointLight, Added<PointLight>>) {
    for mut light in &mut lights {
        light.shadow_maps_enabled = true;
    }
}

/// The camera's yaw (degrees) and look point `t` seconds into the menu: a
/// slow sway eased at both ends by the sine.
fn drift(t: f32) -> (f32, Vec2) {
    let phase = t / SWAY_PERIOD * std::f32::consts::TAU;
    let yaw = YAW + SWAY_YAW * phase.sin();
    let focus = FOCUS + Vec2::new(phase.cos() - 1.0, (2.0 * phase).sin() * 0.5) * SWAY_FOCUS;
    (yaw, focus)
}

/// Overrides the game's follow camera while the menu shows.
fn drift_camera(
    time: Res<Time>,
    mut since: Local<f32>,
    state: Res<State<RunState>>,
    rig: Single<(&mut Transform, &mut Projection), With<Camera3d>>,
) {
    if state.is_changed() {
        *since = 0.0;
    }
    *since += time.delta_secs();
    let (yaw, focus) = drift(*since);
    let (yaw, pitch) = (yaw.to_radians(), PITCH.to_radians());
    let (mut transform, mut projection) = rig.into_inner();
    let target = Vec3::new(focus.x, 0.0, focus.y);
    let offset = Vec3::new(yaw.sin() * pitch.cos(), pitch.sin(), yaw.cos() * pitch.cos()) * 50.0;
    *transform = Transform::from_translation(target + offset).looking_at(target, Vec3::Y);
    if let Projection::Orthographic(ortho) = &mut *projection {
        ortho.scaling_mode = ScalingMode::FixedVertical { viewport_height: VIEW_HEIGHT };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_drift_starts_at_rest_and_stays_close() {
        assert_eq!(drift(0.0), (YAW, FOCUS));
        for t in (0..200).map(|i| i as f32 * 0.7) {
            let (yaw, focus) = drift(t);
            assert!((yaw - YAW).abs() <= SWAY_YAW + 1e-4);
            assert!(focus.distance(FOCUS) <= 2.5 * SWAY_FOCUS);
        }
    }

    #[test]
    fn the_room_walls_its_north_and_west_and_holds_the_stairs() {
        let map = room();
        assert_eq!(map.tiles[map.xy_idx(4, 0)], TileType::Wall);
        assert_eq!(map.tiles[map.xy_idx(0, 5)], TileType::Wall);
        assert_eq!(map.tiles[map.xy_idx(5, 5)], TileType::Floor);
        assert_eq!(map.tiles[map.xy_idx(STAIRS.0, STAIRS.1)], TileType::DownStairs);
        for (name, x, y) in PROPS {
            let on_wall = map.tiles[map.xy_idx(x, y)] == TileType::Wall;
            assert_eq!(on_wall, name == "wall_torch", "{name} at {x},{y}");
        }
    }
}
