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
# Fake the mouse at a window point (logical pixels) and enter targeting with an item:
cargo run -- --screenshot out.png --depth 3 --reveal-map --cursor 720,430 \
  --console "spawn fireball" --target "Fireball Scroll"
# --click X,Y left-clicks grid cell X,Y (repeatable; the next input waits for the
# walk to end; a click on another hero selects them). --keys presses keys through the
# real input: g . (pick up, descend), i (inventory), n (Tab: select the next hero),
# w a s d (pan), c (recentre), + and - (scroll to zoom). They run in the order given,
# e.g. walk five tiles east so explored-but-unseen tiles show:
cargo run -- --screenshot fog.png --seed 42 --depth 3 --click 45,20 --frames 120
# Select the third hero (Tab twice) and walk them; at seed 42 depth 3 the party
# stands on (40,21) (41,21) (39,21) (40,22):
cargo run -- --screenshot tab.png --seed 42 --depth 3 --keys nn --click 43,25 --frames 150
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
- **Controls**: Left click walks the active hero to an explored tile (one tile per turn, stopping
  when a hostile comes into view), attacks an adjacent hostile, walks to the down stairs and takes
  them, and on the hero's own tile picks up an item. A click on another hero, or Tab, selects
  them (no turn passes). WASD pans the camera, the scroll wheel zooms, C recentres. G picks up,
  I inventory, X drops (D pans), R removes equipment, `.` descends, Space waits, T opens the
  ledger in town, Q saves and quits
- **Mechanics**: Hunger/survival, items/equipment

The Architecture section below describes the code as it is now. The current
build is mid-conversion: the map and entities are grey-box 3D primitives on
an isometric camera, with a party of four heroes. Only the active hero moves
and acts (one step per turn); the others stand still. No followers, explore
mode or tactical combat yet.

## Architecture

This is a Bevy-based roguelike game following the tutorial at https://bfnightly.bracketproductions.com/rustbook/.

### Plugin Structure

The game uses Bevy's plugin system. Each module defines a plugin that registers its systems:

- **GameFlowPlugin** (`game_flow.rs`) - The `RunState` turn state machine and turn scheduling
- **ResourcesPlugin** (`resources.rs`) - Loads shared resources like fonts
- **PartyPlugin** (`party.rs`) - The party: `PartyMember { slot }` on every hero, the `ActiveUnit`
  resource (the hero the input drives), `HEROES` (names and colours), and the pure rules
  `spawn_tiles`, `next_unit` (Tab, skipping the dead) and `party_wiped`
- **PlayerPlugin** (`player.rs`) - Input for the active hero: keys, Tab and a click on a hero to
  select, left click to walk/attack/pick up/descend, the
  `WalkPath` resource (the cells of a click walk still to take, and whether to take the stairs on
  arrival) and the hover path preview
- **ViewshedPlugin** (`viewshed.rs`) - Field of view calculation using Bresenham line algorithm
- **MapPlugin** (`map.rs`) - Map data, fog of war, position-to-screen translation, `grid_to_world`
- **CameraPlugin** (`camera.rs`) - The logical `Camera` follow target, WASD pan, scroll zoom, the isometric 3D camera and the 2D overlay camera
- **PickingPlugin** (`picking.rs`) - Cursor to grid cell via the `Camera3d` ray on the floor plane, the `HoveredTile` resource, the hover highlight and `TileMarks` (flat unlit quads for tile overlays)
- **OcclusionPlugin** (`occlusion.rs`) - Cuts walls down to stubs and hides town roofs so the party and the visible floor stay in view (see Coordinate System)
- **MapGenVizPlugin** (`mapgen_viz.rs`) - The map generation visualizer (main menu tool)
- **MonstersPlugin** (`monsters.rs`) - Monster spawning and AI. `monster_target` picks the nearest
  hero a monster sees or stands next to
- **GuiPlugin** (`gui.rs`) - Main menu and builder-select menu (other UI lives in `ui/`)
- **LightingPlugin** (`lighting.rs`) - `LightSource` point lights, flicker, the shadow caster cap,
  and the per-depth ambient light, fog, bloom and town sun (see Lighting)

Non-plugin modules worth knowing:

- `map_builders/` - All procedural generation, organized as builder chains (see its module doc)
- `map_render.rs` - The one place map tiles get drawn. `spawn_map_tiles` spawns 3D tiles in game
  (looks in `tile_style`, shared meshes and a bright and a dim material per `TileType` in the
  `TileAssets` resource) and `Text2d` glyphs for the visualiser (`TileReveal::Preview`, looks in
  `tile_glyph`)
