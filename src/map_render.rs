//! Spawns the entities that draw the map on screen.
//!
//! Every path that puts a map on screen (new game, next level, loading a
//! save, the map visualiser) goes through `spawn_map_tiles`. The game gets 3D
//! meshes styled by `tile_style`; the visualiser (`TileReveal::Preview`)
//! stays 2D with glyphs from `tile_glyph`.

use std::collections::HashMap;

use bevy::prelude::*;

use crate::map::{grid_to_world, Map, Position, Revealed, RevealedState, Tile, TileType, WallGlyph};
use crate::models::{tile_model_name, AvailableModels};

/// Wall height in world units (metres).
pub const WALL_HEIGHT: f32 = 2.0;

/// A grey-box primitive. Each shape is one mesh shared by every tile using it.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum TileShape {
    Floor,
    Wall,
    Water,
    Stairs,
}

impl TileShape {
    fn mesh(self) -> Mesh {
        match self {
            TileShape::Floor | TileShape::Water => Plane3d::default().mesh().size(1.0, 1.0).into(),
            TileShape::Wall => Cuboid::new(1.0, WALL_HEIGHT, 1.0).into(),
            TileShape::Stairs => Cuboid::new(1.0, 0.2, 1.0).into(),
        }
    }

    /// How far the mesh centre sits above (or below) the tile's floor point.
    fn lift(self) -> f32 {
        match self {
            TileShape::Floor => 0.0,
            TileShape::Water => -0.1,
            TileShape::Wall => WALL_HEIGHT / 2.0,
            TileShape::Stairs => 0.1,
        }
    }
}

/// The single source of truth for what each tile looks like in 3D.
pub fn tile_style(tile: TileType) -> (TileShape, Color) {
    match tile {
        TileType::Floor => (TileShape::Floor, Color::srgb(0.45, 0.45, 0.45)),
        TileType::WoodFloor => (TileShape::Floor, Color::srgb(0.45, 0.32, 0.2)),
        TileType::Road => (TileShape::Floor, Color::srgb(0.55, 0.52, 0.45)),
        TileType::Grass => (TileShape::Floor, Color::srgb(0.25, 0.45, 0.2)),
        TileType::Bridge => (TileShape::Floor, Color::srgb(0.5, 0.35, 0.2)),
        TileType::Wall => (TileShape::Wall, Color::srgb(0.65, 0.65, 0.65)),
        TileType::ShallowWater => (TileShape::Water, Color::srgb(0.2, 0.5, 0.6)),
        TileType::DeepWater => (TileShape::Water, Color::srgb(0.1, 0.2, 0.5)),
        TileType::DownStairs => (TileShape::Stairs, Color::srgb(0.2, 0.8, 0.8)),
    }
}

/// Marks an in-game 3D tile; `attach_tile_mesh` gives it its mesh and material.
#[derive(Component)]
pub struct TileMesh(pub TileType);

/// The shared tile meshes and materials, created on first use so there is
/// one mesh per `TileShape` and one bright and one dim material per
/// `TileType`, never per tile.
#[derive(Resource, Default)]
pub struct TileAssets {
    meshes: HashMap<TileShape, Handle<Mesh>>,
    materials: HashMap<TileType, TileMaterials>,
}

struct TileMaterials {
    visible: Handle<StandardMaterial>,
    explored: Handle<StandardMaterial>,
}

/// How much brightness and colour an explored but unseen tile keeps.
const EXPLORED_BRIGHTNESS: f32 = 0.3;
const EXPLORED_SATURATION: f32 = 0.3;

/// The dim, mostly grey colour of an explored but unseen tile, for primitives and models alike.
pub fn dimmed(color: Color) -> Color {
    let c = color.to_srgba();
    let grey = 0.3 * c.red + 0.59 * c.green + 0.11 * c.blue;
    let fade = |v: f32| (grey + (v - grey) * EXPLORED_SATURATION) * EXPLORED_BRIGHTNESS;
    Color::srgba(fade(c.red), fade(c.green), fade(c.blue), c.alpha)
}

impl TileAssets {
    /// The material for a tile in view (`explored == false`) or remembered.
    pub fn material(&self, tile: TileType, explored: bool) -> Option<Handle<StandardMaterial>> {
        let m = self.materials.get(&tile)?;
        Some(if explored { m.explored.clone() } else { m.visible.clone() })
    }
}

fn tile_reveal_state(map: &Map, idx: usize, reveal: TileReveal) -> RevealedState {
    match reveal {
        TileReveal::Hidden => RevealedState::Hidden,
        TileReveal::Preview => RevealedState::Visible,
        TileReveal::FromSave => {
            if map.revealed_tiles[idx] {
                RevealedState::Explored
            } else {
                RevealedState::Hidden
            }
        }
    }
}

fn spawn_map_meshes(commands: &mut Commands, map: &Map, reveal: TileReveal) {
    for y in 0..map.height {
        for x in 0..map.width {
            let idx = map.xy_idx(x, y);
            let tile = map.tiles[idx];
            // Interior walls are never visible - only walls that border a floor.
            if tile == TileType::Wall && !map.is_adjacent_to_floor(x, y) {
                continue;
            }
            let mut entity = commands.spawn((
                Tile,
                TileMesh(tile),
                Position { x, y },
                Revealed(tile_reveal_state(map, idx, reveal)),
            ));
            if tile == TileType::Wall {
                entity.insert(map.wall_glyph_at(x, y));
            }
        }
    }
}

