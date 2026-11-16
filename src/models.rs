//! The model pipeline: glTF scenes from `assets/models/<name>.glb` stand in
//! for the grey-box primitives, with no code change per model.
//!
//! Mobs, items and props name their model in the raws (`Model`); tiles name
//! theirs by `tile_model_name`. `attach_entity_mesh` and `attach_tile_mesh`
//! ask `AvailableModels` and keep the primitive when there is no file.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use bevy::asset::io::file::FileAssetReader;
use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use bevy::world_serialization::WorldInstanceReady;

use crate::entity_render::{EntityShape, ModelScene};
use crate::levels::TOWN_DEPTH;
use crate::lighting::wall_offset;
use crate::map::{Map, Position, Revealed, RevealedState, TileType};
use crate::map_render::{dimmed, TileMesh};

pub struct ModelsPlugin;

impl Plugin for ModelsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AvailableModels>()
            .init_resource::<DimmedMaterials>()
            .add_observer(model_ready)
            .add_systems(Update, (update_model_fog, face_prop_models));
    }
}

/// The raws `model` name of a mob, item or prop.
#[derive(Component, Clone, Debug)]
pub struct Model(pub String);

/// Every model in `assets/models/`, found and loaded once at startup. Deciding
/// here whether a file exists keeps a missing model silent: no load is tried.
// shortcut: holds every model in memory for the whole run; load per level if the kit grows large.
#[derive(Resource)]
pub struct AvailableModels(pub(crate) HashMap<String, Handle<WorldAsset>>);

impl FromWorld for AvailableModels {
    fn from_world(world: &mut World) -> Self {
        let dir = FileAssetReader::get_base_path().join("assets/models");
        let asset_server = world.resource::<AssetServer>();
        let names = scan(&dir);
        Self(
            names
                .iter()
                .filter_map(|name| {
                    let path = model_path(&names, name)?;
                    Some((name.clone(), asset_server.load(GltfAssetLabel::Scene(0).from_asset(path))))
                })
                .collect(),
        )
    }
}

impl AvailableModels {
    /// The scene to spawn for `name`, or `None` to keep the primitive.
    pub fn scene(&self, name: &str) -> Option<WorldAssetRoot> {
        self.0.get(name).cloned().map(WorldAssetRoot)
    }

    /// The scene for a tile from the first of `names` that has a file: for each,
    /// `<name>`, `<name>_2` or `<name>_3` picked by `tile_variant`, falling back
    /// to `<name>` when that variant has no file.
    pub fn tile_scene(&self, names: &[String], x: i32, y: i32) -> Option<WorldAssetRoot> {
        names.iter().find_map(|name| {
            match tile_variant(x, y) {
                1 => None,
                n => self.scene(&format!("{name}_{n}")),
            }
            .or_else(|| self.scene(name))
        })
    }

    /// True once every model has loaded or failed (a bad file logs one error).
    pub fn loaded(&self, asset_server: &AssetServer) -> bool {
        self.0.values().all(|handle| {
            asset_server.is_loaded_with_dependencies(handle)
                || asset_server.recursive_dependency_load_state(handle).is_failed()
        })
    }
}

fn model_path(names: &HashSet<String>, name: &str) -> Option<String> {
    names.contains(name).then(|| format!("models/{name}.glb"))
}

fn scan(dir: &Path) -> HashSet<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return HashSet::new();
    };
    entries
        .filter_map(|e| {
            let path = e.ok()?.path();
            (path.extension()? == "glb").then(|| path.file_stem()?.to_str().map(String::from))?
        })
        .collect()
}

/// The model file for a tile. Walls add their `WallGlyph` mask (N=1, S=2,
/// W=4, E=8, set where that neighbour is a wall next to floor), so the kit
/// can have a piece per join.
pub fn tile_model_name(tile: TileType, wall_mask: u8) -> String {
    let name = match tile {
        TileType::Wall => return format!("tile_wall_{wall_mask}"),
        TileType::Floor => "floor",
        TileType::WoodFloor => "wood_floor",
        TileType::Road => "road",
        TileType::Grass => "grass",
        TileType::Bridge => "bridge",
        TileType::ShallowWater => "shallow_water",
        TileType::DeepWater => "deep_water",
        TileType::DownStairs => "down_stairs",
        TileType::UpStairs => "up_stairs",
    };
    format!("tile_{name}")
}

