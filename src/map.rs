use std::cmp::{max, min};

use bevy::prelude::*;
use rand::prelude::*;

use crate::components::RenderOrder;
use crate::distance::DistanceAlg;
use crate::player::Player;
use crate::resources::UiFont;
use crate::shapes::Rect;
use crate::viewshed::Viewshed;

pub const FONT_SIZE: f32 = 16.;
pub const MAP_HEIGHT: usize = 43;
pub const MAP_WIDTH: usize = 80;
pub const GRID_PX: Vec2 = Vec2 {
    x: FONT_SIZE * 1.,
    y: FONT_SIZE * 1.,
};

/// Grid based position
#[derive(Component, Clone, Copy)]
pub struct Position {
    pub x: i32,
    pub y: i32,
}

#[derive(Component)]
pub struct Tile;

#[derive(PartialEq, Copy, Clone, Debug)]
pub enum TileType {
    Floor,
    Wall,
}

#[derive(Debug)]
pub enum RevealedState {
    Explored,
    Hidden,
    Visible,
}

#[derive(Component)]
pub struct Revealed(pub RevealedState);

#[derive(Resource)]
pub struct Map {
    pub rooms: Vec<Rect>,
    pub tiles: Vec<TileType>,
    pub width: i32,
    pub height: i32,
    pub revealed_tiles: Vec<bool>,
    pub visible_tiles: Vec<bool>,
    pub blocked_tiles: Vec<bool>,
    pub tile_content: Vec<Vec<Entity>>,
}

impl Map {
    pub fn xy_idx(&self, x: i32, y: i32) -> usize {
        (y as usize * self.width as usize) + x as usize
    }

    fn apply_room_to_map(&mut self, room: &Rect) {
        for y in room.y1 + 1..=room.y2 {
            for x in room.x1 + 1..=room.x2 {
                let idx = self.xy_idx(x, y);
                self.tiles[idx] = TileType::Floor;
            }
        }
    }

    fn apply_horizontal_tunnel(&mut self, x1: i32, x2: i32, y: i32) {
        for x in min(x1, x2)..=max(x1, x2) {
            let idx = xy_idx(x, y);
            if idx > 0 && idx < MAP_WIDTH * MAP_HEIGHT {
                self.tiles[idx] = TileType::Floor;
            }
        }
    }

    fn apply_vertical_tunnel(&mut self, y1: i32, y2: i32, x: i32) {
        for y in min(y1, y2)..=max(y1, y2) {
            let idx = xy_idx(x, y);
            if idx > 0 && idx < MAP_WIDTH * MAP_HEIGHT {
                self.tiles[idx] = TileType::Floor;
            }
        }
    }

    pub fn is_exit_valid(&self, x: i32, y: i32) -> bool {
        if x < 1 || x > self.width - 1 || y < 1 || y > self.height - 1 {
            return false;
        }
        let idx = self.xy_idx(x, y);

        !self.blocked_tiles[idx]
    }

    /// Check if a tile is walkable (ignores entities, only checks walls)
    fn is_walkable(&self, x: i32, y: i32) -> bool {
        if x < 1 || x > self.width - 1 || y < 1 || y > self.height - 1 {
            return false;
        }
        let idx = self.xy_idx(x, y);

        self.tiles[idx] != TileType::Wall
    }

    /// Get available exits ignoring entity blocking (for pathfinding)
    pub fn get_available_exits_ignoring_entities(&self, idx: usize) -> Vec<(usize, f32)> {
        let mut exits = Vec::new();
        let x = idx as i32 % self.width;
        let y = idx as i32 / self.width;
        let w = self.width as usize;

        // Cardinal directions
        if self.is_walkable(x - 1, y) {
            exits.push((idx - 1, 1.0))
        };
        if self.is_walkable(x + 1, y) {
            exits.push((idx + 1, 1.0))
        };
        if self.is_walkable(x, y - 1) {
            exits.push((idx - w, 1.0))
        };
        if self.is_walkable(x, y + 1) {
            exits.push((idx + w, 1.0))
        };

        // Diagonal directions
        if self.is_walkable(x - 1, y - 1) {
            exits.push(((idx - w) - 1, 1.45))
        };
        if self.is_walkable(x + 1, y - 1) {
            exits.push(((idx - w) + 1, 1.45))
        };
        if self.is_walkable(x - 1, y + 1) {
            exits.push(((idx + w) - 1, 1.45))
        };
        if self.is_walkable(x + 1, y + 1) {
            exits.push(((idx + w) + 1, 1.45))
        };

        exits
    }