- `entity_render.rs` - 3D bodies for creatures, items, props, particles and bloodstains. An entity
  with `EntityShape` + `Renderable` (raws glyph and colour) gets a shared mesh per shape and a
  shared material per colour from `EntityAssets` via the `attach_entity_mesh` observer. Spawn
  map entities with `RenderableBundle`; re-inserting `EntityShape` re-dresses (doors opening)
- `models.rs` - The model pipeline (see Models below)
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

Each step of the active hero (a click walk takes one per turn) triggers the `PlayerTurn` → `MonsterTurn` →
`AwaitingInput` cycle.

### Models

Drop a glTF binary in `assets/models/<name>.glb` and the matching primitive turns into it,
with no code change. A missing file keeps the grey-box primitive, silently (`AvailableModels`
scans the folder once at startup, so restart after adding a file).

- Mobs, items and props: set `"model": "<name>"` in the entry's `renderable` in
  `raws/spawns.json`, beside `glyph`. An open door looks for `<name>_open`.
- Tiles: the name comes from `models::tile_model_name`: `tile_floor`, `tile_wood_floor`,
  `tile_road`, `tile_grass`, `tile_bridge`, `tile_shallow_water`, `tile_deep_water`,
  `tile_down_stairs`, and `tile_wall_<mask>` where the mask (0 to 15) is `Map::wall_glyph_at`:
  N=1, S=2, W=4, E=8, set where that neighbour is a wall next to floor.
- Conventions: one Blender unit is one metre and one tile, origin at the tile centre on the
  floor, walls 2 m tall. In Blender's top view +X is east and +Y is north (towards smaller grid y).
- Fog: entity models hide with their entity. Tile models swap each mesh's material for a
  dimmed copy (`map_render::dimmed`) when explored but out of view.
- Export: Blender sources live in `art/`. Each top-level collection becomes one `.glb` named
  after it: `blender art/<file>.blend --background --python art/export.py`.
  `art/export.py` is untested in Blender so far; check its first export by eye.
- `art/make_test_model.py` writes `assets/models/test_pyramid.glb` (no Blender needed). Copy it to
  a used name, e.g. `tile_floor.glb`, to check the pipeline, and delete the copy afterwards.
- Every model is loaded at startup and kept. The harness starts the game only once they have
  all loaded, so model entities spawn in the same frame and order each run (byte-identical PNGs).
- Wall cut-away (`occlusion.rs`) scales the wall tile's `Transform` on y, so a wall model is
  squashed to a stub the same way as the primitive.
- Model scenes need `reflect_auto_register` (in `Cargo.toml`): scenes are copied into the world
  by reflection.

### Dead Code Convention

Unused-but-useful code (alternate map builders, presets, utilities) is kept
as a toolbox for later, marked `#[allow(dead_code)]` with a short "Toolbox:"
comment. Don't delete it; only delete code that is superseded by a newer copy
of the same logic. The build should stay at zero warnings.

### Save/Load

- `saveload.rs` - Serializes game state to `savegame.json` using serde
- Saves on quit (Q key), loads on Continue from main menu
- The save holds the whole party (`SaveData::party`, in slot order), the active hero's slot, and
  each backpack item's owner by slot
- Permadeath: save file deleted on load and when the last hero dies. A fallen hero leaves play
  and drops what they carried; the selection moves to the next living hero
- WASM: save/load disabled (no filesystem access)
- Town state (`economy/town.rs`) lives in its own `town.json`, loaded at startup
  and saved whenever `TownState` changes. Permadeath never deletes it. A bad or
  wrong-version file is renamed to `town.json.bad-<secs>` and a fresh town founded.

### Coordinate System

- `Position` component uses grid coordinates (not pixels) and stays the gameplay truth
- Map origin (0,0) is top-left; Y increases downward
- 3D: `map::grid_to_world` is the only grid-to-world mapping. Grid `(x, y)` is the tile centre at
  world `(x, 0, y)`, one tile is one world unit, the floor is `y = 0`, walls are 2 units tall
- The 3D camera is orthographic, yaw 45 degrees, pitch 35 degrees, and follows the logical
  `Camera` resource. That resource follows the active hero until WASD sets `panned`; that hero
  moving, selecting another, or C clears it. Move that resource to move the view. Zoom is the projection's `viewport_height`
