//! Wall occlusion: keeps the party and the floor in view under the
//! isometric camera.
//!
//! The camera looks from the +x, +z side, so a wall hides the tiles up to two
//! steps towards -x, -z behind it. Any wall that would hide a visible floor
//! tile or a party member is cut down to a low stub (`walls_to_cut`).
//!
//! Town buildings (closed `WoodFloor` rooms) get a flat roof. A roof hides
//! while a party member is inside its building or behind it; while it shows,
//! its walls stay full height so it never floats.

use bevy::camera::visibility::VisibilitySystems;
use bevy::prelude::*;
use bevy::transform::TransformSystems;

use crate::map::{Map, Position, Revealed, RevealedState, TileType};
use crate::map_render::{TileMesh, WALL_HEIGHT};
use crate::party::PartyMember;

/// How tall a cut wall stays, in world units.
const STUB_HEIGHT: f32 = 0.25;
/// Roof slab thickness. It sits on top of the walls.
const ROOF_THICKNESS: f32 = 0.1;

/// The steps from a tile to every wall that hides (some of) it. A 2 m wall
/// under a 35 degree pitch hides about 2 tiles diagonally behind it.
const OCCLUDER_OFFSETS: [(i32, i32); 6] = [(1, 0), (0, 1), (1, 1), (2, 1), (1, 2), (2, 2)];

pub struct OcclusionPlugin;

impl Plugin for OcclusionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Buildings>()
            .add_systems(Update, spawn_roofs)
            .add_systems(
                PostUpdate,
                apply_occlusion
                    .before(TransformSystems::Propagate)
                    .before(VisibilitySystems::VisibilityPropagate),
            );
    }
}

/// Which building (if any) each map tile belongs to: its floor, door and walls.
#[derive(Resource, Default)]
struct Buildings {
    of_tile: Vec<Option<usize>>,
    count: usize,
}

/// A roof slab over one building tile.
#[derive(Component)]
struct Roof(usize);

/// The walls that would hide a visible non-wall tile or a party member, by tile index.
pub fn walls_to_cut(map: &Map, party: &[(i32, i32)]) -> Vec<bool> {
    let mut cut = vec![false; map.tiles.len()];
    let mut mark = |(x, y): (i32, i32)| {
        for (dx, dy) in OCCLUDER_OFFSETS {
            let (wx, wy) = (x + dx, y + dy);
            if wx < map.width && wy < map.height {
                let idx = map.xy_idx(wx, wy);
                if map.tiles[idx] == TileType::Wall {
                    cut[idx] = true;
                }
            }
        }
    };
    for (idx, _) in map.visible_tiles.iter().enumerate().filter(|(_, v)| **v) {
        if map.tiles[idx] != TileType::Wall {
            mark((idx as i32 % map.width, idx as i32 / map.width));
        }
    }
    for &member in party {
        mark(member);
    }
    cut
}

/// Finds town buildings: 4-connected `WoodFloor` regions closed off by walls
/// and doorway floors. A building owns its floor plus the walls and doorways
/// around it. Market stalls (open to the street) are not buildings.
fn find_buildings(map: &Map) -> Buildings {
    let (w, h) = (map.width, map.height);
    let tile = |x: i32, y: i32| (x >= 0 && y >= 0 && x < w && y < h).then(|| map.tiles[map.xy_idx(x, y)]);
    let mut of_tile = vec![None; map.tiles.len()];
    let mut seen = vec![false; map.tiles.len()];
    let mut count = 0;
    for start in 0..map.tiles.len() {
        if seen[start] || map.tiles[start] != TileType::WoodFloor {
            continue;
        }
        let mut region = vec![start];
        let mut closed = true;
        seen[start] = true;
        let mut i = 0;
        while i < region.len() {
            let (x, y) = (region[i] as i32 % w, region[i] as i32 / w);
            for (nx, ny) in [(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)] {
                match tile(nx, ny) {
                    Some(TileType::WoodFloor) => {
                        let n = map.xy_idx(nx, ny);
                        if !seen[n] {
                            seen[n] = true;
                            region.push(n);
                        }
                    }
                    Some(TileType::Wall | TileType::Floor) => {}
                    _ => closed = false,
                }
            }
            i += 1;
        }
        if !closed {
            continue;
        }
        for &idx in &region {
            let (x, y) = (idx as i32 % w, idx as i32 / w);
            for dy in -1..=1 {
                for dx in -1..=1 {
                    if tile(x + dx, y + dy).is_some() {
                        of_tile[map.xy_idx(x + dx, y + dy)] = Some(count);
                    }
                }
            }
        }
        count += 1;
    }
    Buildings { of_tile, count }
}

/// Gives every non-wall building tile a roof slab when a map's tiles spawn.
fn spawn_roofs(
    mut commands: Commands,
    map: Res<Map>,
    added: Query<(Entity, &TileMesh, &Position), Added<TileMesh>>,
    mut buildings: ResMut<Buildings>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut assets: Local<Option<(Handle<Mesh>, Handle<StandardMaterial>)>>,
) {
    if added.is_empty() {
        return;
    }
    *buildings = find_buildings(&map);
    let (mesh, material) = assets
        .get_or_insert_with(|| {
            (
                meshes.add(Cuboid::new(1.0, ROOF_THICKNESS, 1.0)),
                materials.add(StandardMaterial {
                    base_color: Color::srgb(0.4, 0.2, 0.15),
                    perceptual_roughness: 1.0,
                    ..default()
                }),
            )
        })
        .clone();
    for (entity, TileMesh(tile), pos) in &added {
        let Some(Some(b)) = buildings.of_tile.get(map.xy_idx(pos.x, pos.y)) else {
            continue;
        };
        if *tile == TileType::Wall {
            continue;
        }
        // A child of a floor tile, so it is despawned with the map.
        commands.entity(entity).with_child((
            Roof(*b),
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material.clone()),
            Transform::from_xyz(0.0, WALL_HEIGHT + ROOF_THICKNESS / 2.0, 0.0),
            Visibility::Hidden,
        ));
    }
}

