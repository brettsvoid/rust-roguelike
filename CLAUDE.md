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

# Batch check: run every harness scene in tools/scenes.json (or the named ones), each in its
# own scratch folder, check the game log lines each expects, and build one contact sheet.
tools/scenes.py                  # all scenes: a pass/fail table, then sheet.png
tools/scenes.py combat cover     # only these ("abil*" matches a prefix); --list shows them all

# One harness run: a seeded new game, wait N frames, save a PNG, exit. The window is hidden
# and the picture is a fixed 1600x1000 off-screen image, so nothing shows on screen and it
# works with the screen locked. Depth 0 is the town, the dungeon is depths 1 to 12. It exits
# the process right after saving: tearing the renderer down could wait on the GPU for minutes.
# Defaults: --seed 42 --depth 0 --frames 60, fog on unless --reveal-map.
cargo run -- --screenshot out.png --seed 42 --depth 2 --reveal-map --frames 60
```

Desktop only: there is no web build. F09 built `wasm32-unknown-unknown` once (Bevy 0.20.1) and
it failed to compile: `getrandom` 0.4.3, pulled in through `rand` 0.10 by `bevy_math` and by the
game, refuses that target unless its `wasm_js` feature is on. The `wasm-release` profile and the
`target_arch = "wasm32"` branches (no-op save and an in-memory town) were removed with it.

### Verifying changes: batch, don't poll

A game run is the expensive step, so work in batches (the owner asked for this):

1. Write all the code and its unit tests first. Loop on `cargo check` and `cargo test`; do not
   launch the game to see each small change.
2. Describe what must be seen as scenes in `tools/scenes.json` (name, note, harness `args`,
   and the game log lines to `expect`, in order). Most rules show up in the log, so most
   checks need no picture.
3. Run the batch once: `tools/scenes.py <your scenes>`. Read the pass/fail table and the
   `.log` files first. Then open the contact sheet once (shrink it if it is big). Open a
   single capture only when the sheet shows something you must see closer.
4. Write down everything that is wrong, fix it all, and run the batch again. Three rounds
   should be enough; if not, say why in the report.
5. Before reporting, run the whole suite once (`tools/scenes.py`). Every scene must pass, and
   scenes whose log lines your change moved must be updated in the manifest, not deleted.

The coordinator runs the same suite before every merge.

### Harness flags

```text
--seed N --depth N --frames N --reveal-map
--menu             stay on the main menu and capture it (no game starts)
--visualise N      open the Map Visualiser on builder N of its menu (0 Simple Map, 3 Cellular
                   Automata) instead of a game; snapshots change every 30 frames
--console CMD      a debug console command, repeatable. "town ..." commands run before the
                   level is built. Useful: godmode, "place DX DY NAME" (any raws entry at an
                   offset from the active hero), "spawn <item>", "gold N", "hurt N",
                   "effect <spark|fireball|missile|heal|confusion|blood>", "teleport X Y", "teleport heart" (onto the Heart),
                   "town level <sector> <1-5>", "town days N", kill_all (every monster
                   falls as if killed: stand-in body, gold, "is dead", experience), kill
                   (the active hero falls), "kill party" (every hero: game over),
                   "xp N" (to every living hero), "depth N" (jump to a fresh level at depth N).
--do CMD           a console command queued with the inputs, run in its turn (e.g.
                   `--keys g --do "depth 1" --keys .`)
--click X,Y        left-click grid cell X,Y: walk, attack, select a hero, confirm a target
--press X,Y        left-click a point of the 1600x1000 picture (HUD buttons and cards)
--cursor X,Y       hover the mouse at a point of the picture
--keys CHARS       g pick up, . stairs, i inventory, r remove, t ledger, n Tab, space wait,
                   e end turn, f attack, o overwatch, h hunker, 1 2 abilities, w a s d pan,
                   c recentre, + - zoom, a to j a menu line, ! Escape (n is Tab, so in a
                   shop it switches tab)
--shop VENDOR      walk to a vendor and open the shop
--target ITEM      enter targeting with an item
--ledger           open the town ledger
--every N          also save every Nth frame as out-FFFF.png (a sequence for motion)
--reload           queued with the inputs: save, load the save back, log "Reload: <mode>; the
                   loaded game matches the saved one" or the first difference
