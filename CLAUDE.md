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
# It renders into a fixed 1600x1000 off-screen image, so the picture does not depend on the
# window size and works with the screen locked or asleep. --cursor points are in that image.
# Depth 0 is the town, the dungeon is depths 1 to 12. Same seed and flags give a
# byte-identical PNG.
# Known exception: two runs can differ by a handful of isolated pixels (about 5 to 20)
# with identical game state, so compare by eye or with a pixel count, not a hash. The
# lead is GPU light clustering order (see F10 in docs/TODO.md). The harness window is
# hidden, so nothing flashes up on screen while it runs.
# The harness reads and writes town.json in the working directory: copy a baseline
# back before each run when you compare captures, since sales and purchases change it.
# Defaults: --seed 42 --depth 0 --frames 60, fog on unless --reveal-map.
cargo run -- --screenshot out.png --seed 42 --depth 2 --reveal-map --frames 60
# Fake the mouse at a window point (logical pixels) and enter targeting with an item:
cargo run -- --screenshot out.png --depth 2 --reveal-map --cursor 720,430 \
  --console "spawn fireball" --target "Fireball Scroll"
# --click X,Y left-clicks grid cell X,Y (repeatable; the next input waits for the
# walk to end; a click on another hero selects them). --keys presses keys through the
# real input: g . (pick up, take the stairs), i (inventory), t (town ledger), n (Tab:
# select the next hero), e (end the party phase in combat), f (attack the nearest hostile in
# reach), o (overwatch), h (hunker down),
# w a s d (pan: each holds the key for 10 frames, about 2.3 tiles), c (recentre),
# + and - (one wheel notch to zoom). The camera eases (about a second to settle), and the
# capture is taken at the fixed frame --frames, so a short --frames shows it mid-glide.
# Input is fed only while the game waits for it, a shop is
# open or the inventory is open (a to j use that line). They run in the order given,
# e.g. walk five tiles east so explored-but-unseen tiles show:
cargo run -- --screenshot fog.png --seed 42 --depth 2 --click 45,20 --frames 120
# Select the third hero (Tab twice) and walk them; at seed 42 depth 2 the party
# stands on (40,21) (41,21) (39,21) (40,22):
cargo run -- --screenshot tab.png --seed 42 --depth 2 --keys nn --click 43,25 --frames 150
# Play an effect on the active hero's tile (spark, fireball, missile, heal, confusion; blood
# stains the active hero's tile and two east). It starts about frame 3, so frame 12 is ~0.15 s in:
cargo run -- --screenshot fx.png --depth 2 --console "effect missile" --frames 12
# Walk south-west until two goblins see the party (combat, round 1, Aldric on (36,24)).
# Aldric moves (1 AP, 1 left), Tab, Brenna dashes (2 AP), E ends the phase: the goblins
# close in (one moves four tiles over two moves). Tab twice back to Aldric, who attacks
# the goblin beside him, which ends his turn:
cargo run -- --screenshot combat.png --depth 2 --reveal-map --console godmode \
  --click 30,28 --click 34,27 --keys n --click 35,28 --keys enn --click 33,28 --frames 300
# --press X,Y left-clicks a point of the 1600x1000 capture through the HUD's own click
# handling (queued with the other inputs; each press waits a frame for the HUD layout).
# In the party phase the action buttons sit on y=944 at x=548 Attack, 620 Overwatch,
# 692 Hunker, 764 Use Item, 836 Wait, 908 End Turn; exploring, Use Item and Wait sit at
# 692 and 764; the party cards are at x=150, y=889 + 30 per slot. Wait starts combat with
# a goblin placed four tiles east, End Turn shows the "Round 2" banner, the second card
# selects Brenna and Attack hits the goblin beside her:
cargo run -- --screenshot hud.png --depth 2 --reveal-map --console godmode --console "place 4 0 Goblin" \
  --console "place 3 0 Table" --press 764,944 --press 908,944 --press 150,919 --press 548,944 --frames 160
# The harness prints the whole game log as "log>" lines when it captures. Keys also reach the
# inventory: `.gia.` picks up a spawned bow and equips it (the leading and trailing `.` pad the
# frame where the item spawns and the one where the inventory closes). Aldric shoots the goblin
# two tiles away in round 2 ("hit for 8 (65%)" at seed 42):
cargo run -- --screenshot bow.png --depth 2 --reveal-map --console godmode --console "spawn bow" \
  --keys .gia. --click 30,28 --click 35,26 --click 33,30 --keys ennn --click 33,28 --frames 40
