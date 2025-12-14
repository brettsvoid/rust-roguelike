# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build Commands

```bash
# Build (uses dynamic linking for faster dev builds)
cargo build

# Run the game
cargo run

# Build for release
cargo build --release

# Run with file watcher (hot reloading for assets)
cargo run --features file_watcher
```

## Architecture

This is a Bevy-based roguelike game following the tutorial at https://bfnightly.bracketproductions.com/rustbook/.

### Plugin Structure

The game uses Bevy's plugin system. Each module defines a plugin that registers its systems:

- **ResourcesPlugin** (`resources.rs`) - Loads shared resources like fonts
- **PlayerPlugin** (`player.rs`) - Player spawning and input handling (HJKL/arrow keys/numpad)
- **ViewshedPlugin** (`viewshed.rs`) - Field of view calculation using Bresenham line algorithm
- **MapPlugin** (`map.rs`) - Procedural dungeon generation, tile rendering, fog of war
- **MonstersPlugin** (`monsters.rs`) - Monster spawning and AI
- **GuiPlugin** (`gui.rs`) - UI panels, menus (inventory, targeting, main menu), tooltips

### Game State

The game uses a turn-based `RunState` enum:
- `MainMenu` - Default state, shows main menu (New Game / Continue / Quit)
- `PreRun` - Initial setup before gameplay begins
- `AwaitingInput` - Waiting for player input
- `PlayerTurn` - Processing player actions (combat, items)
- `MonsterTurn` - Processing monster AI
- `ShowInventory` / `ShowDropItem` / `ShowTargeting` - UI overlay states

Player movement triggers `PlayerTurn` → `MonsterTurn` → `AwaitingInput` cycle.

### Save/Load

- `saveload.rs` - Serializes game state to `savegame.json` using serde
- Saves on quit (Q key), loads on Continue from main menu
- Permadeath: save file deleted on load and on player death
- WASM: save/load disabled (no filesystem access)

### Coordinate System

- `Position` component uses grid coordinates (not pixels)
- `translate_positions` system converts grid positions to screen coordinates
- Map origin (0,0) is top-left; Y increases downward
- Text2d characters render each tile ("." for floor, "#" for wall)

### Key Resources

- `Map` - Contains tile data, rooms, and visibility state
- `UiFont` - Shared font handle for all text rendering

## Commit Style

Commits should be one line in the format: `feat: add [feature description]`