    pub fn get_available_exits(&self, idx: usize) -> Vec<(usize, f32)> {
        let mut exits = Vec::new();
        let x = idx as i32 % self.width;
        let y = idx as i32 / self.width;
        let w = self.width as usize;

        // Cardinal directions
        if self.is_exit_valid(x - 1, y) {
            exits.push((idx - 1, 1.0))
        };
        if self.is_exit_valid(x + 1, y) {
            exits.push((idx + 1, 1.0))
        };
        if self.is_exit_valid(x, y - 1) {
            exits.push((idx - w, 1.0))
        };
        if self.is_exit_valid(x, y + 1) {
            exits.push((idx + w, 1.0))
        };

        // Diagonal directions
        if self.is_exit_valid(x - 1, y - 1) {
            exits.push(((idx - w) - 1, 1.45))
        };
        if self.is_exit_valid(x + 1, y - 1) {
            exits.push(((idx - w) + 1, 1.45))
        };
        if self.is_exit_valid(x - 1, y + 1) {
            exits.push(((idx + w) - 1, 1.45))
        };
        if self.is_exit_valid(x + 1, y + 1) {
            exits.push(((idx + w) + 1, 1.45))
        };

        exits
    }

    pub fn get_pathing_distance(&self, idx1: usize, idx2: usize) -> f32 {
        let w = self.width as usize;
        let p1 = Vec2::new((idx1 % w) as f32, (idx1 / w) as f32);
        let p2 = Vec2::new((idx2 % w) as f32, (idx2 / w) as f32);

        DistanceAlg::Chebyshev.distance2d(p1, p2)
    }

    pub fn populate_blocked(&mut self) {
        for (i, tile) in self.tiles.iter().enumerate() {
            self.blocked_tiles[i] = *tile == TileType::Wall;
        }
    }

    pub fn clear_content_index(&mut self) {
        for content in self.tile_content.iter_mut() {
            content.clear();
        }
    }

    pub fn new_map_rooms_and_corridors() -> Map {
        let size = MAP_WIDTH * MAP_HEIGHT;
        let mut map = Map {
            rooms: Vec::new(),
            tiles: vec![TileType::Wall; size],
            width: MAP_WIDTH as i32,
            height: MAP_HEIGHT as i32,
            revealed_tiles: vec![false; size],
            visible_tiles: vec![false; size],
            blocked_tiles: vec![false; size],
            tile_content: vec![Vec::new(); size],
        };

        const MAX_ROOMS: i32 = 30;
        const MIN_SIZE: i32 = 6;
        const MAX_SIZE: i32 = 10;

        let mut rng = rand::thread_rng();

        for _ in 0..MAX_ROOMS {
            let w = rng.gen_range(MIN_SIZE..=MAX_SIZE);
            let h = rng.gen_range(MIN_SIZE..=MAX_SIZE);
            let x_roll = map.width - w - 1;
            let y_roll = map.height - h - 1;
            let x = rng.gen_range(0..x_roll);
            let y = rng.gen_range(0..y_roll);
            let new_room = Rect::new(x, y, w, h);
            let mut ok = true;
            for other_room in map.rooms.iter() {
                if new_room.intersect(other_room) {
                    ok = false
                }
            }
            if ok {
                map.apply_room_to_map(&new_room);

                if !map.rooms.is_empty() {
                    let (new_x, new_y) = new_room.center();
                    let (prev_x, prev_y) = map.rooms[map.rooms.len() - 1].center();
                    if rng.gen_range(0..2) == 1 {
                        map.apply_horizontal_tunnel(prev_x, new_x, prev_y);
                        map.apply_vertical_tunnel(prev_y, new_y, prev_x);
                    } else {
                        map.apply_vertical_tunnel(prev_y, new_y, prev_x);
                        map.apply_horizontal_tunnel(prev_x, new_x, prev_y);
                    }
                }

                map.rooms.push(new_room);
            }
        }

        map
    }
}

impl Default for Map {
    fn default() -> Self {
        Map::new_map_rooms_and_corridors()
    }
}

pub struct MapPlugin;
impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Map>()
            .add_systems(Startup, draw_map)
            .add_systems(
                Update,
                (
                    translate_positions,
                    update_revealed_state,
                    update_revealed_tiles,
                    update_visible_tiles,
                    update_renderable_visibility,
                ),
            );
    }
}

pub fn xy_idx(x: i32, y: i32) -> usize {
    (y as usize * MAP_WIDTH) + x as usize
}