# --shop VENDOR clicks that vendor (blacksmith, alchemist, clothier, barkeep, priest,
# enchanter): the active hero walks beside them and the shop opens. Inputs after it are
# fed into the shop: n (Tab: buy or sell), a to j (trade that line), ! (Escape: close).
# "gold N" fills the purse. A monster in view stops the walk, so add --console kill_all
# when rats in an advanced town get in the way. Buy the first item, close the shop and
# show the inventory:
cargo run -- --screenshot buy.png --reveal-map --console "gold 40" --shop blacksmith \
  --keys 'a!i' --frames 420
# Sell a picked-up dagger:
cargo run -- --screenshot sell.png --reveal-map --console "spawn dagger" --keys g \
  --shop blacksmith --keys na --frames 420
# The return-to-town loop. --reveal-map holds on every level reached, and the game log
# is printed as "log>" lines at capture. A key in the first input frame can miss items
# the console just spawned, so press g once more than needed. Sell a dagger, take the
# dungeon entrance (12,28 in a fresh town; it moves with the town's seed), read the
# Town Portal on depth 1 and open the ledger to see the news of the day away:
cargo run -- --screenshot loop.png --reveal-map --console "spawn dagger" \
  --console "spawn portal" --keys ggg --shop blacksmith --keys 'na!' --click 12,28 \
  --keys iat --frames 900
# "place DX DY NAME" spawns any raws entry (prop, monster or item, exact name) at an offset
# from the active hero (at the start, Aldric on 40,21 at depth 2). Stage cover: tables east of
# Aldric, a goblin closes in and attacks ("Goblin attacks Aldric: miss (45%, half cover)"),
# Aldric hits it from beside its table ("(45%, half cover)") and Brenna flanks it ("(65%, flanked)"):
cargo run -- --screenshot cover.png --depth 2 --reveal-map --console godmode \
  --console "place 5 0 Goblin" --console "place 1 0 Table" --console "place 1 -1 Table" \
  --console "place 1 1 Table" --click 40,20 --keys e --click 41,21 --keys n --click 40,21 --click 41,21 --frames 120
# Overwatch: Aldric equips a bow, goes on overwatch (o), E ends the phase and the goblin
# walking in draws a reaction shot ("Overwatch! Aldric shoots Goblin: miss (45%)"):
cargo run -- --screenshot ow.png --depth 2 --reveal-map --console godmode --console "spawn bow" \
  --console "place 6 0 Goblin" --keys .gia.oe --frames 120
# A goblin hemmed in behind three others goes on overwatch ("Goblin goes on overwatch.") and
# reacts when Aldric walks up beside it ("Overwatch! Goblin attacks Aldric: miss (45%)"):
cargo run -- --screenshot ow_monster.png --depth 2 --reveal-map --console godmode \
  --console "place 7 0 Goblin" --console "place 6 -1 Goblin" --console "place 6 0 Goblin" \
  --console "place 6 1 Goblin" --keys ee --click 46,20 --frames 90
# Hunker down: in the cover staging Aldric hunkers (h) and the goblin's attack shows the
# doubled cover ("Goblin attacks Aldric: miss (25%, half cover, hunkered)"):
cargo run -- --screenshot hunker.png --depth 2 --reveal-map --console godmode \
  --console "place 5 0 Goblin" --console "place 1 0 Table" --console "place 1 -1 Table" \
  --console "place 1 1 Table" --click 40,20 --keys he --frames 90