/// The model files a map tile tries, best first. Walls take their mask from
/// `Map::wall_glyph_at`. The town (depth 0) dresses its walls from the town
/// kit: a wall beside a building's floor (wood floor, or the stone floor of a
/// doorway or hovel) is `tile_house_wall_<mask>`, any other (the town wall,
/// ruins) `tile_town_wall_<mask>`. Both fall back to the shared `tile_model_name`.
pub fn tile_model_names(map: &Map, x: i32, y: i32) -> Vec<String> {
    let tile = map.tiles[map.xy_idx(x, y)];
    let wall_mask = if tile == TileType::Wall { map.wall_glyph_at(x, y).0 } else { 0 };
    let shared = tile_model_name(tile, wall_mask);
    if tile != TileType::Wall || map.depth != TOWN_DEPTH {
        return vec![shared];
    }
    let house = (-1..=1).flat_map(|dy| (-1..=1).map(move |dx| (x + dx, y + dy))).any(|(nx, ny)| {
        nx >= 0
            && ny >= 0
            && nx < map.width
            && ny < map.height
            && matches!(map.tiles[map.xy_idx(nx, ny)], TileType::WoodFloor | TileType::Floor)
    });
    let kit = if house { "house" } else { "town" };
    vec![format!("tile_{kit}_wall_{wall_mask}"), shared]
}

/// Which of three variants (1 to 3) a cell shows, from a hash of the cell
/// so the same map always looks the same.
pub fn tile_variant(x: i32, y: i32) -> u32 {
    let mut h = (x as u32).wrapping_mul(0x9E37_79B1) ^ (y as u32).wrapping_mul(0x85EB_CA77);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h % 3 + 1
}

/// How far to turn a model about the vertical axis. Models face south (+z); a
/// prop on a wall (a wall torch) turns to its open side, the side
/// `lighting::wall_offset` hangs the flame on, and a door between walls to
/// its north and south turns a quarter so it spans the passage. Furniture
/// beside a wall turns its back to it, trying the wall north, west, east,
/// then south (so it faces the camera's sides first).
pub fn model_yaw(map: &Map, pos: Position, shape: EntityShape) -> f32 {
    let is_wall = |x: i32, y: i32| {
        x >= 0 && y >= 0 && x < map.width && y < map.height && map.tiles[map.xy_idx(x, y)] == TileType::Wall
    };
    let open = wall_offset(map, pos);
    if open != Vec2::ZERO {
        return open.x.atan2(open.y);
    }
    match shape {
        EntityShape::Door | EntityShape::OpenDoor if is_wall(pos.x, pos.y - 1) && is_wall(pos.x, pos.y + 1) => {
            std::f32::consts::FRAC_PI_2
        }
        EntityShape::Prop => [(0, -1), (-1, 0), (1, 0), (0, 1)]
            .into_iter()
            .find(|&(dx, dy)| is_wall(pos.x + dx, pos.y + dy))
            .map_or(0.0, |(dx, dy)| ((-dx) as f32).atan2((-dy) as f32)),
        _ => 0.0,
    }
}

/// Turns each new prop model to the map (see `model_yaw`). A door that opens
/// gets a new model scene, so it is turned again.
fn face_prop_models(
    mut scenes: Query<(&ChildOf, &mut Transform), Added<ModelScene>>,
    props: Query<(&Position, &EntityShape)>,
    map: Option<Res<Map>>,
) {
    let Some(map) = map else {
        return;
    };
    for (child_of, mut transform) in &mut scenes {
        if let Ok((&pos, &shape)) = props.get(child_of.parent()) {
            transform.rotation = Quat::from_rotation_y(model_yaw(&map, pos, shape));
        }
    }
}

/// The bright and dim material of one mesh inside a tile model.
#[derive(Component)]
struct ModelFog {
    visible: Handle<StandardMaterial>,
    explored: Handle<StandardMaterial>,
}

