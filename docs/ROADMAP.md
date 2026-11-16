# Roadmap

Working list of what's done, what's in progress, and what's still to do.
The game design lives in [design.md](design.md) - this file tracks the work.

The overhaul to an isometric 3D game with party tactics and a living town is
tracked ticket by ticket in [TODO.md](TODO.md). Where this file and TODO.md
disagree, TODO.md wins.

## Done

- [x] Core loop: turn-based movement, melee combat, monster AI, death
- [x] Field of view + fog of war
- [x] Items, inventory, ranged targeting, AoE, confusion, magic mapping
- [x] Hunger system
- [x] Traps (hidden, revealed by search)
- [x] Save/load with permadeath (Q to save+quit, Continue from menu)
- [x] Map builder chain architecture (initial builders + meta builders)
- [x] Big builder toolbox: simple rooms, BSP, cellular automata, drunkard's
      walk, DLA, maze, Voronoi, WFC, prefabs, erosion, corridor styles
- [x] Map generation visualizer (watch algorithms run, from the main menu)
- [x] Camera system (map size decoupled from screen size)
- [x] Data-driven entities and spawn tables from JSON (`raws/`)
- [x] Procedural town as starting location (depth 1)
- [x] Bystander NPCs (WIP - town population)

## Next up

Outside the overhaul tickets:

- [ ] Level theming per depth band (see the depth table in design.md) -
      pick builder chains + spawn tables per theme instead of fully random
- [ ] Boss encounters at key depths
- [x] The Heart of the Abyss + return journey (TODO.md P05)
- [ ] Difficulty scaling on the way up with the Heart
- [ ] Item identification (unidentified potions/scrolls)
- [ ] Sound effects / music

Moved into [TODO.md](TODO.md):

- Shopkeepers and shops: E02 onwards (vendors, town state, trade, tiered
  stock, returning to town)
- Equipment progression and drop tables: E01 (item values) and E05 (tiered
  weapons, armour and loot by depth)
- Graphics: the 3D plan (F and A tickets) replaces the old sprite plan

## Code health

Done in the big cleanup (July 2026):

- [x] One shared `spawn_map_tiles` / `tile_glyph` (was copy-pasted 5x)
- [x] main.rs split into `game_flow`, `levels`, `mapgen_viz`, `map_render`
- [x] Removed the legacy pre-chain `MapBuilder` impls (~1,500 lines)
- [x] Every map builder documented (what the algorithm does, what it's good for)
- [x] Zero compiler + clippy warnings (toolbox code kept, marked `#[allow(dead_code)]`)

Still to do:

- [ ] Spawning consistency: `VoronoiSpawning`, `RoomBasedSpawner`, and
      `CorridorSpawner` hardcode entity names - they should roll on the JSON
      spawn tables like `spawn_room` does
- [ ] Save/load doesn't persist equipment (Equipped/Equippable) or bystanders
      yet - TODO.md ticket P01 reworks `saveload.rs` and fixes this
- [ ] `MapBuilder` trait has only one implementor (`BuilderChain`) - could
      collapse the trait away and return `BuilderChain` directly
- [ ] Wire up unused toolbox pieces when wanted: `RoomDrawer` (circular
      rooms), `CorridorSpawner`, `DoorPlacement` in the random chains,
      erosion presets, `GameRng::seeded` for a daily-run mode

## Toolbox (unwired but ready)

Kept in the code with `#[allow(dead_code)]`, waiting to be used:

| Piece | Where | What it's for |
| --- | --- | --- |
| `RoomDrawer::circles()` | `room_modifiers.rs` | circular rooms |
| `CorridorSpawner` | `corridors.rs` | ambushes in hallways |
| `DoorPlacement` | `doors.rs` | doors (only in `default_builder` chain) |
| `DrunkardsWalkEroder::heavy/symmetric` | `erosion.rs` | stronger erosion looks |
| `WfcBuilder::with_chunk_size` | `wfc.rs` | tune WFC chunk size |
| Prefab placements (Left/Center/Bottom) | `prefab.rs` | more section spots |
| `XStart`/`YStart` variants | `area_based.rs` | start player at edges |
| `a_star` (entity-blocking) | `pathfinding.rs` | stricter pathing |
| `GameRng::seeded` | `rng.rs` | reproducible runs / daily seeds |
| Modal menu toolkit | `ui/menu.rs` | future menus |