# "spawn heart" drops a stand-in Heart of the Abyss; carrying it, the portal is refused:
cargo run -- --screenshot heart.png --depth 4 --console "spawn portal" \
  --console "spawn heart" --keys gggia --frames 300
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
- **Controls**: Exploring, left click walks the active hero to an explored tile (one tile per
  turn, stopping when a hostile comes into view), attacks an adjacent hostile, walks to stairs
  and takes them, and on the hero's own tile picks up an item; the other heroes follow.
  In combat, left click on a tile in the drawn move range moves there as one action (blue zone
  1 AP, green zone a 2 AP dash), on a hostile the party sees attacks it with the hero's weapon
  if it is in range and line of sight (otherwise logs why and spends nothing), and on an
  adjacent closed door opens it; anything else logs "Out of range.". Hovering a hostile in
  combat shows the active hero's chance to hit it in the tooltip (with "half cover", "full
  cover" or "flanked"); hovering a tile in the move range shows cover shields on its covered sides. A click on another hero, or Tab, selects them
  (no turn passes; in combat only heroes with AP left).
  Holding WASD pans the camera smoothly, the scroll wheel, two-finger touchpad scroll or a
  pinch zooms, C recentres. G picks up, I inventory, X drops
  (D pans), R removes equipment, `.` takes the stairs, Space waits (in combat it ends that hero's
  turn), O puts the active hero on overwatch and H hunkers them down (combat only; each ends
  that hero's turn), E ends the party phase in combat, T opens the ledger in town, Q saves and quits
- **Mechanics**: Hunger/survival, items/equipment

The Architecture section below describes the code as it is now. The current
build is mid-conversion: the map and entities are grey-box 3D primitives on
an isometric camera, with a party of four heroes. Exploring, the active hero
leads and the others follow; on contact the game drops into combat rounds where
every unit spends 2 action points a round and attacks roll to hit, less the target's cover.

## Architecture

This is a Bevy-based roguelike game following the tutorial at https://bfnightly.bracketproductions.com/rustbook/.

### Plugin Structure

The game uses Bevy's plugin system. Each module defines a plugin that registers its systems:

- **GameFlowPlugin** (`game_flow.rs`) - The `RunState` frame-loop state, the `Mode` resource
  (explore or combat) and `advance`, the pure transition between them (see Game State)
- **ResourcesPlugin** (`resources.rs`) - Loads shared resources like fonts
- **PartyPlugin** (`party.rs`) - The party: `PartyMember { slot }` on every hero, the `ActiveUnit`
  resource (the hero the input drives), `HEROES` (names and colours), and the pure rules
  `spawn_tiles`, `next_unit` (Tab, skipping the dead and, in combat, those out of AP) and
  `follow` (where followers step when the leader moves in explore)
- **PlayerPlugin** (`player.rs`) - Input for the active hero: keys, Tab and a click on a hero to
  select, left click to walk/attack/pick up/descend (in combat, to move within range), the
  `WalkPath` resource (the cells of a click walk still to take, and whether to take the stairs on
  arrival), the hover path preview, and in combat the `MoveRange` resource (the active hero's
  range, recomputed each frame of the party phase) and its two-zone overlay
- **ViewshedPlugin** (`viewshed.rs`) - Field of view calculation using Bresenham line algorithm
- **MapPlugin** (`map.rs`) - Map data, fog of war, position-to-screen translation, `grid_to_world`
- **CameraPlugin** (`camera.rs`) - The logical `Camera` follow target, WASD pan, scroll zoom, the isometric 3D camera and the 2D overlay camera
- **PickingPlugin** (`picking.rs`) - Cursor to grid cell via the `Camera3d` ray on the floor plane, the `HoveredTile` resource, the hover highlight and `TileMarks` (flat unlit quads for tile overlays)
- **OcclusionPlugin** (`occlusion.rs`) - Cuts walls down to stubs and hides town roofs so the party and the visible floor stay in view (see Coordinate System)
- **MapGenVizPlugin** (`mapgen_viz.rs`) - The map generation visualizer (main menu tool)
- **MonstersPlugin** (`monsters.rs`) - Monster spawning and AI. `monster_target` picks the nearest
  hero a monster sees or stands next to. Exploring, `monster_ai` takes one step or attacks per
  turn; in combat each monster spends its AP by the heroes' rules (move as far along its path as
  one AP reaches, attack when a hero is in reach of its weapon, which ends its turn). Monsters
  have no weapons yet, so in practice they attack from beside a hero
- **GuiPlugin** (`gui.rs`) - Main menu and builder-select menu (other UI lives in `ui/`)
- **HudPlugin** (`ui/hud.rs`) - The bottom bar: party cards (portrait, health, AP pips, status
  badges; a click selects the hero as Tab does), the action bar, depth/gold/mode and the log.
  A button writes the key it stands for to the `ActionRequest` resource and
  `player::handle_player_input` takes it after the real key presses, so a button and its
  hotkey share one code path. The rules (which buttons, why one is disabled, the shot
  preview numbers, which banner a phase change shows) are pure functions in
  `ui/combat_hud.rs`. Shared colours and sizes live in `ui/style.rs`
- **TooltipPlugin** (`ui/tooltip.rs`) - Names what is under the cursor; in combat a hostile shows
  the shot preview (hit, crit, damage, cover or flank) from `combat::aim_at`
