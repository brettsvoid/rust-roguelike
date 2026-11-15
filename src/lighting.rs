//! Lighting: near-black dungeons lit by torches and candles, and a low
//! evening sun over the town.
//!
//! Anything with a `LightSource` (props with a raws `light` entry, and the
//! player's torch) gets a child `PointLight` and, unless it is the player, a
//! small glowing flame that blooms. Lights flicker on `Time`, which the
//! screenshot harness steps by a fixed amount per frame, so screenshots stay
//! byte-identical. Only the `MAX_SHADOW_LIGHTS` lights nearest the player
//! cast shadows. `apply_environment` sets the ambient light, fog and sun for
//! the current depth.

use bevy::camera::Hdr;
use bevy::light::{CascadeShadowConfigBuilder, NotShadowCaster};
use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;

use crate::entity_render::EntityShape;
use crate::map::{Map, Position, TileType};
use crate::player::Player;

/// A light carried by an entity. Set from the raws `light` entry for props.
#[derive(Component, Clone, Debug)]
pub struct LightSource {
    pub colour: Color,
    /// How far the light reaches, in tiles.
    pub radius: f32,
    /// How deep the flicker dips: 0 is steady, 1 can dip to black.
    pub flicker: f32,
    /// Height of the flame above the floor, in world units.
    pub height: f32,
}

/// The torch every player carries.
const PLAYER_TORCH: LightSource = LightSource {
    colour: Color::srgb(1.0, 0.62, 0.32),
    radius: 9.0,
    flicker: 0.15,
    height: 1.8,
};

/// The torch is held out towards the camera (which looks from +x, +z), so
/// the side of the player we see is lit and its shadow falls behind it.
const PLAYER_TORCH_OFFSET: Vec2 = Vec2::new(0.4, 0.4);

/// Point lights are cube shadow maps (six passes each), so only the nearest
/// few to the player cast shadows.
const MAX_SHADOW_LIGHTS: usize = 4;

/// Lumens per square tile of radius, so a light is about equally bright at
/// the same fraction of its radius whatever its size.
const LUMENS_PER_RADIUS_SQUARED: f32 = 6_000.0;

/// The town is depth 1 (see `levels.rs`).
const TOWN_DEPTH: i32 = 1;

pub struct LightingPlugin;

impl Plugin for LightingPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(give_player_torch).add_systems(
            Update,
            (apply_environment, attach_lights, flicker_lights, cap_shadow_casters).chain(),
        );
    }
}

#[derive(Component)]
struct Flicker {
    base: f32,
    amount: f32,
    phase: f32,
}

#[derive(Component)]
struct Sun;

fn give_player_torch(add: On<Add<Player>>, mut commands: Commands) {
    commands.entity(add.entity).insert(PLAYER_TORCH);
}

/// Where a light sits relative to its tile centre on the floor plane. A light
/// on a wall tile (a wall torch) hangs just off the face of the first open
/// side, so the wall shadows the room behind it.
fn wall_offset(map: &Map, pos: Position) -> Vec2 {
    let is_wall = |x: i32, y: i32| {
        x < 0 || y < 0 || x >= map.width || y >= map.height || map.tiles[map.xy_idx(x, y)] == TileType::Wall
    };
    if !is_wall(pos.x, pos.y) {
        return Vec2::ZERO;
    }
    [(0, 1), (1, 0), (0, -1), (-1, 0)]
        .into_iter()
        .find(|&(dx, dy)| !is_wall(pos.x + dx, pos.y + dy))
        .map_or(Vec2::ZERO, |(dx, dy)| Vec2::new(dx as f32, dy as f32) * 0.6)
}

fn attach_lights(
    mut commands: Commands,
    map: Res<Map>,
    sources: Query<(Entity, &LightSource, &Position, &EntityShape, Has<Player>), Added<LightSource>>,
    mut flame_mesh: Local<Option<Handle<Mesh>>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (entity, source, &pos, shape, is_player) in &sources {
        let offset = if is_player { PLAYER_TORCH_OFFSET } else { wall_offset(&map, pos) };
        // Children sit relative to the body, which is lifted off the floor.
        let local = Vec3::new(offset.x, source.height - shape.lift(), offset.y);
        // A fixed phase per tile so neighbouring flames don't pulse together.
        let phase = ((pos.x as f32 * 12.989 + pos.y as f32 * 78.233).sin() * 43_758.5).fract() * std::f32::consts::TAU;
        let base = LUMENS_PER_RADIUS_SQUARED * source.radius * source.radius;
        let mut parent = commands.entity(entity);
        parent.with_child((
            PointLight {
                color: source.colour,
                intensity: base,
                range: source.radius,
                ..default()
            },
            Transform::from_translation(local),
            // Lights spill into view from around corners, so they ignore the
            // fog of war that hides their prop.
            Visibility::Visible,
            Flicker {
                base,
                amount: source.flicker,
                phase,
            },
        ));
        if is_player {
            continue;
        }
        let mesh = flame_mesh
            .get_or_insert_with(|| meshes.add(Sphere::new(0.07).mesh().ico(1).unwrap()))
            .clone();
        let material = materials.add(StandardMaterial {
            base_color: source.colour,
            // Far above 1.0 in HDR, so the bloom pass makes it glow.
            emissive: source.colour.to_linear() * 400.0,
            ..default()
        });
        parent.with_child((
            Mesh3d(mesh),
            MeshMaterial3d(material),
            Transform::from_translation(local),
            // The flame sits on its own light; casting would black out the light.
            NotShadowCaster,
        ));
    }
}

