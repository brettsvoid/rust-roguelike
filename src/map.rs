use std::cmp::{max, min};
use std::collections::HashSet;

use bevy::prelude::*;
use rand::prelude::*;

use crate::components::RenderOrder;
use crate::debug::DebugState;
use crate::distance::DistanceAlg;
use crate::player::Player;
use crate::resources::UiFont;
use crate::shapes::Rect;
use crate::viewshed::Viewshed;
use crate::RunState;

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

/// Wall glyph based on 4-bit bitmask of cardinal neighbors (N=1, S=2, W=4, E=8)
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct WallGlyph(pub u8);

impl WallGlyph {
    /// Calculate wall glyph based on neighboring walls
    pub fn from_neighbors(north: bool, south: bool, west: bool, east: bool) -> Self {
        let mut mask = 0u8;
        if north {
            mask |= 1;
        }
        if south {
            mask |= 2;
        }
        if west {
            mask |= 4;
        }
        if east {
            mask |= 8;
        }
        WallGlyph(mask)
    }

    /// Get the box-drawing character for this wall configuration (CP437-style)
    pub fn to_char(&self) -> char {
        match self.0 {
            0 => '○',  // Pillar (no neighbors)
            1 => '│',  // N only
            2 => '│',  // S only
            3 => '│',  // N+S (vertical)
            4 => '─',  // W only
            5 => '┘',  // N+W (bottom-right corner)
            6 => '┐',  // S+W (top-right corner)
            7 => '┤',  // N+S+W (right T)
            8 => '─',  // E only
            9 => '└',  // N+E (bottom-left corner)
            10 => '┌', // S+E (top-left corner)
            11 => '├', // N+S+E (left T)
            12 => '─', // W+E (horizontal)
            13 => '┴', // N+W+E (bottom T)
            14 => '┬', // S+W+E (top T)
            15 => '┼', // All 4 (cross)
            _ => '#',
        }
    }
}

#[derive(PartialEq, Copy, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum TileType {
    Floor,
    Wall,
    DownStairs,
}

#[derive(Debug)]
pub enum RevealedState {
    Explored,
    Hidden,
    Visible,
}

#[derive(Component)]
pub struct Revealed(pub RevealedState);

#[derive(Component)]
pub struct BloodstainMarker;

#[derive(Resource)]
pub struct Map {
    pub rooms: Vec<Rect>,
    pub tiles: Vec<TileType>,
    pub width: i32,
    pub height: i32,
    pub depth: i32,
    pub revealed_tiles: Vec<bool>,
    pub visible_tiles: Vec<bool>,
    pub blocked_tiles: Vec<bool>,
    pub tile_content: Vec<Vec<Entity>>,
    pub bloodstains: HashSet<usize>,
}

impl Map {
    pub fn xy_idx(&self, x: i32, y: i32) -> usize {
        (y as usize * self.width as usize) + x as usize
    }