- **HealthBarsPlugin** (`ui/health_bars.rs`) - A bar over each unit, a UI node placed by
  projecting the unit's `GlobalTransform` through the `Camera3d` each frame; hidden with the unit
- **BannerPlugin** (`ui/banner.rs`) - The eased centred banner on a mode or phase change
- **LightingPlugin** (`lighting.rs`) - `LightSource` point lights, flicker, the shadow caster cap,
  and the per-depth ambient light, fog, bloom and town sun (see Lighting)
- **EffectsPlugin** (`effects.rs`) - Combat and spell effects (see Effects)

Non-plugin modules worth knowing:

- `map_builders/` - All procedural generation, organized as builder chains (see its module doc)
- `map_render.rs` - The one place map tiles get drawn. `spawn_map_tiles` spawns 3D tiles in game
  (looks in `tile_style`, shared meshes and a bright and a dim material per `TileType` in the
  `TileAssets` resource) and `Text2d` glyphs for the visualiser (`TileReveal::Preview`, looks in
  `tile_glyph`)
- `entity_render.rs` - 3D bodies for creatures, items, props and bloodstains (a flat splat mesh,
  turned per tile). An entity
  with `EntityShape` + `Renderable` (raws glyph and colour) gets a shared mesh per shape and a
  shared material per colour from `EntityAssets` via the `attach_entity_mesh` observer. Spawn
  map entities with `RenderableBundle`; re-inserting `EntityShape` re-dresses (doors opening)
- `models.rs` - The model pipeline (see Models below)
- `tactics.rs` - The pure action point rules: `AP_PER_ROUND` (2), `DEFAULT_MOVEMENT` (4 tiles per
  AP), `Action` and `Action::spend` (move and item 1 AP, dash 2; attack, wait, overwatch and
  hunker spend the rest and need at least 1), `move_range` (cheapest-first flood fill over `Map::get_available_exits`,
  diagonals 1.45 like the pathfinder, giving each reachable tile its AP cost and path) and
  `confused_step`. Movement is `CombatStats::movement`, from the raws `stats.movement` for
  monsters (Giant Bat 6)
- `cover.rs` - Cover and flanking, pure rules over the `Map`. `cover_at(map, cell)`: walls and
  anything blocking sight (a closed door) are `Cover::Full`, else the prop's raws `cover` from
  `Map::prop_cover` (rebuilt by `map_indexing_system` from the `Cover` component), else none.
  Units give no cover. `cover_sides(map, cell)` lists the covered sides (N, S, E, W): the hover
  shields. `exposure(map, target, attacker)` is the XCOM rule: a side counts when the attacker is
  strictly beyond it (a diagonal attacker faces two), the best counting one wins (`Covered`); a
  target with cover none of which counts is `Flanked`; none at all is `Open`. Cover applies to
  melee and ranged alike. `Exposure::hunker(hunkered)` turns `Covered` into `Hunkered` for a
  hunkered target (open and flanked stay as they are). `Exposure::aim_penalty` (half 20, full
  40, doubled when hunkered: 40 and 80) feeds `hit_chance`, `crit_chance(base)` adds +30
  flanked and is 0 against a hunkered target, `note` is the log text ("half cover, hunkered").
  Raws props: `"cover": "none" | "half" | "full"` (doors leave it out)
- `combat.rs` - Attacks, damage and death. The pure rules: `hit_chance(aim, defence, distance,
  weapon, cover, height)` is `aim - defence - cover - 10 per tile past the weapon's optimal range
  + 10 per level above the target`, clamped to 5..95, and 0 past the weapon's range. Callers pass
  `cover::Exposure::aim_penalty` and `LEVEL_GROUND` until a map with height. `in_reach` checks range
  (Chebyshev) and `viewshed::line_of_sight` (Bresenham, walls and `BlocksVisibility` block);
  `aim_at` combines both with `cover::exposure` (hunkered or not) and returns the chance and the `Exposure`. `roll_attack` rolls hit then crit, `roll_damage` rolls the weapon's
  damage + power (+1 well fed), +half rounded up on a crit, less armour, never below 0. All
  rolls go through `GameRng`. Every attack resolves in `Strikes::strike` (`Strikes` is a
  `SystemParam`; `Fighter` is a unit and the cell it attacks from or is hit on), which logs
  "Aldric shoots Goblin #0: hit for 5 (55%)", or "(35%, half cover)" / "(65%, flanked)"
  when cover applied (`odds`), and returns the damage dealt. `attack_system` resolves
  `WantsToAttack` (melee or ranged alike) through it. `Arms` (a `SystemParam`) gives a unit's
  equipped `Weapon` or `Weapon::NATURAL` (power -1..+1, range 1, 5% crit) and its worn armour.
  `CombatStats`: `armour` (raws and saves call it `defense`) cuts damage, `defence` cuts hit
  chance, `aim` defaults to `DEFAULT_AIM` (65)