/// Shows or hides roofs and cuts walls down to stubs, every frame.
fn apply_occlusion(
    map: Res<Map>,
    buildings: Res<Buildings>,
    party: Query<&Position, With<PartyMember>>,
    mut tiles: Query<(&TileMesh, &Position, &Revealed, &mut Transform)>,
    mut roofs: Query<(&Roof, &mut Visibility)>,
) {
    let party: Vec<(i32, i32)> = party.iter().map(|pos| (pos.x, pos.y)).collect();
    if party.is_empty() || buildings.of_tile.len() != map.tiles.len() {
        return;
    }
    let building_at = |x: i32, y: i32| {
        (x < map.width && y < map.height)
            .then(|| buildings.of_tile[map.xy_idx(x, y)])
            .flatten()
    };

    // A roof shows once any of its building is seen, unless a member is inside or behind it.
    let mut roof_shown = vec![false; buildings.count];
    for (TileMesh(_), pos, revealed, _) in &tiles {
        if let Some(b) = building_at(pos.x, pos.y) {
            roof_shown[b] |= !matches!(revealed.0, RevealedState::Hidden);
        }
    }
    let over_party = party.iter().flat_map(|&(px, py)| {
        std::iter::once((0, 0))
            .chain(OCCLUDER_OFFSETS)
            .filter_map(move |(dx, dy)| building_at(px + dx, py + dy))
    });
    for b in over_party {
        roof_shown[b] = false;
    }
    for (Roof(b), mut visibility) in &mut roofs {
        let want = if roof_shown[*b] { Visibility::Visible } else { Visibility::Hidden };
        visibility.set_if_neq(want);
    }

    let cut = walls_to_cut(&map, &party);
    for (TileMesh(tile), pos, _, mut transform) in &mut tiles {
        if *tile != TileType::Wall {
            continue;
        }
        let under_roof = building_at(pos.x, pos.y).is_some_and(|b| roof_shown[b]);
        let height = if cut[map.xy_idx(pos.x, pos.y)] && !under_roof {
            STUB_HEIGHT
        } else {
            WALL_HEIGHT
        };
        let mut wall = *transform;
        wall.translation.y = height / 2.0;
        wall.scale.y = height / WALL_HEIGHT;
        transform.set_if_neq(wall);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 7 by 7 map: walls round a 5 by 5 room, everything in view.
    fn room() -> Map {
        let mut map = Map::new(7, 7, 1);
        for y in 1..6 {
            for x in 1..6 {
                let idx = map.xy_idx(x, y);
                map.tiles[idx] = TileType::Floor;
            }
        }
        map.visible_tiles = vec![true; 49];
        map
    }

    fn cut_cells(map: &Map, party: &[(i32, i32)]) -> Vec<(i32, i32)> {
        walls_to_cut(map, party)
            .iter()
            .enumerate()
            .filter(|(_, c)| **c)
            .map(|(i, _)| (i as i32 % map.width, i as i32 / map.width))
            .collect()
    }

    #[test]
    fn only_the_near_walls_of_a_room_are_cut() {
        // The camera sits at +x, +z: the south (y = 6) and east (x = 6) walls
        // face it, the north and west walls stay full height.
        let cut = cut_cells(&room(), &[(3, 3)]);
        let mut near: Vec<_> = (1..7).map(|x| (x, 6)).chain((1..6).map(|y| (6, y))).collect();
        near.sort_by_key(|&(x, y)| (y, x));
        assert_eq!(cut, near);
    }

    #[test]
    fn each_member_alone_cuts_the_walls_that_hide_it() {
        let mut map = room();
        map.visible_tiles = vec![false; 49];
        assert_eq!(cut_cells(&map, &[(5, 5)]), vec![(6, 5), (5, 6), (6, 6)]);
        assert_eq!(cut_cells(&map, &[(5, 1), (5, 5)]), vec![(6, 1), (6, 2), (6, 3), (6, 5), (5, 6), (6, 6)]);
    }

    #[test]
    fn closed_wood_rooms_are_buildings_and_stalls_are_not() {
        // A 4 by 4 building with a doorway at (1, 0), and a stall on the grass.
        let mut map = Map::new(8, 6, 1);
        map.tiles = vec![TileType::Grass; 48];
        for y in 0..4 {
            for x in 0..4 {
                let edge = x == 0 || y == 0 || x == 3 || y == 3;
                let idx = map.xy_idx(x, y);
                map.tiles[idx] = if edge { TileType::Wall } else { TileType::WoodFloor };
            }
        }
        let door = map.xy_idx(1, 0);
        map.tiles[door] = TileType::Floor;
        let stall = map.xy_idx(6, 4);
        map.tiles[stall] = TileType::WoodFloor;

        let buildings = find_buildings(&map);
        assert_eq!(buildings.count, 1);
        let owned: Vec<usize> = (0..48).filter(|&i| buildings.of_tile[i] == Some(0)).collect();
        let footprint: Vec<usize> = (0..4).flat_map(|y| (0..4).map(move |x| y * 8 + x)).collect();
        assert_eq!(owned, footprint);
    }
}