/// One dim copy per model material, shared by every tile using it.
#[derive(Resource, Default)]
struct DimmedMaterials(HashMap<AssetId<StandardMaterial>, Handle<StandardMaterial>>);

/// Fog of war for tile models: each mesh swaps between its own material and
/// a dimmed copy, the way grey-box tiles swap theirs.
fn model_ready(
    ready: On<WorldInstanceReady>,
    mut commands: Commands,
    tiles: Query<(&Revealed, &TileMesh)>,
    parents: Query<&ChildOf>,
    children: Query<&Children>,
    mut meshes: Query<&mut MeshMaterial3d<StandardMaterial>>,
    mut dimmed_materials: ResMut<DimmedMaterials>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // A tile model's scene is a child of the tile (see `attach_tile_mesh`).
    let Ok((revealed, TileMesh(tile))) = parents.get(ready.entity).and_then(|c| tiles.get(c.parent())) else {
        return;
    };
    for entity in children.iter_descendants(ready.entity) {
        let Ok(mut material) = meshes.get_mut(entity) else {
            continue;
        };
        let visible = material.0.clone();
        let explored = dimmed_materials
            .0
            .entry(visible.id())
            .or_insert_with(|| {
                let mut dim = materials.get(&visible).cloned().unwrap_or_default();
                dim.base_color = dimmed(dim.base_color);
                // Remembered tiles ignore the lights, like the grey-box ones.
                dim.unlit = true;
                materials.add(dim)
            })
            .clone();
        if matches!(revealed.0, RevealedState::Explored) {
            material.0 = explored.clone();
        }
        commands.entity(entity).insert(ModelFog { visible, explored });
        if !crate::map_render::casts_shadow(*tile) {
            commands.entity(entity).insert(NotShadowCaster);
        }
    }
}