- Weapons in the raws: `weapon { slot, damage: [min, max], crit_chance, range (default 1),
  optimal_range (default range) }`; `slot` "ranged" (the Shortbow: range 8, optimal 5) takes
  the weapon hand like "melee". Damage scrolls roll `spell_damage` (a quarter either side of
  their `damage`) and ignore armour; Magic Missile rolls to hit with aim +25 (`SPELL_AIM_BONUS`) less cover,
  area scrolls hit everything in the radius, Confusion always lands
- `overwatch.rs` - Overwatch and hunker down. `Overwatch` and `Hunkered` are components put on
  by the O and H keys (or the monster AI) and removed at the start of the unit's next turn
  (`combat::start_of_turn`). The pure rule `reactions(watchers, mover_party, path, sight)`:
  each watcher of the other side fires once, at the first path cell within its weapon range
  and line of sight (the `in_reach` rule; melee is the cells beside it); a unit that does not
  move triggers nothing. `walk_under_watch` walks a mover along its path, resolving each
  reaction through `Strikes::strike` at the cell it triggers on with `OVERWATCH_AIM_PENALTY`
  (20) less aim, logged "Overwatch! Aldric shoots Goblin: ..."; the watcher's overwatch is spent
  and a mover it kills stops there. A hero's combat move inserts `WantsToMove { path }`
  (`MoveRange::path`), walked by `move_system` first in the `PartyAction` chain; monster moves
  call `walk_under_watch` from `monster_ai`. Stand-in monster rules until C07: in combat, a
  monster at a third of its hit points or less in cover from the hero it targets hunkers
  ("Goblin hunkers down."), and one that sees a hero but can neither attack it nor take a step
  towards it with AP left goes on overwatch ("Goblin goes on overwatch."). The HUD party row
  shows "Overwatch" or "Hunkered" after the AP
- `levels.rs` - New-game setup and level changes (`change_level`, run in `RunState::NextLevel`).
  The town is depth 0 (`TOWN_DEPTH`), the dungeon depths 1 to 12. Down stairs go one level
  deeper, or from town to the deepest depth reached; the up stairs (only on depth 1, at the start)
  and a Town Portal scroll (`use_town_portal`, refused in town or while the party carries the
  Heart of the Abyss) go to town. `Delve` holds the deepest depth and the levels entered since
  leaving town; a return to town runs `trade::simulate` for one day per level entered, then
  `stock::restock`. Every entity with a `Position` that is not a hero is despawned with the
  level (carried items have none)
- `saveload.rs` - Save/load plus the save-on-quit (Q) system

### Game State

Two pieces of state, both in `game_flow.rs`:

`RunState` (a Bevy state, re-exported as `crate::RunState`) is what the frame loop is doing:
- `MainMenu` - Default state, shows main menu (New Game / Continue / Quit)
- `PreRun` - Initial setup before gameplay begins; resets `Mode` to explore
- `AwaitingInput` - Waiting for the party's input
- `PartyAction` - Resolving the active hero's action (move, attack, item, wait, end turn)
- `EnemyTurn` - The monsters act (hunger ticks here too). It opens with `start_of_turn` for the
  monsters and ends with it for the heroes (the start of their next turn): `Poison` deals its
  damage and `Confusion` counts down
- `ShowInventory` / `ShowDropItem` / `ShowRemoveItem` / `ShowTargeting` / `ShowLedger` / `ShowShop` - UI
  overlays; they return to `AwaitingInput` and leave `Mode` alone
- `NextLevel`, `MagicMapReveal`, `GameOver`, and the visualiser's `MapBuilderSelect` / `MapGeneration`

`Mode` (a resource) is the tactical mode: `Explore`, or `Combat { round, ap_spent }`. Every change
of mode or phase goes through `advance(mode, step, situation)`, a pure function over plain data
(the living heroes, and per hostile whether it sees a hero and a hero sees it), unit tested in
`game_flow.rs`. The `PartyAction` and `EnemyTurn` chains end in a system that calls it.
- Explore: each hero action is followed by `EnemyTurn` (one turn per step); the leader walks a
  click path and the others follow