/// Observer: dress a freshly spawned `TileMesh` in its model, or else its
/// shared mesh and material.
pub fn attach_tile_mesh(
    add: On<Add<TileMesh>>,
    mut commands: Commands,
    tiles: Query<(&TileMesh, &Position, Option<&WallGlyph>)>,
    models: Res<AvailableModels>,
    mut assets: ResMut<TileAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Ok((TileMesh(tile), pos, wall)) = tiles.get(add.entity) else {
        return;
    };
    let (shape, color) = tile_style(*tile);
    let mut entity = commands.entity(add.entity);
    entity.insert(Transform::from_translation(grid_to_world(pos.x, pos.y) + Vec3::Y * shape.lift()));
    if let Some(scene) = models.scene(&tile_model_name(*tile, wall.map_or(0, |w| w.0))) {
        // A child back on the floor: the tile keeps the primitive's transform,
        // so the wall cut-away (`occlusion.rs`) squashes a model wall the same way.
        // No `Mesh3d` brings `Visibility` in here, and the scene child needs it on its parent.
        entity
            .insert_if_new(Visibility::default())
            .with_child((scene, Transform::from_xyz(0.0, -shape.lift(), 0.0)));
        return;
    }
    let mesh = assets
        .meshes
        .entry(shape)
        .or_insert_with(|| meshes.add(shape.mesh()))
        .clone();
    let material = assets
        .materials
        .entry(*tile)
        .or_insert_with(|| {
            let mut material = |base_color, unlit| {
                materials.add(StandardMaterial {
                    base_color,
                    perceptual_roughness: 1.0,
                    unlit,
                    ..default()
                })
            };
            TileMaterials {
                visible: material(color, false),
                // Remembered tiles ignore the lights (the dungeon is near black).
                explored: material(dimmed(color), true),
            }
        })
        .visible
        .clone();
    commands.entity(add.entity).insert((Mesh3d(mesh), MeshMaterial3d(material)));
}

/// How freshly spawned tiles start out visibility-wise.
#[derive(Clone, Copy)]
pub enum TileReveal {
    /// Everything starts under fog - new games and level transitions.
    Hidden,
    /// Everything visible - the map generation visualizer.
    Preview,
    /// Previously explored tiles stay dimly visible - loading a save.
    FromSave,
}

/// The single source of truth for what each tile looks like.
/// Walls are drawn with box-drawing characters instead (see `WallGlyph`),
/// but take their color from here.
pub fn tile_glyph(tile: TileType) -> (&'static str, Color) {
    match tile {
        TileType::Floor | TileType::WoodFloor => (".", Color::srgb(0.5, 0.5, 0.5)),
        TileType::Wall => ("#", Color::srgb(0.0, 1.0, 0.0)),
        TileType::DownStairs => (">", Color::srgb(0.0, 1.0, 1.0)),
        TileType::Road => ("≡", Color::srgb(0.5, 0.5, 0.5)),
        TileType::Grass => ("\"", Color::srgb(0.0, 0.8, 0.0)),
        TileType::ShallowWater => ("~", Color::srgb(0.0, 0.8, 0.8)),
        TileType::DeepWater => ("~", Color::srgb(0.0, 0.3, 0.8)),
        TileType::Bridge => (".", Color::srgb(0.6, 0.4, 0.2)),
    }
}

/// Spawn a tile entity for every drawable tile in the map: glyphs for the
/// visualiser (`Preview`), 3D meshes for the game.
pub fn spawn_map_tiles(commands: &mut Commands, map: &Map, font: &TextFont, reveal: TileReveal) {
    match reveal {
        TileReveal::Preview => spawn_map_glyphs(commands, map, font, reveal),
        TileReveal::Hidden | TileReveal::FromSave => spawn_map_meshes(commands, map, reveal),
    }
}

fn spawn_map_glyphs(commands: &mut Commands, map: &Map, font: &TextFont, reveal: TileReveal) {
    for y in 0..map.height {
        for x in 0..map.width {
            let idx = map.xy_idx(x, y);
            let tile = map.tiles[idx];

            let revealed = tile_reveal_state(map, idx, reveal);

            if tile == TileType::Wall {
                // Interior walls are never visible - only spawn walls that
                // border a floor, and pick a box-drawing glyph that connects
                // to the neighboring walls.
                if map.is_adjacent_to_floor(x, y) {
                    let wall_glyph = map.wall_glyph_at(x, y);
                    let (_, color) = tile_glyph(tile);
                    commands.spawn((
                        Tile,
                        Position { x, y },
                        wall_glyph,
                        Text2d::new(wall_glyph.to_char().to_string()),
                        font.clone(),
                        TextColor(color),
                        Revealed(revealed),
                    ));
                }
                continue;
            }

            let (glyph, color) = match (tile, reveal) {
                // The visualizer shows in-progress maps where stairs may move
                // around - draw them as plain floor until the map is final.
                (TileType::DownStairs, TileReveal::Preview) => tile_glyph(TileType::Floor),
                _ => tile_glyph(tile),
            };

            commands.spawn((
                Tile,
                Position { x, y },
                Text2d::new(glyph),
                font.clone(),
                TextColor(color),
                Revealed(revealed),
            ));
        }
    }
}