--load             take Continue at the main menu instead of a new game; a scene's "files"
                   puts a savegame.json in its folder (a bad one leaves the main menu)
```

Inputs run in the order given, one at a time, while the game waits for input and nothing is
animating. The game log is printed as `log>` lines when the capture is taken. Two runs can
differ by a handful of isolated pixels with the same game state (see F10), so compare by eye
or by the log, not by hash. In the party phase the action buttons sit on y=944 at x=406
Attack, 498 and 590 the abilities, 682 Overwatch, 774 Hunker, 866 Use Item, 958 Wait, 1050
End Turn (exploring: 590, 682, 774, 866 for the abilities, Use Item, Wait); the party cards
are at x=150, y=889 + 30 per slot. The scenes in `tools/scenes.json` are the worked examples.


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
  that hero's turn), 1 and 2 use the active hero's abilities (a targeted one opens the
  targeting screen), E ends the party phase in combat, T opens the ledger in town, Q saves and quits
- **Mechanics**: Hunger/survival, items/equipment

The Architecture section below describes the code as it is now. The map and
entities are 3D models (grey-box primitives where a model file is missing) on
an isometric camera, with a party of four heroes, one of each class (fighter, ranger, mage,
cleric). Exploring, the active hero
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
  resource (the hero the input drives), `HEROES` (names, colours and raws class), and the pure rules
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
- **MonstersPlugin** (`monsters.rs`) - Monster AI, applying the pure rules of `enemy_ai.rs`.
  `Monster` requires `Alertness` (starts `Idle`) and `Archetype` (raws `archetype`, default
  brute). `look_round` updates every monster's alertness (`enemy_ai::spread_alert`) and logs
  the changes ("Goblin spots Aldric!", "Goblin is alerted.", "Goblin loses the party's
  trail."): `spot_party` runs it after every party action (in the `PartyAction` chain before
  `after_party_action`), `monster_ai` at the start of the enemy turn, where it also counts a
  turn of the hunt. Then only alerted monsters act. Exploring, one step or an attack per turn.
  In combat, `enemy_ai::plan` per AP: move to the best-scoring tile, attack (through
  `Strikes::strike`, at once), hunker, go on overwatch or wait. One that sees no hero hunts:
  a one-AP move at a time towards where it last saw one (a fleeing coward does not).
  `monster_target` picks the nearest hero a monster sees or stands next to (the one it spots)
- **GuiPlugin** (`gui.rs`) - Main menu and builder-select menu (other UI lives in `ui/`). The
  main menu ("The Abyss Below": New Game, Continue, Map Visualiser, Quit) takes its letter, the
  arrow keys and Enter, or the mouse; Continue is dimmed without a save. `MainMenuNotice` is an
  empty one-line `Text` under the entries for a notice. Behind it `menu_scene.rs` builds a fixed
  dungeon corner from the tile, prop and hero models with torches and a slow camera drift on
  `Time` (it overrides the follow camera while the menu shows; the HUD bar is hidden there)
- **HudPlugin** (`ui/hud.rs`) - The bottom bar: party cards (portrait, name, class, health, AP
  pips, status badges; a click selects the hero as Tab does), the action bar (with a button per
  ability of the active hero, named after it, its caption the hotkey or why it is refused,
  "Cooldown 2" while resting), depth/gold/mode and the log.
  A button writes the key it stands for to the `ActionRequest` resource and
  `player::handle_player_input` takes it after the real key presses, so a button and its
  hotkey share one code path. The rules (which buttons, why one is disabled, the shot
  preview numbers, which banner a phase change shows) are pure functions in
  `ui/combat_hud.rs`. Shared colours, sizes and the `ease` curve live in `ui/style.rs`, for every
  UI screen. A card's portrait disc
  shows the hero's level, and its name is the hero's `Name` (a recruit's own)
- **PanelPlugin** (`ui/panel.rs`) - The one modal panel look: inventory, drop, remove, shop,
  ledger, game over and the targeting banner. A screen builds a `PanelSpec` (title, aside,
  tabs, rows, body lines, message, footer hints) with a pure function and writes it with
  `panel::show`; `render_panels` redraws on change. A click on a row, tab or hint, or Enter on
  the row picked by hover or the arrow keys, writes the key it stands for as a `KeyboardInput`,
  so the screen's own key handler does the rest. Panels ease in on spawn and out after
  `close_panels` (each screen's `OnExit`)
- **TooltipPlugin** (`ui/tooltip.rs`) - Names what is under the cursor; in combat a hostile shows
  the shot preview (hit, crit, damage, cover or flank) from `combat::aim_at`
- **HealthBarsPlugin** (`ui/health_bars.rs`) - A bar over each monster the party sees and over a
  hero only when hurt or hovered (the cards show the rest), a UI node placed by projecting a point
  just above the unit's model through the `Camera3d` each frame. The height is the top of the
  unit's mesh bounds (`Aabb`), measured once its model has loaded; hidden with the unit
- **BannerPlugin** (`ui/banner.rs`) - The eased centred banner on a mode or phase change
- **LightingPlugin** (`lighting.rs`) - `LightSource` point lights, flicker, the shadow caster cap,
  and the per-depth ambient light, fog, bloom and town sun (see Lighting)
- **EffectsPlugin** (`effects.rs`) - Combat and spell effects (see Effects)
- **PresentationPlugin** (`presentation.rs`) - How the rules' results play out. The rules stay
  instant (`Position` changes at once); where things happen, typed `Beat`s are pushed onto the
  `Presentation` queue: `Move` (heroes in `player.rs`, every combat move and monster step in
  `overwatch::walk_under_watch`, cut at each reaction shot), `Lunge` / `Projectile` (the shooter
  draws for `DRAW_SECS`, then the bolt flies), `Floater` and `Flinch` (`combat::Strikes::strike`),
  `Cast` (`inventory::item_use_system`, any item or spell used; holds nothing), `Death` (`presentation::fall` from
  `combat::delete_the_dead`: the unit is despawned at once and a stand-in body inherits its queued
  beats, falls and fades), `CameraFocus` and `Wait` (`monsters::monster_ai`, round each monster
  that acts in combat). `play_beats` alone sets creature (`EntityShape::Creature`) transforms; a
  creature no beat names rests on its tile. Ordering (`holds`): a beat starts once every earlier
  beat has played its `hold`; a unit's beats never overlap; floaters and deaths hold nothing;
  a lunge holds until its blow lands; moves pushed while exploring overlap other units' moves,
  and a move that starts where the same unit's queued move ends extends it, so a click walk is one
  continuous glide (the walk takes its next step once the leader has `WALK_LEAD` tiles or less
  left to glide). In combat the map input waits while `Presentation::busy`, and any key or
  click finishes the playing beats. Every speed and duration is a named constant at the top of
  the file; the path, glide, ordering and floater rules are pure and unit tested.
  Body animation (`animation.rs`, registered by this plugin, A09) is procedural: `rig_model`
  finds each creature model's parts by node name once (`Part::named`), picks a body plan from
  them (`Plan`: biped, floater (wraith), quadruped (rat, hellhound), bat, spider) and puts a
  `Rig` on the model scene; `animate` (after `play_beats` and `turn_models`) sets each part's
  local transform to its rest pose plus `pose(plan, part, motion)` for the beat the unit plays
  (`Presentation::playing`: walk in step with the tiles glided, the blow landing half way
  through a lunge, a draw and release, a cast, a recoil, limp limbs in death), plus an idle
  layer (breathing, sway) that eases in when no beat plays, plus wings and hovering that never
  stop. Every motion is at rest at both ends, so motions hand over without a snap. All amounts
  are constants at the top of `animation.rs`; a rig hidden by fog is not posed
- **AbilitiesPlugin** (`abilities.rs`) - Classes and abilities. Raws `classes` (starting stats,
  `gear`, `abilities` by name) and `abilities` (`cooldown` and an `effect`: `"cleave"`,
  `{"aimed_shot": {"aim_bonus": N}}` or `{"spell": {...}}` with an item's consumable effect
  keys, plus `"targeting": "ally"` for a heal on a hero). `raws::spawn_class_kit` gives a
  hero `HeroClass`, the gear (first item per slot equipped, the rest in the backpack) and one
  child entity per ability with `HeroAbility` (slot, effect, cooldown, rounds `left`); a
  spell's entity also carries the scroll components, so it is cast as a `WantsToUseItem`
  naming it, through the targeting screen and `inventory::item_use_system`, never used up.
  `ability_system` (in the `PartyAction` chain before the item system) starts the cooldown,
  records the AP rule in `HeroAction` and resolves Cleave and Aimed Shot through
  `Strikes::strike`. One limit for all: the cooldown, in rounds, ticked down by
  `tick_cooldowns` at the start of the heroes' turn (after each enemy phase; exploring, each
  turn); used in round 1 with cooldown 3, ready in round 4. AP: an ability aimed at a hostile
  ends the turn like an attack (and is combat only); one on the party (Heal, Second Wind)
  costs 1 AP like an item and works exploring. Pure rules: `HeroAbility::target` (adjacent,
  own, weapon reach, hostile, ally, tile with radius), `HeroAbility::action`, `refusal`
  ("Combat only", "Cooldown", "No AP", "No target"), `has_target`, `cleave_targets`, `healed`.
  `update_active_abilities` writes `ActiveAbilities` (the active hero's abilities this frame)
  for the action bar and the 1 and 2 keys, so both decide alike. Aiming an Aimed Shot, the
  tooltip's shot preview adds the bonus

Non-plugin modules worth knowing:

- `map_builders/` - All procedural generation, organized as builder chains (see its module doc)
- `map_render.rs` - The one place map tiles get drawn. `spawn_map_tiles` spawns 3D tiles in game
  (looks in `tile_style`, shared meshes and a bright and a dim material per `TileType` in the
  `TileAssets` resource) and `Text2d` glyphs for the visualiser (`TileReveal::Preview`, looks in
  `tile_glyph`)
- `entity_render.rs` - 3D bodies for creatures, items, props and bloodstains (a flat splat mesh,
  turned per tile). An entity
  with `EntityShape` + `Renderable` (the raws colour, which tints the primitive) gets a shared mesh per shape and a
  shared material per colour from `EntityAssets` via the `attach_entity_mesh` observer. Spawn
  map entities with `RenderableBundle`; re-inserting `EntityShape` re-dresses (doors opening)
- `models.rs` - The model pipeline (see Models below)
- `experience.rs` - Experience and levels. Every hero has `Experience { level, xp }` (xp is a
  running total). A hostile carries `XpValue`: the raws mob `xp`, else `mob_xp` (half its max hp
  plus its power; a goblin 12). When one dies, `combat::delete_the_dead` sums them and runs
  `grant_xp`, which gives the full amount to every living hero ("The party gains 12
  experience."). `xp_for(level)` is `XP_STEP` (50) times 1+2+...+(level-1): 50, 150, 300, 500.
  Each level (`level_up`) adds a fifth of the class's base max hp (healed at once) and
  `LEVEL_AIM` (2) aim, logs "Aldric reaches level 2." and shows a "Level 2!" floater
  (`Outcome::LevelUp`)
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
  (Chebyshev) and `viewshed::line_of_sight` (Bresenham drawn both ways, both must be clear, so
  no shot is one-way; walls and `BlocksVisibility` block);
  `aim_at` combines both with `cover::exposure` (hunkered or not) and returns the chance and the `Exposure`. `roll_attack` rolls hit then crit, `roll_damage` rolls the weapon's
  damage + power (+1 well fed), +half rounded up on a crit, less armour, never below 0. All
  rolls go through `GameRng`. Every attack resolves in `Strikes::strike` (`Strikes` is a
  `SystemParam`; `Fighter` is a unit and the cell it attacks from or is hit on), which logs
  "Aldric shoots Goblin #0: hit for 5 (55%)", or "(35%, half cover)" / "(65%, flanked)"
  when cover applied (`odds`), and returns the damage dealt. `attack_system` resolves
  `WantsToAttack` (melee or ranged alike) through it. `Arms` (a `SystemParam`) gives a unit's
  equipped `Weapon` or `Weapon::NATURAL` (power -1..+1, range 1, 5% crit) and its worn armour.
  `CombatStats`: `armour` (raws and saves call it `defense`) cuts damage, `defence` cuts hit
  chance, `aim` defaults to `DEFAULT_AIM` (65). A monster's own `Weapon` component (raws mob
  `"weapon": "<items entry>"`, e.g. the Goblin Archer's and Orc Shaman's Shortbow) is its
  weapon when it holds none (`Arms::weapon`). `damage_system` alerts a hurt monster
- `enemy_ai.rs` - The monsters' tactics, pure rules over plain data. `Alertness` is `Idle` or
  `Alerted { last_seen, unseen }`. `spread_alert`: seeing a hero alerts (with that sighting);
  being hurt alerts (`combat::damage_system`; last seen is then the nearest hero); then every
  idle monster within `ALERT_RADIUS` (5, Chebyshev) of an alerted one is alerted too, chained,
  so a group is whoever stands that close. An alerted monster that sees no hero hunts towards
  where it last saw one and gives up (back to `Idle`) after `HUNT_TURNS` (3) of its turns
  unseen. `score_tile` scores a tile the monster can reach (`tactics::move_range`) or its own:
  `cover` (the aim penalty its cover puts on the worst shot of the heroes that see it, flanked
  -20, unseen 40), `flank` (it can flank a hero in reach from there), `distance` (path steps to
  the nearest hero off its `preferred`), `hit` (its best `aim_at` chance, only with AP left to
  attack), `overwatch` (reaction shots the walk there draws, `overwatch::reactions`) and `reach`
  (heroes within `THREAT_RANGE`, 5). `weights` per `Archetype` (cover, flank, distance, hit,
  overwatch, reach; preferred): brute 0, 0, -20, 1, 0, 0; 1. Skirmisher 1, 30, -5, 1, -40, 0;
  1. Ranged 1, 10, -5, 1, -40, -15; its weapon's optimal range. A coward with an alerted ally
  within `ALERT_RADIUS` and above a third of its hit points fights as a skirmisher with flank
  10; otherwise it flees: 1, 0, -10, 0, -40, -30; `FLEE_RANGE` (8). `plan` moves to the best
  tile (its own tile wins a tie, then the cheaper tile, then lower x, then lower y), or on it
  attacks the hero it hits most surely (a fleeing coward does not), else hunkers if in cover
  and fleeing or wounded (never a brute), else goes on overwatch if it sees a hero, else waits
- Weapons in the raws: `weapon { slot, damage: [min, max], crit_chance, range (default 1),
  optimal_range (default range) }`; `slot` "ranged" (the Shortbow: range 8, optimal 5) takes
  the weapon hand like "melee". Damage scrolls roll `spell_damage` (a quarter either side of
  their `damage`) and ignore armour; Magic Missile rolls to hit with aim +25 (`SPELL_AIM_BONUS`) less cover,
  area scrolls hit everything in the radius (heroes too), Confusion always lands. A healing
  item heals its user, or the unit on its target tile when aimed (Heal). The log names the
  caster ("Corin casts Fireball.", "Brenna uses Snare on Goblin, confusing them for 2 turns.")
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
  call `walk_under_watch` from `monster_ai`. Monsters hunker ("Goblin hunkers down.") and go on
  overwatch ("Goblin goes on overwatch.") by `enemy_ai::plan`. The HUD party row shows
  "Overwatch" or "Hunkered" after the AP
- `levels.rs` - New-game setup and level changes (`change_level`, run in `RunState::NextLevel`).
  The town is depth 0 (`TOWN_DEPTH`), the dungeon depths 1 to 12 (`DEEPEST`). Every dungeon
  level has up stairs where the party arrives (`map_builders::LevelStairs`, the last builder of
  every dungeon chain). Down stairs go one level deeper, or from town to the deepest depth
  reached; up stairs go one level up (`Travel::Ascend`), from depth 1 to town; a Town Portal
  scroll (`use_town_portal`, refused in town or while the party carries the Heart of the Abyss)
  goes to town. Levels are not kept: a level climbed into is built afresh (new layout, new
  monsters) and the party stands on its down stairs (climbing into town, on the dungeon entrance). Depth 12 has no down stairs: the raws item
  Heart of the Abyss lies where they would have been (the far end of the level), a Demon Lord
  beside it, unless the party already carries it. Reaching town carrying the Heart wins the run:
  `PendingVictory` sends the next `PreRun` to `RunState::Victory` (`ui/menus/victory.rs`, a
  summary panel; any key returns to the main menu) and the save file is deleted as on a death.
  `Delve` holds the deepest depth, the levels entered since leaving town and the levels entered
  this run; a return to town runs `trade::simulate` for one day per level entered, then
  `stock::restock`. Every entity with a `Position` that is not a hero is despawned with the
  level (carried items have none). `PendingTravel` is set by a portal or the console's `depth N`
  (a jump to a fresh level at depth N). The tooltip names stairs ("Stairs up", "Stairs up to
  town", "Stairs down") and the log says "You climb to level N." or "You descend to level N."
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
  damage and `Confusion` counts down; ability cooldowns tick down there too
- `ShowInventory` / `ShowDropItem` / `ShowRemoveItem` / `ShowTargeting` / `ShowLedger` / `ShowShop` - UI
  overlays; they return to `AwaitingInput` and leave `Mode` alone
- `NextLevel`, `MagicMapReveal`, `GameOver`, `Victory` (the run is won: the victory panel over the town), and the visualiser's `MapBuilderSelect` / `MapGeneration`

`Mode` (a resource) is the tactical mode: `Explore`, or `Combat { round, ap_spent }`. Every change
of mode or phase goes through `advance(mode, step, situation)`, a pure function over plain data
(the living heroes, and per hostile whether it sees a hero, a hero sees it and it is alerted), unit tested in
`game_flow.rs`. The `PartyAction` and `EnemyTurn` chains end in a system that calls it.
- Explore: each hero action is followed by `EnemyTurn` (one turn per step); the leader walks a
  click path and the others follow
- Explore to combat: a hero sees a hostile that is aware of the party (sees a hero, or is
  alerted: `Sight::alerted`). Combat starts at the party phase of round 1
- Combat party phase: each hero has `AP_PER_ROUND` action points, spent in any order across
  heroes (`Mode::ap_left`, `Mode::can_act`). The map input records the `Action` it took in the
  `HeroAction` resource and `Step::PartyActed(hero, action)` spends it; the inventory and
  targeting screens leave it unset, which counts as an item (1 AP), unless an ability is used
  (`abilities::ability_system` sets its own rule). A hero with AP left stays
  active; one out of AP hands over to the next hero who can act. The phase ends when no living
  hero has AP left or on E, then `EnemyTurn` is the enemy phase
- Status effects under AP: poison bites at the start of the unit's turn (Giant Spider melee hits
  poison, raws `poison: [damage, turns]`); a confused unit's move goes to a random adjacent tile
  (`confused_step`, rolled through `GameRng`) and still costs the AP of the move it chose
- Combat to explore: no hostile is aware of the party: none sees a living hero and none is
  alerted. Stepping out of sight does not end a fight while the monsters hunt; it ends once
  they give up (`enemy_ai::HUNT_TURNS`) or fall
- Game over: no hero left alive
- `Mode` is saved: a game saved in combat loads in the same round with the same AP spent

### Models

Drop a glTF binary in `assets/models/<name>.glb` and the matching primitive turns into it,
with no code change. A missing file keeps the grey-box primitive, silently (`AvailableModels`
scans the folder once at startup, so restart after adding a file).

- Mobs, items and props: set `"model": "<name>"` in the entry's `renderable` in
  `raws/spawns.json`, beside `fg` (the raws `glyph` is no longer read by the game). An open door looks for `<name>_open`. Heroes: the
  `"model"` of their class in the raws `classes`.
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
  room (`model_yaw`). Items are not turned; creatures turn to their way (see Facing below).
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
- Characters kit (A04): `art/build_characters_kit.py` builds one model per hero class and town
  character in `art/characters_kit.blend`, reusing the props script's helpers, plus a `_preview`
  line-up and a `_cam`. Heroes: `hero_fighter` (plate, sword, shield), `hero_ranger` (hood, bow,
  quiver), `hero_mage` (robe, pointed hat, staff), `hero_cleric` (tabard with a sun cross,
  mace), named by the raws `classes` entry's `"model"` (`raws::spawn_class_kit` inserts it, so a
  loaded hero gets it too; no file keeps the capsule). Each hero's accent (tabard, hood and
  cloak, robe and hat, tabard and mantle) is baked in the HUD colour of the default party's
  hero of that class (`party::HEROES`); change both together. Townsfolk (raws `model` on each
  mob): `barkeep`, `patron`, `priest`, `blacksmith`, `clothier`, `alchemist`, `mom`, `peasant`,
  `enchanter`. Rigged for animation (A09, `animation.rs`): each model is separate objects
  `<model>_head`, `_torso`, `_arm_l`, `_arm_r`, `_leg_l`, `_leg_r` and a held item (`_sword`,
  `_bow`, ...), each with its origin at its joint (neck, hips, shoulder, hip joint, grip), and the
  export keeps them as separate glTF nodes. `Figure.finish` nests them: a held item under its arm,
  the head, arms, wings and tail under the torso (a beast's body); torso and legs at the top, so
  the feet stay planted when the torso leans. Keep part names when adding a model. About 1.55 m to the crown, hats to 2 m, 360 to 560
  triangles. Colour is mostly one white `dye` material tinted by the vertex colour (`"dye.red"`
  in the script is the `dye` material with `DYES["red"]`); `humanoid` and the hat and item
  helpers are there for the monsters (A05)
- Monsters kit (A05, A08): `art/build_monsters_kit.py` builds one model per monster in
  `art/monsters_kit.blend`, importing the characters script for `humanoid`, `Figure`, the hat and
  item helpers and the `dye` tints (it adds its own, and two glowing materials, `ember` and
  `ghost`, for eyes, jaws, the flaming sword and the lich's orb). Raws `model` on each mob: `rat`,
  `kobold`, `goblin`, `hobgoblin`, `orc`, `orc_warrior`, `orc_shaman`, `skeleton`, `wraith`,
  `lich`, `imp`, `hellhound`, `demon_lord`, `giant_bat`, `giant_spider`. Humanoids keep the
  characters' parts; beasts are a `Beast` (a `Figure` with no parts) built from `rod` (a prism
  between two points), with parts `body`, `head`, `leg_fl`/`_fr`/`_bl`/`_br`, `tail`, `wing_l`/`_r`
  and `leg_l1` to `_r4` (the spider). Size tells rank, from the rat (0.55 m) to the demon lord
  (2.9 m to the horns); the wraith and bat hover. 170 to 530 triangles. Colours are dark and
  saturated (the torch washes pale ones out); silhouettes do most of the telling apart
- Items kit (A06, A08): `art/build_items_kit.py` builds one model per raws item in
  `art/items_kit.blend`, importing the monsters script and reusing the held weapons and shields
  (`sword`, `bow`, `axe`, `big_axe`, `hammer`, `mace`, `staff`, `shield`; `recoloured` swaps
  their materials, e.g. steel to gold). The raws `model` is the item name in snake case
  (`Alchemist's Fire` is `alchemists_fire`), `heart_of_the_abyss` for the Heart. Each `Item` is rested on the floor, centred and grown by
  `GROW` (1.25) to at most `MAX_SPAN` (1.05 m) at `finish`. Long things lie along the north-east
  diagonal, across the camera's view; shields lean towards the camera. Potions, the Heart, the
  portal seal and magic trims glow; steel here is brighter and not metallic, as metal with
  nothing to reflect reads near black. 32 to 190 triangles. A loaded game respawns each
  item from the raws, model included