- Explore to combat: a hostile and a hero see each other. Combat starts at the party phase of
  round 1
- Combat party phase: each hero has `AP_PER_ROUND` action points, spent in any order across
  heroes (`Mode::ap_left`, `Mode::can_act`). The map input records the `Action` it took in the
  `HeroAction` resource and `Step::PartyActed(hero, action)` spends it; the inventory and
  targeting screens leave it unset, which counts as an item (1 AP). A hero with AP left stays
  active; one out of AP hands over to the next hero who can act. The phase ends when no living
  hero has AP left or on E, then `EnemyTurn` is the enemy phase
- Status effects under AP: poison bites at the start of the unit's turn (Giant Spider melee hits
  poison, raws `poison: [damage, turns]`); a confused unit's move goes to a random adjacent tile
  (`confused_step`, rolled through `GameRng`) and still costs the AP of the move it chose
- Combat to explore: no hostile sees a living hero (dead or out of sight)
- Game over: no hero left alive
- `Mode` is not saved: a loaded game starts in explore and drops back into combat at the next
  step if a hostile still sees the party

### Models

Drop a glTF binary in `assets/models/<name>.glb` and the matching primitive turns into it,
with no code change. A missing file keeps the grey-box primitive, silently (`AvailableModels`
scans the folder once at startup, so restart after adding a file).

- Mobs, items and props: set `"model": "<name>"` in the entry's `renderable` in
  `raws/spawns.json`, beside `glyph`. An open door looks for `<name>_open`.
- Tiles: the name comes from `models::tile_model_name`: `tile_floor`, `tile_wood_floor`,
  `tile_road`, `tile_grass`, `tile_bridge`, `tile_shallow_water`, `tile_deep_water`,
  `tile_down_stairs`, and `tile_wall_<mask>` where the mask (0 to 15) is `Map::wall_glyph_at`:
  N=1, S=2, W=4, E=8, set where that neighbour is a drawn wall (one next to any floor-like tile).
  `models::tile_model_names` lists the names a tile tries, best first: in town (depth 0) a wall
  tries `tile_house_wall_<mask>` when a wood floor or stone floor (doorway, hovel) is among its 8
  neighbours, else `tile_town_wall_<mask>` (the town wall, ruins), then `tile_wall_<mask>`.
  `attach_tile_mesh` reads the mask from the `Map`, not from a component, because the observer
  runs before later inserts on the new tile land.
- Roofs: every tile of a town building (a closed `WoodFloor` room, walls and doorway included)
  gets a `roof` model (+ `_2`, `_3`) on the wall tops (`occlusion::spawn_roofs`), shown and
  hidden with the building; without the file it is a flat slab.
- Conventions: one Blender unit is one metre and one tile, origin at the tile centre on the
  floor, walls 2 m tall. In Blender's top view +X is east and +Y is north (towards smaller grid y).
- Fog: entity models hide with their entity. Tile models swap each mesh's material for a
  dimmed copy (`map_render::dimmed`) when explored but out of view.
- Variants: a tile shows `<name>`, `<name>_2` or `<name>_3`, picked by `models::tile_variant`
  (a hash of the cell, so a map always looks the same); a missing variant falls back to `<name>`.
- Facing: models face south (+z, Blender -Y). `models::face_prop_models` turns a prop on a wall
  tile (the wall torch) to its open side, the side `lighting::wall_offset` hangs the flame on,
  a door with walls north and south a quarter turn, and any other prop beside a wall turns its
  back to that wall (trying north, west, east, then south), so a cabinet or bed faces into the
  room (`model_yaw`). Creatures and items are not turned.
- Export: Blender sources live in `art/`. Each top-level collection becomes one `.glb` named
  after it: `blender art/<file>.blend --background --python art/export.py` (or run the script
  in a live Blender with `__file__` set). Collections named `_...` are skipped.
- Dungeon kit (A01): `art/build_dungeon_kit.py` builds every piece procedurally in
  `art/dungeon_kit.blend`; change a number there, rerun it, then `export.py`. It also lays out a
  `_preview` grid and a small room to judge the pieces together (the pieces stay at the origin,
  hidden in the viewport). Pieces: `tile_floor` (+ `_2`, `_3`), `tile_wall_0` to `tile_wall_15`
  (stones on faces without a wall neighbour, plain faces towards one), `tile_down_stairs`,
  `tile_up_stairs`, `door` / `door_open` and `wall_torch` (raws `model` on the Door and Wall
  Torch props). Three materials (stone, wood, iron); per-stone variation is a vertex colour
  (glTF `COLOR_0`) multiplied into the base colour. Pieces stay under about 260 triangles.
