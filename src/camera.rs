use bevy::prelude::*;

use crate::map::Position;
use crate::player::Player;

// Screen dimensions in tiles (viewport)
pub const SCREEN_WIDTH: i32 = 80;
pub const SCREEN_HEIGHT: i32 = 43;

#[derive(Resource, Default)]
pub struct Camera {
    pub x: i32,
    pub y: i32,
}

impl Camera {
    /// Get visible screen bounds based on camera position
    /// Returns (min_x, max_x, min_y, max_y)
    pub fn get_screen_bounds(&self) -> (i32, i32, i32, i32) {
        let min_x = self.x - (SCREEN_WIDTH / 2);
        let max_x = min_x + SCREEN_WIDTH;
        let min_y = self.y - (SCREEN_HEIGHT / 2);
        let max_y = min_y + SCREEN_HEIGHT;
        (min_x, max_x, min_y, max_y)
    }

    /// Convert world coordinates to screen coordinates
    pub fn world_to_screen(&self, x: i32, y: i32) -> (i32, i32) {
        let (min_x, _, min_y, _) = self.get_screen_bounds();
        (x - min_x, y - min_y)
    }

    /// Convert screen coordinates to world coordinates
    pub fn screen_to_world(&self, screen_x: i32, screen_y: i32) -> (i32, i32) {
        let (min_x, _, min_y, _) = self.get_screen_bounds();
        (screen_x + min_x, screen_y + min_y)
    }

    /// Check if world coordinates are within camera bounds
    pub fn in_bounds(&self, x: i32, y: i32) -> bool {
        let (min_x, max_x, min_y, max_y) = self.get_screen_bounds();
        x >= min_x && x < max_x && y >= min_y && y < max_y
    }
}

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Camera>()
            .add_systems(Update, update_camera);
    }
}

fn update_camera(player_query: Query<&Position, With<Player>>, mut camera: ResMut<Camera>) {
    if let Ok(player_pos) = player_query.get_single() {
        camera.x = player_pos.x;
        camera.y = player_pos.y;
    }
}
