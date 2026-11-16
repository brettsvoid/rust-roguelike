//! Grey-box 3D bodies for everything that stands on the map: creatures,
//! items, props and bloodstains.
//!
//! Spawn an entity with an `EntityShape`, a `Renderable` and a `Position`;
//! `attach_entity_mesh` dresses it in a shared mesh and material and
//! `translate_positions` (in `map.rs`) keeps it on its tile. An entity with a
//! `Model` whose file exists gets that model instead (see `models.rs`).

use std::collections::HashMap;

use bevy::prelude::*;

use crate::map::{grid_to_world, Position};
use crate::models::{AvailableModels, Model};

/// What an entity is drawn as. One mesh per shape, shared by every entity.
#[derive(Component, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum EntityShape {
    Creature,
    Item,
    Prop,
    Door,
    OpenDoor,
    Stain,
}

impl EntityShape {
    fn mesh(self) -> Mesh {
        match self {
            EntityShape::Creature => Capsule3d::new(0.3, 0.8).into(),
            EntityShape::Item => Cuboid::from_length(0.3).into(),
            EntityShape::Prop => Cuboid::from_length(0.6).into(),
            EntityShape::Door => Cuboid::new(0.9, 1.8, 0.9).into(),
            EntityShape::OpenDoor => Cuboid::new(0.9, 0.05, 0.9).into(),
            EntityShape::Stain => blood_splat(),
        }
    }

    /// How far the mesh centre sits above the tile's floor point.
    pub fn lift(self) -> f32 {
        match self {
            EntityShape::Creature => 0.7,
            EntityShape::Item => 0.15,
            EntityShape::Prop => 0.3,
            EntityShape::Door => 0.9,
            EntityShape::OpenDoor => 0.025,
            // Just clear of the floor quad so the two never fight over depth.
            EntityShape::Stain => 0.01,
        }
    }
}

/// A flat blood decal: a pool with droplets flung round it, lying on the floor.
fn blood_splat() -> Mesh {
    let flat = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
    let mut splat = Circle::new(0.24).mesh().resolution(10).build().rotated_by(flat);
    for (x, z, r) in [(0.2, 0.08, 0.12), (-0.12, 0.18, 0.09), (-0.2, -0.14, 0.07), (0.08, -0.26, 0.05), (0.33, -0.12, 0.04)] {
        let drop = Circle::new(r).mesh().resolution(8).build();
        let drop = drop.transformed_by(Transform::from_xyz(x, 0.0, z).with_rotation(flat));
        splat.merge(&drop).expect("circle meshes share attributes");
    }
    splat
}

/// The colour from the raws: it tints the grey-box body of an entity without a model.
#[derive(Component, Clone, Debug)]
pub struct Renderable {
    pub fg: Color,
}

/// Shared entity meshes and materials: one mesh per `EntityShape` and one
/// material per distinct colour, never one per entity.
#[derive(Resource, Default)]
pub struct EntityAssets {
    meshes: HashMap<EntityShape, Handle<Mesh>>,
    /// Keyed by sRGBA bytes.
    materials: HashMap<[u8; 4], Handle<StandardMaterial>>,
}

pub fn entity_transform(shape: EntityShape, pos: Position) -> Transform {
    Transform::from_translation(grid_to_world(pos.x, pos.y) + Vec3::Y * shape.lift())
}

/// A model scene under an entity, offset so the model's origin is on the floor.
#[derive(Component)]
pub struct ModelScene;

/// Observer: dress an entity in its model, or else its shape's mesh and its
/// colour's material. Fires on every insert, so swapping the shape (a door
/// opening) re-dresses it; an open door looks for `<model>_open`.
#[allow(clippy::too_many_arguments)]
pub fn attach_entity_mesh(
    insert: On<Insert<EntityShape>>,
    mut commands: Commands,
    entities: Query<(&EntityShape, &Renderable, Option<&Position>, Option<&Model>, Option<&Children>)>,
    scenes: Query<(), With<ModelScene>>,
    models: Res<AvailableModels>,
    mut assets: ResMut<EntityAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Ok((&shape, renderable, pos, model, children)) = entities.get(insert.entity) else {
        return;
    };
    for &child in children.into_iter().flatten() {
        if scenes.contains(child) {
            commands.entity(child).despawn();
        }
    }
    // Backpack items have no position; they get a transform when dropped.
    if let Some(&pos) = pos {
        commands.entity(insert.entity).insert(entity_transform(shape, pos));
    }
    let scene = model.and_then(|Model(name)| match shape {
        EntityShape::OpenDoor => models.scene(&format!("{name}_open")),
        _ => models.scene(name),
    });
    if let Some(scene) = scene {
        commands
            .entity(insert.entity)
            .remove::<(Mesh3d, MeshMaterial3d<StandardMaterial>)>()
            .insert_if_new(Visibility::default())
            .with_child((ModelScene, scene, Transform::from_xyz(0.0, -shape.lift(), 0.0)));
        return;
    }
    let mesh = assets
        .meshes
        .entry(shape)
        .or_insert_with(|| meshes.add(shape.mesh()))
        .clone();
    let material = assets
        .materials
        .entry(renderable.fg.to_srgba().to_u8_array())
        .or_insert_with(|| {
            materials.add(StandardMaterial {
                base_color: renderable.fg,
                perceptual_roughness: 1.0,
                ..default()
            })
        })
        .clone();
    commands.entity(insert.entity).insert((Mesh3d(mesh), MeshMaterial3d(material)));
}