    /// Check if a wall at (x, y) is adjacent to at least one floor tile (including diagonals)
    pub fn is_adjacent_to_floor(&self, x: i32, y: i32) -> bool {
        let check = |tx: i32, ty: i32| -> bool {
            if tx < 0 || tx >= self.width || ty < 0 || ty >= self.height {
                return false;
            }
            self.tiles[self.xy_idx(tx, ty)] == TileType::Floor
        };
        // Cardinal directions
        check(x, y - 1) || check(x, y + 1) || check(x - 1, y) || check(x + 1, y) ||
        // Diagonal directions (for corners)
        check(x - 1, y - 1) || check(x + 1, y - 1) || check(x - 1, y + 1) || check(x + 1, y + 1)
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

    /// Calculate wall glyph for a given position based on neighbors
    pub fn wall_glyph_at(&self, x: i32, y: i32) -> WallGlyph {
        // Check if a tile is a floor
        let is_floor_at = |tx: i32, ty: i32| -> bool {
            if tx < 0 || tx >= self.width || ty < 0 || ty >= self.height {
                return false;
            }
            self.tiles[self.xy_idx(tx, ty)] == TileType::Floor
        };

        // Check if a wall at (nx, ny) is a "boundary wall" (adjacent to at least one floor, including diagonals)
        let is_boundary_wall = |nx: i32, ny: i32| -> bool {
            if nx < 0 || nx >= self.width || ny < 0 || ny >= self.height {
                return false;
            }
            if self.tiles[self.xy_idx(nx, ny)] != TileType::Wall {
                return false;
            }
            // Check if this wall is adjacent to any floor (cardinal + diagonal)
            is_floor_at(nx, ny - 1)
                || is_floor_at(nx, ny + 1)
                || is_floor_at(nx - 1, ny)
                || is_floor_at(nx + 1, ny)
                || is_floor_at(nx - 1, ny - 1)
                || is_floor_at(nx + 1, ny - 1)
                || is_floor_at(nx - 1, ny + 1)
                || is_floor_at(nx + 1, ny + 1)
        };

        // Draw wall segments only toward boundary walls (walls next to floors)
        let mut mask = 0u8;
        if is_boundary_wall(x, y - 1) {
            mask |= 1;
        } // North
        if is_boundary_wall(x, y + 1) {
            mask |= 2;
        } // South
        if is_boundary_wall(x - 1, y) {
            mask |= 4;
        } // West
        if is_boundary_wall(x + 1, y) {
            mask |= 8;
        } // East

        WallGlyph(mask)
    }

    pub fn new_map_rooms_and_corridors() -> Map {
        let size = MAP_WIDTH * MAP_HEIGHT;
        let mut map = Map {
            rooms: Vec::new(),
            tiles: vec![TileType::Wall; size],
            width: MAP_WIDTH as i32,
            height: MAP_HEIGHT as i32,
            depth: 1,
            revealed_tiles: vec![false; size],
            visible_tiles: vec![false; size],
            blocked_tiles: vec![false; size],
            tile_content: vec![Vec::new(); size],
            bloodstains: HashSet::new(),
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

        // Place down stairs in the center of the last room
        if let Some(last_room) = map.rooms.last() {
            let (stairs_x, stairs_y) = last_room.center();
            let stairs_idx = map.xy_idx(stairs_x, stairs_y);
            map.tiles[stairs_idx] = TileType::DownStairs;
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
        // Run condition: only when player exists (not in MainMenu or GameOver)
        let has_player = not(in_state(RunState::MainMenu)).and(not(in_state(RunState::GameOver)));

        app.init_resource::<Map>().add_systems(
            Update,
            (
                translate_positions,
                update_revealed_state,
                update_revealed_tiles.run_if(has_player.clone()),
                update_visible_tiles.run_if(has_player.clone()),
                update_renderable_visibility,
                update_bloodstains,
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
                // Only spawn walls adjacent to floors (boundary walls)
                if map.is_adjacent_to_floor(x, y) {
                    let glyph = map.wall_glyph_at(x, y);
                    commands.spawn((
                        Tile,
                        Position { x, y },
                        glyph,
                        Text2d::new(glyph.to_char().to_string()),
                        text_font.clone(),
                        TextColor(Color::srgb(0.0, 1.0, 0.0)),
                        Revealed(RevealedState::Hidden),
                    ));
                }
            }
            TileType::DownStairs => {
                commands.spawn((
                    Tile,
                    Position { x, y },
                    Text2d::new(">"),
                    text_font.clone(),
                    TextColor(Color::srgb(0.0, 1.0, 1.0)),
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
    debug_state: Res<DebugState>,
    query: Query<&Viewshed, With<Player>>,
    mut tiles_query: Query<(&Position, &mut Revealed), With<Tile>>,
) {
    let Ok(viewshed) = query.get_single() else {
        return;
    };

    for (pos, mut revealed) in &mut tiles_query {
        let idx = map.xy_idx(pos.x, pos.y);
        let point = (pos.x, pos.y);
        if viewshed.visible_tiles.contains(&point) {
            map.revealed_tiles[idx] = true;
            revealed.0 = RevealedState::Visible;
        } else if debug_state.no_fog && map.revealed_tiles[idx] {
            // No fog mode: keep revealed tiles fully visible
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
        // coords. Use try_insert to handle entities that may be despawned during load.
        commands.entity(entity).try_insert(Transform::from_xyz(
            (position.x as f32) * GRID_PX.x + (GRID_PX.x / 2.) - half_width,
            (position.y as f32) * -GRID_PX.y - (GRID_PX.y / 2.) + half_height,
            z,
        ));
    }
}

fn update_visible_tiles(
    mut map: ResMut<Map>,
    debug_state: Res<DebugState>,
    player: Query<&Viewshed, With<Player>>,
) {
    let Ok(viewshed) = player.get_single() else {
        return;
    };
    map.visible_tiles = vec![false; MAP_WIDTH * MAP_HEIGHT];

    if debug_state.no_fog {
        // No fog mode: all revealed tiles are visible
        for idx in 0..map.revealed_tiles.len() {
            if map.revealed_tiles[idx] {
                map.visible_tiles[idx] = true;
            }
        }
    } else {
        // Normal mode: only viewshed tiles are visible
        for (pos_x, pos_y) in viewshed.visible_tiles.iter() {
            let idx = map.xy_idx(*pos_x, *pos_y);
            map.visible_tiles[idx] = true;
        }
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

fn update_bloodstains(
    mut commands: Commands,
    map: Res<Map>,
    window: Query<&Window>,
    existing: Query<Entity, With<BloodstainMarker>>,
) {
    // Despawn existing bloodstain sprites
    for entity in &existing {
        commands.entity(entity).despawn();
    }

    let Ok(window) = window.get_single() else {
        return;
    };

    let half_width = window.width() / 2.0;
    let half_height = window.height() / 2.0;

    // Spawn bloodstain sprites for visible bloody tiles
    for &idx in &map.bloodstains {
        if map.visible_tiles[idx] {
            let x = (idx % MAP_WIDTH) as i32;
            let y = (idx / MAP_WIDTH) as i32;

            let screen_x = (x as f32) * GRID_PX.x + (GRID_PX.x / 2.0) - half_width;
            let screen_y = (y as f32) * -GRID_PX.y - (GRID_PX.y / 2.0) + half_height;

            commands.spawn((
                BloodstainMarker,
                Sprite {
                    color: Color::srgba(0.75, 0.0, 0.0, 0.25),
                    custom_size: Some(Vec2::new(GRID_PX.x, GRID_PX.y)),
                    ..default()
                },
                Transform::from_xyz(screen_x, screen_y, 0.5), // Just above floor
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Creates a small test map with given dimensions and all walls
    fn create_test_map(width: i32, height: i32) -> Map {
        let size = (width * height) as usize;
        Map {
            rooms: Vec::new(),
            tiles: vec![TileType::Wall; size],
            width,
            height,
            depth: 1,
            revealed_tiles: vec![false; size],
            visible_tiles: vec![false; size],
            blocked_tiles: vec![false; size],
            tile_content: vec![Vec::new(); size],
            bloodstains: HashSet::new(),
        }
    }

    #[test]
    fn test_wall_glyph_isolated_wall() {
        // 3x3 map with single wall in center, floors around it
        // . . .
        // . # .
        // . . .
        let mut map = create_test_map(3, 3);
        // Set all tiles to floor except center
        for i in 0..9 {
            map.tiles[i] = TileType::Floor;
        }
        map.tiles[4] = TileType::Wall; // Center tile (1,1)

        let glyph = map.wall_glyph_at(1, 1);
        // All neighbors are floors, so mask = 0 (pillar)
        assert_eq!(
            glyph.0, 0,
            "Isolated wall should have mask 0, got {}",
            glyph.0
        );
        assert_eq!(glyph.to_char(), '○', "Isolated wall should be pillar");
    }

    #[test]
    fn test_wall_glyph_vertical_wall() {
        // 3x3 map with vertical wall in center column
        // . # .
        // . # .
        // . # .
        let mut map = create_test_map(3, 3);
        for i in 0..9 {
            map.tiles[i] = TileType::Floor;
        }
        map.tiles[1] = TileType::Wall; // (1,0)
        map.tiles[4] = TileType::Wall; // (1,1)
        map.tiles[7] = TileType::Wall; // (1,2)

        let glyph = map.wall_glyph_at(1, 1);
        // N=wall (not floor), S=wall (not floor), W=floor, E=floor
        // mask = N(1) + S(2) = 3
        assert_eq!(
            glyph.0, 3,
            "Vertical wall should have mask 3 (N+S), got {}",
            glyph.0
        );
        assert_eq!(glyph.to_char(), '│', "Vertical wall should be │");
    }

    #[test]
    fn test_wall_glyph_horizontal_wall() {
        // 3x3 map with horizontal wall in center row
        // . . .
        // # # #
        // . . .
        let mut map = create_test_map(3, 3);
        for i in 0..9 {
            map.tiles[i] = TileType::Floor;
        }
        map.tiles[3] = TileType::Wall; // (0,1)
        map.tiles[4] = TileType::Wall; // (1,1)
        map.tiles[5] = TileType::Wall; // (2,1)

        let glyph = map.wall_glyph_at(1, 1);
        // N=floor, S=floor, W=wall (not floor), E=wall (not floor)
        // mask = W(4) + E(8) = 12
        assert_eq!(
            glyph.0, 12,
            "Horizontal wall should have mask 12 (W+E), got {}",
            glyph.0
        );
        assert_eq!(glyph.to_char(), '─', "Horizontal wall should be ─");
    }

    #[test]
    fn test_wall_glyph_corner_top_left() {
        // 3x3 map: top-left corner of a room
        // # # .
        // # . .
        // . . .
        let mut map = create_test_map(3, 3);
        for i in 0..9 {
            map.tiles[i] = TileType::Floor;
        }
        map.tiles[0] = TileType::Wall; // (0,0)
        map.tiles[1] = TileType::Wall; // (1,0)
        map.tiles[3] = TileType::Wall; // (0,1)

        // Test the corner wall at (0,0)
        let glyph = map.wall_glyph_at(0, 0);
        // N=out of bounds (false), S=wall, W=out of bounds (false), E=wall
        // mask = S(2) + E(8) = 10
        assert_eq!(
            glyph.0, 10,
            "Corner at (0,0) should have mask 10 (S+E), got {}",
            glyph.0
        );
        assert_eq!(glyph.to_char(), '┌', "Top-left corner should be ┌");
    }

    #[test]
    fn test_wall_glyph_t_junction() {
        // 3x3 map: T-junction
        // . # .
        // # # #
        // . . .
        let mut map = create_test_map(3, 3);
        for i in 0..9 {
            map.tiles[i] = TileType::Floor;
        }
        map.tiles[1] = TileType::Wall; // (1,0) - top
        map.tiles[3] = TileType::Wall; // (0,1) - left
        map.tiles[4] = TileType::Wall; // (1,1) - center
        map.tiles[5] = TileType::Wall; // (2,1) - right

        let glyph = map.wall_glyph_at(1, 1);
        // N=wall, S=floor, W=wall, E=wall
        // mask = N(1) + W(4) + E(8) = 13
        assert_eq!(
            glyph.0, 13,
            "T-junction should have mask 13 (N+W+E), got {}",
            glyph.0
        );
        assert_eq!(glyph.to_char(), '┴', "T-junction opening south should be ┴");
    }

    #[test]
    fn test_wall_glyph_cross() {
        // 3x3 map: cross/plus shape
        // . # .
        // # # #
        // . # .
        let mut map = create_test_map(3, 3);
        for i in 0..9 {
            map.tiles[i] = TileType::Floor;
        }
        map.tiles[1] = TileType::Wall; // (1,0)
        map.tiles[3] = TileType::Wall; // (0,1)
        map.tiles[4] = TileType::Wall; // (1,1)
        map.tiles[5] = TileType::Wall; // (2,1)
        map.tiles[7] = TileType::Wall; // (1,2)

        let glyph = map.wall_glyph_at(1, 1);
        // All 4 neighbors are walls (not floors)
        // mask = N(1) + S(2) + W(4) + E(8) = 15
        assert_eq!(
            glyph.0, 15,
            "Cross should have mask 15 (all), got {}",
            glyph.0
        );
        assert_eq!(glyph.to_char(), '┼', "Cross should be ┼");
    }

    #[test]
    fn test_wall_glyph_horizontal_cap() {
        // 3x3 map: horizontal cap
        // . . .
        // . # .
        // . # .
        let mut map = create_test_map(3, 3);
        for i in 0..9 {
            map.tiles[i] = TileType::Floor;
        }
        map.tiles[1] = TileType::Wall; // (1,0)
        map.tiles[3] = TileType::Wall; // (0,1)
        map.tiles[4] = TileType::Wall; // (1,1)
        map.tiles[5] = TileType::Wall; // (2,1)
        map.tiles[7] = TileType::Wall; // (1,2)

        let glyph = map.wall_glyph_at(1, 1);
        // All 4 neighbors are walls (not floors)
        // mask = N(1) + S(2) + W(4) + E(8) = 15
        assert_eq!(
            glyph.0, 15,
            "Cross should have mask 15 (all), got {}",
            glyph.0
        );
        assert_eq!(glyph.to_char(), '┼', "Cross should be ┼");
    }
}