fn update_model_fog(
    tiles: Query<(Entity, &Revealed), (With<TileMesh>, With<Children>, Changed<Revealed>)>,
    children: Query<&Children>,
    mut meshes: Query<(&ModelFog, &mut MeshMaterial3d<StandardMaterial>)>,
) {
    for (tile, revealed) in &tiles {
        for entity in children.iter_descendants(tile) {
            if let Ok((fog, mut material)) = meshes.get_mut(entity) {
                let handle = match revealed.0 {
                    RevealedState::Explored => &fog.explored,
                    _ => &fog.visible,
                };
                if material.0 != *handle {
                    material.0 = handle.clone();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The whole path headless: a `.glb` in the folder spawns as meshes. Catches
    /// scene components the type registry is missing (see `reflect_auto_register`).
    #[test]
    fn a_model_file_spawns_its_meshes() {
        use bevy::render::{settings::WgpuSettings, RenderPlugin};
        let mut app = App::new();
        app.add_plugins((
            DefaultPlugins
                .set(RenderPlugin {
                    render_creation: WgpuSettings { backends: None, ..default() }.into(),
                    ..default()
                })
                .disable::<bevy::winit::WinitPlugin>(),
            ModelsPlugin,
        ));
        app.finish();
        app.cleanup();
        let scene = app.world().resource::<AvailableModels>().scene("test_pyramid");
        app.world_mut().spawn(scene.expect("test_pyramid.glb is committed"));
        let mut meshes = app
            .world_mut()
            .query_filtered::<Entity, With<MeshMaterial3d<StandardMaterial>>>();
        for _ in 0..500 {
            if meshes.iter(app.world()).count() > 0 {
                break;
            }
            app.update();
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(meshes.iter(app.world()).count(), 1, "model never spawned its mesh");
    }

    #[test]
    fn tiles_name_their_model_file() {
        assert_eq!(tile_model_name(TileType::Floor, 0), "tile_floor");
        assert_eq!(tile_model_name(TileType::DownStairs, 0), "tile_down_stairs");
        assert_eq!(tile_model_name(TileType::Wall, 10), "tile_wall_10");
    }

    #[test]
    fn town_walls_try_the_town_kit_first() {
        // Row 0: a town wall on the grass. Rows 2 and 3: a house wall over wood floor.
        let mut town = Map::new(4, 4, TOWN_DEPTH);
        town.tiles = vec![TileType::Grass; 16];
        for (x, y, tile) in [(0, 0, TileType::Wall), (1, 0, TileType::Wall), (1, 2, TileType::Wall), (1, 3, TileType::WoodFloor)] {
            let idx = town.xy_idx(x, y);
            town.tiles[idx] = tile;
        }
        assert_eq!(tile_model_names(&town, 1, 0), ["tile_town_wall_4", "tile_wall_4"]);
        assert_eq!(tile_model_names(&town, 1, 2), ["tile_house_wall_0", "tile_wall_0"]);
        assert_eq!(tile_model_names(&town, 3, 3), ["tile_grass"]);
        let mut dungeon = town;
        dungeon.depth = 1;
        assert_eq!(tile_model_names(&dungeon, 1, 0), ["tile_wall_4"]);
    }

    #[test]
    fn tile_variants_are_fixed_per_cell_and_all_used() {
        let mut counts = [0; 3];
        for y in 0..30 {
            for x in 0..30 {
                let v = tile_variant(x, y);
                assert_eq!(v, tile_variant(x, y));
                counts[v as usize - 1] += 1;
            }
        }
        assert!(counts.iter().all(|&n| n > 250), "uneven variants: {counts:?}");
    }

    #[test]
    fn props_turn_to_their_walls() {
        // Row 1 is a wall with floor south of (1, 1); column 3 is a door between walls north and south.
        let mut map = Map::new(5, 5, 2);
        for (x, y) in [(1, 2), (3, 2), (1, 3), (2, 2)] {
            let idx = map.xy_idx(x, y);
            map.tiles[idx] = TileType::Floor;
        }
        let quarter = std::f32::consts::FRAC_PI_2;
        let prop = EntityShape::Prop;
        let door = EntityShape::Door;
        assert_eq!(model_yaw(&map, Position { x: 1, y: 1 }, prop), 0.0, "torch faces south");
        assert_eq!(model_yaw(&map, Position { x: 0, y: 2 }, prop), quarter, "torch faces east");
        assert_eq!(model_yaw(&map, Position { x: 3, y: 2 }, door), quarter, "door between north and south walls");
        assert_eq!(model_yaw(&map, Position { x: 1, y: 2 }, door), 0.0, "door between east and west walls");
    }

    #[test]
    fn furniture_turns_its_back_to_a_wall() {
        // A 3 by 3 room of floor, (1, 1) to (3, 3), inside walls.
        let mut map = Map::new(5, 5, 2);
        for y in 1..=3 {
            for x in 1..=3 {
                let idx = map.xy_idx(x, y);
                map.tiles[idx] = TileType::Floor;
            }
        }
        let yaw = |x, y, shape| model_yaw(&map, Position { x, y }, shape);
        let quarter = std::f32::consts::FRAC_PI_2;
        let prop = EntityShape::Prop;
        assert_eq!(yaw(2, 1, prop), 0.0, "north wall: faces south");
        assert_eq!(yaw(1, 1, prop), 0.0, "north-west corner: the north wall wins");
        assert_eq!(yaw(1, 2, prop), quarter, "west wall: faces east");
        assert_eq!(yaw(3, 2, prop), -quarter, "east wall: faces west");
        assert!((yaw(2, 3, prop) - std::f32::consts::PI).abs() < 1e-6, "south wall: faces north");
        assert_eq!(yaw(2, 2, prop), 0.0, "mid-room: faces south");
        assert_eq!(yaw(1, 2, EntityShape::Creature), 0.0, "creatures are not turned");
    }

    #[test]
    fn only_files_in_the_models_folder_resolve() {
        let names = scan(&Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/models"));
        assert_eq!(
            model_path(&names, "test_pyramid").as_deref(),
            Some("models/test_pyramid.glb")
        );
        assert_eq!(model_path(&names, "no_such_model"), None);
        assert!(scan(Path::new("no/such/dir")).is_empty());
    }
}