- Town kit (A02): `art/build_town_kit.py` builds every piece in `art/town_kit.blend` the same way,
  reusing the dungeon script's helpers with its own palette, plus a `_preview` town corner and a
  `_cam` set to the game's view. Pieces: `tile_grass`, `tile_road`, `tile_wood_floor` (each + `_2`,
  `_3`: tufts, pebbles, plank joints), `tile_shallow_water`, `tile_deep_water`, `tile_bridge`
  (a plank pier over deep water), `tile_house_wall_0` to `_15` (timber frame and plaster on a
  stone footing; the straight runs 3 and 12 have `_2` with a lit window and `_3` with braces),
  `tile_town_wall_0` to `_15` (stone courses with merlons) and `roof` (+ `_2`, `_3`, shingle
  courses). Terrain pieces are closed slabs down to -0.3 m, so a lower neighbour (water beside
  grass) shows a bank, not a gap; water sits at -0.08 (shallow) and -0.14 (deep). Nine materials;
  the window material is emissive. Pieces stay under about 270 triangles, terrain under 100. The
  door is the dungeon kit's `door`, and doorways and hovels keep the dungeon's `tile_floor`
- Props kit (A03): `art/build_props_kit.py` builds one model per prop in `art/props_kit.blend`,
  reusing the town script (and through it the dungeon helpers and the town's stone and wood),
  plus a `_preview` row and a `_cam` at the game's angle. Pieces (raws `model` on each prop):
  `bear_trap`, `keg`, `table`, `chair`, `candle`, `anvil`, `water_trough`, `weapon_rack`,
  `armour_stand` (the raws name is Armor Stand), `chemistry_set`, `dead_thing`, `cabinet`, `bed`,
  `loom`, `hide_rack`, `market_stall`, `lamp_post`; the door and wall torch stay the dungeon
  kit's. Each fits its tile and faces south. Heights follow `cover`: none-cover props stay low,
  half cover reaches 0.5 to 1.2 m, full cover 1.6 to 2.2 m. The game draws a lit prop's flame
  (`lighting.rs`, a teardrop whose 0.09 m round base is centred at the raws `light` height), so
  the candle's wax top sits at `CANDLE_FLAME - FLAME_BASE` and the lamp's burner plate just under
  `LAMP_FLAME - FLAME_BASE`; change those numbers in the script with the raws heights. Eight
  materials of its own (cloth, linen, leather, wax, steel, glass, flesh, blood) beside the town's
  stone, wood, iron and shallows (the trough's water); glass glows faintly. Props stay under
  about 380 triangles
- `art/make_test_model.py` writes `assets/models/test_pyramid.glb` (no Blender needed). Copy it to
  a used name, e.g. `tile_floor.glb`, to check the pipeline, and delete the copy afterwards.
- Every model is loaded at startup and kept. The harness starts the game only once they have
  all loaded, so model entities spawn in the same frame and order each run (byte-identical PNGs).
- Wall cut-away (`occlusion.rs`) scales the wall tile's `Transform` on y, so a wall model is
  squashed to a stub the same way as the primitive. The kit has no separate stub piece: the
  squashed stone courses read as a low ledge under the dark cap.
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
  each backpack item's owner by slot, and the `Delve` (deepest depth, levels since town)
- Not saved yet (P01): `Poison` on a unit, a mob's poison bite (`PoisonsOnHit`), and
  `Overwatch` and `Hunkered`
- Permadeath: save file deleted on load and when the last hero dies. A fallen hero leaves play
  and drops what they carried; the selection moves to the next living hero
- WASM: save/load disabled (no filesystem access)
- Vendors: clicking a vendor in town walks the active hero beside them and opens the
  shop (`ui/menus/shop.rs`, `RunState::ShowShop`). The trade rules are pure functions in
  `economy/shop.rs`; who buys and stocks what is `town::STALLS`. Shelves refill
  (`stock::restock`) when a new game starts in town and on every return to town. Sales
  feed the sector's stock.
- Town state (`economy/town.rs`) lives in its own `town.json`, loaded at startup
  and saved whenever `TownState` changes. Permadeath never deletes it. A bad or
  wrong-version file is renamed to `town.json.bad-<secs>` and a fresh town founded.

### Coordinate System

- `Position` component uses grid coordinates (not pixels) and stays the gameplay truth
- Map origin (0,0) is top-left; Y increases downward
- 3D: `map::grid_to_world` is the only grid-to-world mapping. Grid `(x, y)` is the tile centre at
  world `(x, 0, y)`, one tile is one world unit, the floor is `y = 0`, walls are 2 units tall
- The 3D camera is orthographic, yaw 45 degrees, pitch 35 degrees, and follows the logical
  `Camera` resource: a continuous focus (fractional grid x, y) and a view height. It glides to
  the active hero (exponential smoothing, `FOLLOW_TIME`) until held WASD pans it (`PAN_SPEED`,
  scaled by zoom, eased by `PAN_EASE_TIME`); that hero moving, selecting another, or C ends
  the pan. Leaving `PreRun` (new game, load, new level) snaps instead. Zoom eases the
  projection's `viewport_height` to a target (`ZOOM_EASE_TIME`). Every speed and the wheel,
  touchpad and pinch directions are named constants at the top of `camera.rs`. All of it is
  driven by `Time`, so harness captures repeat
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
  mesh is shared). Town buildings (closed `WoodFloor` rooms) get a roof over every tile, walls
  included, at wall height (the `roof` model, or a flat slab without it).
  A roof hides while any hero is inside the building or behind it; while it shows, its walls
  stay full height
- Click-to-move paths (`pathfinding::a_star_explored`) only cross explored tiles
- Tile overlays (hover, path preview, combat move range, targeting range, area, target) are
  `TileMarks::mark` quads just above the floor, never 2D sprites. Cover shields are upright
  `TileMarks::shield` quads on a tile's side (half low and amber, full tall and blue, unlit and
  double-sided), drawn by `show_path_preview` on the hovered destination of a combat move. The move range draws the one-AP
  zone blue (`move_one`) and the two-AP zone green (`move_two`), inset so the grid shows