/// Makes a map with solid boundaries and 400 randomly placed walls. No guarantees that it won't look awful.
fn new_map_test() -> Vec<TileType> {
    let width = MAP_WIDTH as i32;
    let height = MAP_HEIGHT as i32;
    let mut map = vec![TileType::Floor; MAP_WIDTH * MAP_HEIGHT];

    // Make the boundary walls
    for x in 0..width {
        map[xy_idx(x, 0)] = TileType::Wall;
        map[xy_idx(x, height - 1)] = TileType::Wall;
    }
    for y in 0..height {
        map[xy_idx(0, y)] = TileType::Wall;
        map[xy_idx(width - 1, y)] = TileType::Wall;
    }

    // Now we'll randomly splat a bunch of walls. It won't be pretty, but it's a decent illustration.
    // First, obtain the thread-local RNG:
    let mut rng = rand::thread_rng();

    for _i in 0..400 {
        let x = rng.gen_range(1..width);
        let y = rng.gen_range(1..height);
        let idx = xy_idx(x, y);
        if idx != xy_idx(40, 25) {
            map[idx] = TileType::Wall;
        }
    }

    map
}

fn draw_map(mut commands: Commands, map: Res<Map>, font: Res<UiFont>) {
    let text_font = TextFont {
        font: font.0.clone(),
        font_size: FONT_SIZE,
        ..default()
    };

    let mut y = 0;
    let mut x = 0;
    for tile in map.tiles.iter() {
        // Render a tile depending upon the tile type
        match tile {
            TileType::Floor => {
                commands.spawn((
                    Tile,
                    Position { x, y },
                    Text2d::new("."),
                    text_font.clone(),
                    TextColor(Color::srgb(0.5, 0.5, 0.5)),
                    Revealed(RevealedState::Hidden),
                ));
            }
            TileType::Wall => {
                commands.spawn((
                    Tile,
                    Position { x, y },
                    Text2d::new("#"),
                    text_font.clone(),
                    TextColor(Color::srgb(0.0, 1.0, 0.0)),
                    Revealed(RevealedState::Hidden),
                ));
            }
        }

        x += 1;
        if x > MAP_WIDTH as i32 - 1 {
            x = 0;
            y += 1;
        }
    }
}

fn update_revealed_tiles(
    mut map: ResMut<Map>,
    query: Single<&Viewshed, With<Player>>,
    mut tiles_query: Query<(&Position, &mut Revealed), With<Tile>>,
) {
    let viewshed = &query.into_inner();

    for (pos, mut revealed) in &mut tiles_query {
        let idx = map.xy_idx(pos.x, pos.y);
        let point = (pos.x, pos.y);
        if viewshed.visible_tiles.contains(&point) {
            map.revealed_tiles[idx] = true;
            revealed.0 = RevealedState::Visible;
        } else if matches!(revealed.0, RevealedState::Visible) {
            revealed.0 = RevealedState::Explored;
        }
    }
}

fn update_revealed_state(mut tiles_query: Query<(&mut TextColor, &Revealed), With<Tile>>) {
    for (mut text_color, revealed) in &mut tiles_query {
        match revealed.0 {
            RevealedState::Explored => {
                text_color.0.set_alpha(0.1);
            }
            RevealedState::Visible => {
                text_color.0.set_alpha(1.0);
            }
            RevealedState::Hidden => {
                text_color.0.set_alpha(0.0);
            }
        }
    }
}

fn translate_positions(
    mut commands: Commands,
    window: Single<&Window>,
    query: Query<(Entity, &Position, Option<&RenderOrder>)>,
) {
    let half_height = window.height() / 2.;
    let half_width = window.width() / 2.;
    for (entity, position, render_order) in &query {
        let z = render_order.map(|r| r.0 as f32 * 0.1).unwrap_or(0.0);
        // Map position coords to pixel coords. Y runs in the opposite direction to the pixel
        // coords.
        commands.entity(entity).insert(Transform::from_xyz(
            (position.x as f32) * GRID_PX.x + (GRID_PX.x / 2.) - half_width,
            (position.y as f32) * -GRID_PX.y - (GRID_PX.y / 2.) + half_height,
            z,
        ));
    }
}

fn update_visible_tiles(mut map: ResMut<Map>, player: Single<&Viewshed, With<Player>>) {
    let viewshed = player.into_inner();
    map.visible_tiles = vec![false; MAP_WIDTH * MAP_HEIGHT];
    for (pos_x, pos_y) in viewshed.visible_tiles.iter() {
        let idx = map.xy_idx(*pos_x, *pos_y);
        map.visible_tiles[idx] = true;
    }
}

fn update_renderable_visibility(
    map: Res<Map>,
    mut query: Query<(&Position, &mut Visibility), (With<RenderOrder>, Without<Player>)>,
) {
    for (pos, mut visibility) in &mut query {
        let idx = map.xy_idx(pos.x, pos.y);
        if map.visible_tiles[idx] {
            *visibility = Visibility::Visible;
        } else {
            *visibility = Visibility::Hidden;
        }
    }
}
