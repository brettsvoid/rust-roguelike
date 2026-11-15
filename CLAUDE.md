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

# Screenshot harness: start a seeded new game, wait N frames, save a PNG, exit.
# Depth 1 is the town. Same seed and flags give a byte-identical PNG.
# Defaults: --seed 42 --depth 1 --frames 60, fog on unless --reveal-map.
cargo run -- --screenshot out.png --seed 42 --depth 3 --reveal-map --frames 60
```

Verify every visible change with a harness screenshot that you look at.

## Game Design

See [docs/design.md](docs/design.md) for the full game design document and
[docs/ROADMAP.md](docs/ROADMAP.md) for what's done and what's still to do.
The game is mid-overhaul to isometric 3D with party tactics and a living town.
[docs/TODO.md](docs/TODO.md) holds the owner's decisions and the ticket list
for that overhaul. Key points:
- **Goal**: Descend 12 levels to retrieve the Heart of the Abyss, then escape back to the surface
- **Genre**: Turn-based tactical roguelike with permadeath and procedural generation
- **Theme**: Classic fantasy dungeon crawler (orcs, goblins, undead, demons)
- **Presentation**: Isometric true 3D, dark torch-lit dungeons, stylised low-poly models
- **Party and combat**: A party of 4 heroes (default), XCOM-style combat (action points, hit chances, cover, overwatch). Free click-to-move exploration drops into turn mode on contact
- **Living town**: Loot sold in town is traded with neighbouring towns, sectors level up and stock better gear. The town persists across permadeath
- **Controls**: Point and click, WASD camera pan
- **Mechanics**: Hunger/survival, items/equipment

The Architecture section below describes the code as it is now. The current
build is still the ASCII single-player game with keyboard movement.

## Architecture

This is a Bevy-based roguelike game following the tutorial at https://bfnightly.bracketproductions.com/rustbook/.

### Plugin Structure

The game uses Bevy's plugin system. Each module defines a plugin that registers its systems:

- **GameFlowPlugin** (`game_flow.rs`) - The `RunState` turn state machine and turn scheduling
- **ResourcesPlugin** (`resources.rs`) - Loads shared resources like fonts
- **PlayerPlugin** (`player.rs`) - Player spawning and input handling (HJKL/arrow keys/numpad)
- **ViewshedPlugin** (`viewshed.rs`) - Field of view calculation using Bresenham line algorithm
- **MapPlugin** (`map.rs`) - Map data, fog of war, position-to-screen translation
- **MapGenVizPlugin** (`mapgen_viz.rs`) - The map generation visualizer (main menu tool)
- **MonstersPlugin** (`monsters.rs`) - Monster spawning and AI
- **GuiPlugin** (`gui.rs`) - Main menu and builder-select menu (other UI lives in `ui/`)

Non-plugin modules worth knowing:

- `map_builders/` - All procedural generation, organized as builder chains (see its module doc)
- `map_render.rs` - The one place map tiles get drawn (`spawn_map_tiles`, `tile_glyph`)
- `levels.rs` - New-game setup and descending to the next level
- `saveload.rs` - Save/load plus the save-on-quit (Q) system

### Game State

The game uses a turn-based `RunState` enum (defined in `game_flow.rs`, re-exported as `crate::RunState`):
- `MainMenu` - Default state, shows main menu (New Game / Continue / Quit)
- `PreRun` - Initial setup before gameplay begins
- `AwaitingInput` - Waiting for player input
- `PlayerTurn` - Processing player actions (combat, items)
- `MonsterTurn` - Processing monster AI
- `ShowInventory` / `ShowDropItem` / `ShowTargeting` - UI overlay states

Player movement triggers `PlayerTurn` → `MonsterTurn` → `AwaitingInput` cycle.

### Dead Code Convention

Unused-but-useful code (alternate map builders, presets, utilities) is kept
as a toolbox for later, marked `#[allow(dead_code)]` with a short "Toolbox:"
comment. Don't delete it; only delete code that is superseded by a newer copy
of the same logic. The build should stay at zero warnings.

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
