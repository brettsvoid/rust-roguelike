# Roadmap

Working list of what's done, what's in progress, and what's still to do.
The game design lives in [design.md](design.md) - this file tracks the work.

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

- [ ] Finish town: shopkeepers with actual shops, dungeon entrance flow
- [ ] Level theming per depth band (see the depth table in design.md) -
      pick builder chains + spawn tables per theme instead of fully random
- [ ] Equipment progression: more weapons/armor in the raws, drop tables
- [ ] Boss encounters at key depths
- [ ] The Heart of the Abyss + return journey (difficulty scaling on the way up)
- [ ] Item identification (unidentified potions/scrolls)
- [ ] Sound effects / music

## Sprite graphics (future)

The plan: move from ASCII glyphs to image sprites, eventually animated.

- [ ] Pick a tileset (e.g. Kenney or DawnLike are free; or custom pixel art)
- [ ] Swap tile rendering from `Text2d` to `Sprite` with a `TextureAtlas` -
      the seam for this is `map_render.rs` (all map tiles spawn there) and
      `RenderableBundle` in `components.rs` (all entities spawn through it)
- [ ] Map `TileType` + wall bitmask -> atlas indices (the `WallGlyph` mask
      already computes which wall piece to use - same logic, different art)
- [ ] Entity sprites driven from the raws JSON (add a `sprite` field next
      to `glyph` so both renderers work during the transition)
- [ ] Animated sprites (Bevy: timer-driven `TextureAtlas` index cycling) -
      idle animations first, then attack/damage flashes
- [ ] Particle/effect sprites to replace the text-based particles

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
      yet - extend `saveload.rs` when equipment matters, or replace the
      hand-rolled serialization with something reflection-based
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