- Facing: a creature's model scene (not the entity, so a hero's torch stays put) turns to the
  way it last moved, struck or was struck from, eased over `presentation::TURN_SECS`
  (`presentation::heading`, `Facing`, `turn_models`)
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

- `saveload.rs` - Saves the game to `savegame.json` (serde) on quit (Q, `save_on_quit`, an
  exclusive system); Continue on the main menu loads it (`continue_game`)
- Every entity is saved by what it was spawned from plus the state that changes in play, and
  loaded by spawning it afresh and writing that state over it. Heroes (`SavedHero`): slot,
  name, raws class (the model and abilities are spawned from it, `raws::spawn_class_abilities`,
  never the starting gear), position, `CombatStats`, `HungerClock`, `Experience`, status and
  each ability's cooldown. Mobs, bystanders and vendors, props (doors, traps, torches) and items
  (`SavedThing`): the raws name (a mob's name less ` #id`), the shown name, where it is (on
  the ground, in a hero's backpack, or worn by a hero in an `EquipmentSlot`), and when present
  `CombatStats`, `Alertness`, `Gold`, an open door, `Hidden`. So weapons, archetypes,
  `PoisonsOnHit`, raws `xp`, models, lights and cover come back from the raws. Status on both:
  `Poison`, `Confusion`, `Overwatch`, `Hunkered`. Also saved: the map (tiles, explored tiles,
  depth, bloodstains), `Mode` (round and AP spent by slot; `game_flow::KeepMode` makes the next
  `PreRun` keep it), the active hero's slot, the purse, `Delve` and the game log
- `snapshot(world)` lists every named entity's gameplay values and the type of every component
  it has, by name not `Entity` id. The round-trip test saves a world mid-combat, loads it into
  a fresh world and compares snapshots, so a component the save forgets shows up as a
  difference. The harness's `--reload` does the same in a running game ("Reload: ... the loaded
  game matches the saved one"). Component names need `--features bevy/debug`; without it the
  difference shows type ids
- Versioned (`SAVE_VERSION`). A save that is unreadable, from another version or naming a
  class the raws lack is moved to `savegame.json.bad-<secs>` and reported in the game log and
  on the main menu (`save_status`, checked when the menu opens); the menu then has no Continue
- Not saved: the camera, presentation beats, a walk under way, the targeting or a menu open
  (a loaded game starts waiting for input); visible tiles and viewsheds (recomputed); a lit
  effect
- Permadeath: save file deleted on load, when the last hero dies and when the run is won. A fallen hero leaves play
  and drops what they carried; the selection moves to the next living hero
- Vendors: clicking a vendor in town walks the active hero beside them and opens the
  shop (`ui/menus/shop.rs`, `RunState::ShowShop`). The trade rules are pure functions in
  `economy/shop.rs`; who buys and stocks what is `town::STALLS`. Shelves refill
  (`stock::restock`) when a new game starts in town and on every return to town. Sales
  feed the sector's stock.
- Recruits: the barkeep's shop has a third tab, Hire (Tab cycles Buy, Sell, Hire). It
  offers one recruit per class (`economy::shop::recruits`, names from `RECRUITS`, the first
  not in the party) at the provisions sector's level (1 to 5) for `RECRUIT_PRICE` (60) gold a
  level. `shop::hire` refuses with "The party is full." or "You cannot afford ..."; a hire
  fills the lowest empty slot (`spawner::spawn_recruit`: the slot's colour, the recruit's
  name, class stats raised to its level, then `spawn_class_kit`) beside the active hero
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
  the pan. `Camera::look_at(Some(cell))` glides to a point instead, overriding a pan, until
  `look_at(None)`: the presentation uses it for a monster acting off-screen in combat (more than
  `CAMERA_FOCUS_MARGIN` of the screen in from an edge counts as on screen) and releases it once
  the queue is idle, so the view glides back to the active hero. Leaving `PreRun` (new game,
  load, new level) snaps instead. Zoom eases the
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
  whenever its `Position` changes, except creatures, which `presentation::play_beats` moves. Tiles are placed once at spawn. Effects and bloodstains have
  no `Position` (they are not on the tile for gameplay) and get their transform at spawn
- 2D: `translate_glyphs` places the visualiser's `Text2d` tiles in screen pixels, the whole
  map from the window's top left (it ignores the game camera). Nothing in play is drawn in 2D

### Fog of War

- Tiles: `Revealed` is `Hidden` (not drawn), `Explored` (dim, desaturated, unlit material, so
  remembered tiles stay readable in the dark) or `Visible` (lit material);
  `update_tile_mesh_visibility` applies it
- Vision is the union of every living hero's `Viewshed`: it fills `Map::visible_tiles` and the
  revealed tiles
- Entities other than heroes are shown only while the tile their body is over (their
  `Transform`, so a gliding monster) is in `Map::visible_tiles` (`update_renderable_visibility`); `Hidden` (unfound traps) always hides
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
  from the caster's tile, also every ranged weapon shot), `Healing`, `Confusion`. Attacks play
  theirs from the presentation queue instead (a `Projectile` beat plays the bolt, a `Flinch` the
  spark), so they show in order with the bodies
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
