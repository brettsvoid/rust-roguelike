//! The model pipeline: glTF scenes from `assets/models/<name>.glb` stand in
//! for the grey-box primitives, with no code change per model.
//!
//! Mobs, items and props name their model in the raws (`Model`); tiles name
//! theirs by `tile_model_name`. `attach_entity_mesh` and `attach_tile_mesh`
//! ask `AvailableModels` and keep the primitive when there is no file.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use bevy::asset::io::file::FileAssetReader;
use bevy::prelude::*;
use bevy::world_serialization::WorldInstanceReady;

use crate::map::{Revealed, RevealedState, TileType};
use crate::map_render::{dimmed, TileMesh};

pub struct ModelsPlugin;

impl Plugin for ModelsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AvailableModels>()
            .init_resource::<DimmedMaterials>()
            .add_observer(model_ready)
            .add_systems(Update, update_model_fog);
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
    tiles: Query<&Revealed, With<TileMesh>>,
    parents: Query<&ChildOf>,
    children: Query<&Children>,
    mut meshes: Query<&mut MeshMaterial3d<StandardMaterial>>,
    mut dimmed_materials: ResMut<DimmedMaterials>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // A tile model's scene is a child of the tile (see `attach_tile_mesh`).
    let Ok(revealed) = parents.get(ready.entity).and_then(|c| tiles.get(c.parent())) else {
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
