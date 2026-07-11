//! Spawns the Text2d entities that draw the map on screen.
//!
//! Every path that puts a map on screen (new game, next level, loading a
//! save, the map visualizer) goes through `spawn_map_tiles` - so tile
//! appearance only ever needs changing in `tile_glyph`.

use bevy::prelude::*;

use crate::map::{Map, Position, Revealed, RevealedState, Tile, TileType};

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

/// Spawn a tile entity for every drawable tile in the map.
pub fn spawn_map_tiles(commands: &mut Commands, map: &Map, font: &TextFont, reveal: TileReveal) {
    for y in 0..map.height {
        for x in 0..map.width {
            let idx = map.xy_idx(x, y);
            let tile = map.tiles[idx];

            let revealed = match reveal {
                TileReveal::Hidden => RevealedState::Hidden,
                TileReveal::Preview => RevealedState::Visible,
                TileReveal::FromSave => {
                    if map.revealed_tiles[idx] {
                        RevealedState::Explored
                    } else {
                        RevealedState::Hidden
                    }
                }
            };

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