- `translate_positions` moves a 3D entity (`EntityShape`) to `grid_to_world` plus its shape's lift
  whenever its `Position` changes. Tiles are placed once at spawn. Effects and bloodstains have
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
  past the fog of war) and, except for heroes, an emissive teardrop flame that blooms and
  stretches on the same flicker wave. A light on
  a wall tile hangs just off the wall face on the open side. Intensity scales with radius squared
- Flicker runs on `Time`. The harness sets `TimeUpdateStrategy::ManualDuration` (1/60 s per frame),
  so screenshots stay byte-identical. Don't drive visuals from wall-clock time any other way.
  The harness also compiles shaders synchronously, so frame N is the same picture on any machine
- `FRAME_TIME_LOG=1 cargo run --release` turns vsync off and logs frame times, for performance checks
- Only the `MAX_SHADOW_LIGHTS` point lights nearest the active hero cast shadows (cube maps are six
  passes each); the rest light through walls within their radius
- `apply_environment` switches on `Map::depth`: depth 0 (the town) gets a low warm directional sun
  with shadows and brighter ambient; dungeons get near-black ambient. Both get HDR, bloom,
  the default TonyMcMapface tonemapping and linear distance fog that darkens the far side of view
- `WallTorches` (`map_builders/wall_torches.rs`) is appended to every dungeon chain in
  `level_builder`; it hangs torches on one-sided walls, spaced apart

### Effects

- Gameplay asks for one with `ResMut<Effects>` and `effects.play(EffectKind, x, y)` at the target
  tile: `HitSpark` (attack and trap hits), `Fireball { radius }`, `MagicMissile { from }` (a bolt
  from the caster's tile, also every ranged weapon shot), `Healing`, `Confusion`
- `EffectKind::style` is the one table of how each kind looks: mesh, HDR colour, lifetime, piece
  count, `Motion` and an optional point light. `piece_transform` places every piece as a pure
  function of progress (0 to 1) and `envelope` sets the brightness, so new kinds are a table row
- Pieces share one unlit additive material per effect, whose alpha fades it; colours above 1
  bloom. Effects age on `Time` (fixed per frame in the harness) and despawn at the end of life
- Effects have no `Position`, so they show through the fog of war like their light

### Key Resources

- `Map` - Contains tile data, rooms, and visibility state
- `ActiveUnit` - The hero the player commands (`None` outside a run)
- `UiFont` - Shared font handle for all text rendering
- `HoveredTile` - The grid cell under the mouse this frame (see Coordinate System)

## Commit Style

Commits should be one line in the format: `feat: add [feature description]`