fn flicker_lights(time: Res<Time>, mut lights: Query<(&mut PointLight, &Flicker)>) {
    let t = time.elapsed_secs();
    for (mut light, f) in &mut lights {
        if f.amount == 0.0 {
            continue;
        }
        // Three detuned sines read as an uneven flame rather than a pulse.
        let wave = 0.5 * (t * 7.0 + f.phase).sin()
            + 0.3 * (t * 13.0 + f.phase * 2.3).sin()
            + 0.2 * (t * 23.0 + f.phase * 4.1).sin();
        light.intensity = f.base * (1.0 - f.amount * (0.5 + 0.5 * wave));
    }
}

/// Lets only the lights nearest the player cast shadows.
fn cap_shadow_casters(
    player: Option<Single<&GlobalTransform, With<Player>>>,
    mut lights: Query<(Entity, &GlobalTransform, &mut PointLight)>,
) {
    let Some(player) = player else {
        return;
    };
    let centre = player.translation();
    let mut by_distance: Vec<(f32, Entity)> = lights
        .iter()
        .map(|(e, t, _)| (t.translation().distance_squared(centre), e))
        .collect();
    // Entity breaks ties so the choice never depends on query order.
    by_distance.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    for (rank, (_, entity)) in by_distance.into_iter().enumerate() {
        let Ok((_, _, mut light)) = lights.get_mut(entity) else {
            continue;
        };
        let cast = rank < MAX_SHADOW_LIGHTS;
        if light.shadow_maps_enabled != cast {
            light.shadow_maps_enabled = cast;
        }
    }
}

/// The ambient light, fog and sun for the current depth: a near-black
/// dungeon, or the town under a low sun. Only does work when the depth
/// switches between the two.
fn apply_environment(
    mut commands: Commands,
    map: Res<Map>,
    mut current: Local<Option<bool>>,
    camera: Single<(Entity, &mut Camera), With<Camera3d>>,
    suns: Query<Entity, With<Sun>>,
) {
    let town = map.depth == TOWN_DEPTH;
    if *current == Some(town) {
        return;
    }
    *current = Some(town);
    for sun in &suns {
        commands.entity(sun).despawn();
    }

    let (ambient, ambient_brightness, fog) = if town {
        (Color::srgb(0.45, 0.5, 0.75), 120.0, Color::srgb(0.12, 0.1, 0.14))
    } else {
        (Color::srgb(0.5, 0.55, 0.8), 6.0, Color::BLACK)
    };
    let (camera, mut render) = camera.into_inner();
    render.clear_color = ClearColorConfig::Custom(fog);
    commands.entity(camera).insert((
        Hdr,
        Bloom::NATURAL,
        AmbientLight {
            color: ambient,
            brightness: ambient_brightness,
            ..default()
        },
        // The camera is 50 units back, so this darkens the far side of the view.
        DistanceFog {
            color: fog,
            falloff: FogFalloff::Linear { start: 48.0, end: 75.0 },
            ..default()
        },
    ));

    if town {
        commands.spawn((
            Sun,
            DirectionalLight {
                color: Color::srgb(1.0, 0.6, 0.35),
                illuminance: 2_500.0,
                shadow_maps_enabled: true,
                ..default()
            },
            // Low in the west, so buildings throw long shadows across the streets.
            Transform::default().looking_to(Vec3::new(1.0, -0.4, -0.5), Vec3::Y),
            CascadeShadowConfigBuilder {
                num_cascades: 1,
                maximum_distance: 90.0,
                ..default()
            }
            .build(),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wall_torch_hangs_off_the_open_face() {
        let mut map = Map::new(3, 3, 2);
        let floor = map.xy_idx(1, 2);
        map.tiles[floor] = TileType::Floor;
        assert_eq!(wall_offset(&map, Position { x: 1, y: 1 }), Vec2::new(0.0, 0.6));
        assert_eq!(wall_offset(&map, Position { x: 1, y: 2 }), Vec2::ZERO);
    }
}