- Camera layering: the `Camera3d` (order 0) draws the world and clears the screen; the `Camera2d`
  (order 1, `IsDefaultUiCamera`) draws UI and any `Text2d` on top. The 3D camera is HDR and the
  2D one is not, so each has its own texture: the 2D camera clears to transparent and Bevy
  alpha-blends it over the world (with no clear, the main menu stays on screen). Queries for the
  render camera must filter `With<Camera2d>` or `With<Camera3d>`, since `single()` fails with two
- Mouse: `picking::ray_to_tile` turns a ray into the grid cell it hits on `y = 0`, and
  `update_hovered_tile` writes `HoveredTile { cursor, tile }` each frame. `tile` is `None` off the
  window, over a `BlocksPicking` UI panel (the HUD, the targeting banner), off the map, or outside
  gameplay. Every mouse consumer (tooltip, targeting, debug tile info) reads `HoveredTile`; order
  such systems `.after(picking::update_hovered_tile)`. Don't convert the cursor any other way
- Occlusion: the camera looks from the +x, +z side, so a wall hides tiles up to two steps
  towards -x, -z. `occlusion::walls_to_cut` lists every wall that would hide a visible non-wall
  tile or any hero; those are drawn as a 0.25-unit stub (the wall `Transform` is scaled, the
  mesh is shared). Town buildings (closed `WoodFloor` rooms) get a flat roof slab at wall height.
  A roof hides while any hero is inside the building or behind it; while it shows, its walls
  stay full height
- Click-to-move paths (`pathfinding::a_star_explored`) only cross explored tiles
- Tile overlays (hover, path preview, targeting range, area, target) are `TileMarks::mark` quads
  just above the floor, never 2D sprites
- `translate_positions` moves a 3D entity (`EntityShape`) to `grid_to_world` plus its shape's lift
  whenever its `Position` changes. Tiles are placed once at spawn. Particles and bloodstains have
  no `Position` (they are not on the tile for gameplay) and get their transform at spawn
- 2D: `translate_glyphs` places the visualiser's `Text2d` tiles in screen pixels

### Fog of War

- Tiles: `Revealed` is `Hidden` (not drawn), `Explored` (dim, desaturated, unlit material, so
  remembered tiles stay readable in the dark) or `Visible` (lit material);
  `update_tile_mesh_visibility` applies it
- Vision is the union of every living hero's `Viewshed`: it fills `Map::visible_tiles` and the
  revealed tiles
- Entities other than heroes are shown only on tiles in `Map::visible_tiles`
  (`update_renderable_visibility`); `Hidden` (unfound traps) always hides
- Debug `reveal_map` marks every tile explored; `no_fog` (and the harness's `--reveal-map`)
  treats every revealed tile as visible, so entities show too

### Lighting

- The dungeon has near-black ambient light, set per camera (`AmbientLight` on the `Camera3d`).
  Light comes from `LightSource` (colour, radius in tiles, flicker depth, flame height) entities.
  Props get one from the raws `light` entry (Candle, Lamp Post, Wall Torch); every hero gets
  `PLAYER_TORCH` from an observer on `PartyMember`
- `attach_lights` gives each `LightSource` a child `PointLight` (always visible, so light spills
  past the fog of war) and, except for heroes, an emissive flame mesh that blooms. A light on
  a wall tile hangs just off the wall face on the open side. Intensity scales with radius squared
- Flicker runs on `Time`. The harness sets `TimeUpdateStrategy::ManualDuration` (1/60 s per frame),
  so screenshots stay byte-identical. Don't drive visuals from wall-clock time any other way.
  The harness also compiles shaders synchronously, so frame N is the same picture on any machine
- `FRAME_TIME_LOG=1 cargo run --release` turns vsync off and logs frame times, for performance checks
- Only the `MAX_SHADOW_LIGHTS` point lights nearest the active hero cast shadows (cube maps are six
  passes each); the rest light through walls within their radius
- `apply_environment` switches on `Map::depth`: depth 1 (the town) gets a low warm directional sun
  with shadows and brighter ambient; dungeons get near-black ambient. Both get HDR, bloom,
  the default TonyMcMapface tonemapping and linear distance fog that darkens the far side of view
- `WallTorches` (`map_builders/wall_torches.rs`) is appended to every dungeon chain in
  `level_builder`; it hangs torches on one-sided walls, spaced apart

### Key Resources

- `Map` - Contains tile data, rooms, and visibility state
- `ActiveUnit` - The hero the player commands (`None` outside a run)
- `UiFont` - Shared font handle for all text rendering
- `HoveredTile` - The grid cell under the mouse this frame (see Coordinate System)

## Commit Style

Commits should be one line in the format: `feat: add [feature description]`
