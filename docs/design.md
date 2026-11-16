# The Abyss Below - Game Design Document

_An isometric 3D tactical roguelike built in Rust with Bevy_

---

## Overview

**The Abyss Below** is a turn-based roguelike. A party of heroes descends
through procedurally generated dungeons to retrieve the legendary **Heart of
the Abyss**, then fights its way back to the surface to claim victory.

The game is seen in isometric true 3D. Dungeons are dark, ominous and lit by
torches. Fights use XCOM-style squad tactics. Between delves the party returns
to a town that trades its loot with neighbouring towns and grows over many
runs.

This is a living document. The work that gets the game here is the ticket list
in [TODO.md](TODO.md). Ticket IDs (for example C03) in this document point
there. Items marked **default** are assumptions the owner can change.

---

## Pillars

1. **Isometric true 3D.** Real meshes, real lights and shadows, seen through
   an orthographic isometric camera. Dungeons are dark and ominous, lit by
   torches and candles.
2. **A party and tactical combat.** The player controls a party of heroes.
   Combat borrows from XCOM: action points, hit chances, cover and flanking,
   overwatch. Only the fighting borrows from XCOM.
3. **Free exploration, turn-based fights.** Outside combat the party moves
   freely by click. The game drops into turn mode when hostiles and the party
   become aware of each other.
4. **The living town.** The party sells loot in town. The town trades it with
   neighbouring towns, its sectors level up, and it grows and stocks better
   weapons, items and magic. The town persists across permadeath, so every
   run, win or lose, advances it. The town takes the place of XCOM's home base.
5. **Point and click.** The mouse moves, attacks and selects. WASD pans the
   camera.

---

## Story & Setting

### Theme

Classic fantasy dungeon crawling with orcs, goblins, undead, demons and
magical artefacts.

### Premise

Beneath the ruins of an ancient fortress lies the Abyss: a labyrinth of
caverns, forgotten halls and dark realms that descend into the depths of the
world. At the very bottom rests the **Heart of the Abyss**, an artefact of
immense power that has corrupted everything around it.

A small town stands near the entrance. Adventurers set out from it seeking
glory and riches. Their goal: descend to the bottom, claim the Heart, and
escape alive.

### Tone

Dark but not humourless. Danger lurks around every corner, but moments of
discovery and triumph provide relief. The town is the light the party comes
back to.

---

## Core Gameplay

### Genre

- Turn-based tactical roguelike
- Permadeath: the run ends when every party member is dead
- Procedurally generated levels
- Persistent progression through the town only

### Core Loop

1. Set out from town with a party of heroes
2. Explore a dungeon level by clicking to move
3. Fight in turn mode when the party meets hostiles
4. Manage resources (health, hunger, consumables, gold)
5. Find loot, descend deeper, or return to town
6. In town: sell loot, read the news of what the town traded, buy better gear
7. Delve again, deeper each time
8. Retrieve the Heart of the Abyss at the bottom and escape to the surface

### Key Mechanics

- **Combat**: Squad tactics with action points, hit chances, cover and
  overwatch
- **Exploration**: Click-to-move, fog of war, hidden secrets, varied dungeon
  layouts
- **Survival**: Hunger per party member, limited healing, resource scarcity
- **Items**: Equipment, consumables and magical artefacts, with values and
  tiers
- **The town**: Trade, sector levels and stock that improve across runs

---

## The Party

### Party Size

**Default:** the party has 4 heroes. The run ends when every member is dead.

The player selects a unit by clicking it or with Tab. The party sees what any
member sees: vision is the union of all members' viewsheds. Inventory,
equipment and hunger are per member.

### Classes

Four classes, each with starting stats, gear and one or two abilities (C10):

| Class   | Example ability |
| ------- | --------------- |
| Fighter | Cleave          |
| Ranger  | Aimed shot      |
| Mage    | Fireball        |
| Cleric  | Heal            |

A new game starts with one of each class. Existing scroll effects are reused as
ability effects. The exact stats and abilities are open until C10.

### Experience and Recruits

Kills give experience, and levels raise stats. A dead hero is replaced by
hiring a recruit at the pub for gold. Recruit quality rises with the town's
level (C11). The experience curve and recruit prices are set by the balance
pass (see [Balance](#balance)).

---

## Exploration and Combat Modes

The game has two top-level modes (C02).

**Explore mode.** A click moves the whole party. Followers path behind the
leader. Time advances per step. Movement stops when a hostile comes into view.

**Combat mode.** Combat starts when a hostile and a party member see each
other. Rounds alternate a party phase and an enemy phase. Combat ends when no
aware hostile remains, and the game returns to explore mode.

---

## Combat System

### Action Points

**Default:** each unit has 2 action points (AP) per round, and a `movement`
stat (C03).

| Action    | Cost                   |
| --------- | ---------------------- |
| Move      | 1 AP                   |
| Dash      | 2 AP                   |
| Attack    | Ends the unit's turn   |
| Use item  | Open until C03         |
| End turn  | Ends the unit's turn   |

Units act in any order within the party phase. The move range is drawn as a
one-AP zone and a two-AP zone. Monsters spend AP by the same rules.

### Hit Chance

Attacks roll to hit (C04). The hit chance comes from the attacker's aim, the
target's defence, range, cover and height. Weapons have a damage range, range
bands and a crit chance. Melee, bows and spells use the same rules. Line of
sight decides who can be targeted. The numbers are set by the balance pass
(see [Balance](#balance)).

### Cover and Flanking

Walls and tall props give full cover. Low props give half cover. Cover is per
direction. A flanked target gets no cover and the attacker gets a crit bonus.
The cover shield shows on the hovered destination tile (C05).

### Overwatch and Hunker Down

Overwatch spends the unit's turn. The unit fires at the first hostile that
moves in view during the enemy phase, with an aim penalty. Hunker down doubles
cover until the unit's next turn. Enemies can use both (C06).

### Status Effects

Poison, confusion and similar effects carry over from the current game. How
they interact with AP is open.

### Enemy Behaviour

Monsters wait in groups until alerted. Once active they score candidate tiles
by cover, flanking, distance and hit chance, and spend AP on the best move and
action. Each monster has an archetype: brute, skirmisher, ranged or coward
(C07).

### Death and Loot

A dead monster drops its loot. Monsters also drop gold (E01).

---

## The Living Town

The town is the starting location and the party's home between delves. It
persists across permadeath. Its state is saved in its own file and is never
deleted when the party dies (E03).

### Vendors

The blacksmith, alchemist, clothier, barkeep and priest are vendors. Each buys
and sells its own item categories. The sell price is a fraction of the buy
price (E02).

### Trade and Sectors

The town has sectors: smithing, alchemy, textiles, provisions, faith and
arcana. Each has a goods stock and a level (E03).

Goods the party sells enter the matching sector's stock. Each day, caravans
carry surplus to a few neighbouring towns with their own demands and return
gold and materials. Sector progress rises with trade volume, and a sector
levels up at thresholds. Time passes while the party is in the dungeon.
Prices move with stock, so flooding one good lowers its sell price (E04). The
thresholds are set by the balance pass (see [Balance](#balance)).

A news log reports what the town traded while the party was away. A ledger
screen shows the town's level, each sector's progress to its next level, what
it will unlock, trade routes and the recent news (E08).

### Growth

Sector level decides which item tiers each vendor stocks. A fresh town sells
only tier 1. Higher levels unlock new vendors, such as an enchanter once
arcana reaches a threshold (E05).

The town grows visibly. Abandoned buildings are repaired, hovels become
houses, and new buildings, market stalls, townsfolk and lights appear. The
layout stays the same between visits (E06).

### Returning to Town

**Default:** the up stairs on the first dungeon level and a Town Portal scroll
bring the party back to town. Leaving town returns the party to the deepest
depth reached, on a freshly generated level. Each return runs the trade
simulation for the days spent away (E07).

---

## Level Progression

### Dungeon Depth: 12 Levels

| Depth | Name              | Theme           | Key Features                         |
| ----- | ----------------- | --------------- | ------------------------------------ |
| 1     | Ruined Cellars    | Tutorial area   | Easy enemies, basic loot             |
| 2-3   | Limestone Caverns | Natural caves   | Goblins, rats, bats                  |
| 4-5   | Forgotten Mines   | Dwarven ruins   | Orcs, traps, better equipment        |
| 6-7   | Fungal Depths     | Mushroom forest | Poison, confusion, strange creatures |
| 8-9   | Sunken Temple     | Flooded ruins   | Undead, curses, holy artefacts       |
| 10-11 | The Dark Realm    | Abyssal plane   | Demons, elite monsters               |
| 12    | Heart Chamber     | Final level     | Boss encounter, the Heart            |

In the code the town is depth 0 and the dungeon runs from depth 1 to 12, as
in this table. Dungeon loot is tiered by depth (E05).

### Return Journey

The Heart lies at the far end of depth 12, guarded by a Demon Lord; that level
has no stairs down. A Town Portal will not work while the party carries the
Heart, so the party climbs: every dungeon level has up stairs where the party
arrives, and each level climbed into is built afresh, the party arriving on
its down stairs. Carrying the Heart up the stairs of depth 1 into town wins the
run. Harder levels on the way up are not built yet.

---

## Balance

The balance pass (P04) tuned the numbers with a simulator, not by feel. Every
number below can be measured again with the same command.

### The simulator

`src/balance.rs` is a test-only module. It plays whole runs with no window:

```bash
export CARGO_TARGET_DIR=target   # or your own
cargo test --release balance::report -- --ignored --nocapture        # 200 campaigns of 10 runs
BALANCE_CAMPAIGNS=50 cargo test --release balance::report -- --ignored --nocapture
cargo test --release balance::staged_fights -- --ignored --nocapture # the harness check
```

A campaign is 10 runs, one after another, on one town. Run 1 is a first-time
party in a fresh town. Runs 6 to 10 are a party with a levelled town. All rolls
go through a seeded `GameRng`, so a report repeats exactly.

What the simulator takes from the game, unchanged:

- The level generators and their spawns: `map_builders::level_builder`,
  `spawner::room_spawns` and every builder's spawn list. It builds 40 levels per
  depth and each run draws from them.
- The alert groups: hostiles within `enemy_ai::ALERT_RADIUS` of each other wake
  together, so each such group is one fight.
- The rules: `combat::hit_chance`, `roll_attack`, `roll_damage`,
  `spell_damage`, the cover penalties and crit rules, `experience` (experience,
  levels, level gains) and the half heal on the stairs.
- The data: classes, gear, abilities, monsters, items and the spawn table from
  `raws/spawns.json`.
- The economy: `shop::sell`, `shop::buy`, `shop::recruits`, `trade::simulate`,
  `stock::restock` and the sector thresholds.

What it simplifies:

- A fight has no map. Both sides attack every round. In round 1 a melee unit
  reaches its foe with a 50% chance. Ranged units shoot from their weapon's
  optimal range. A group of more than 6 arrives in waves of 6, with no rest
  between waves.
- Cover is a draw per attack, not a position. A hero is flanked by a
  skirmisher or a coward 30% of the time, and in full cover against a brute or
  an archer 25% of the time. A ranged hostile is in full cover 50% of the time,
  a skirmisher or a coward 25%, a brute never.
- Overwatch, hunkering, movement, line of sight, traps and hunger are not
  modelled. Monsters pick a hero at random; melee monsters pick the fighter or
  the cleric three times as often.
- Fireball hits 2 hostiles and no hero. Cleave hits 2. Snare costs its target
  one turn.
- The party policy: focus the hostile with the fewest hit points; heal a hero
  below half health; drink a potion below 30%; use abilities when ready.
- Between fights the party rests to full health. (In the game, Space heals the
  active hero 1 hp a turn when no hostile is in view.)
- Going down, the party clears every group on the level and picks up every
  item. Climbing with the Heart, it fights only the groups within 6 tiles of
  the path between the stairs.
- Loot: a hero wears gear that is better than theirs, potions are kept for
  fights, food is kept, and everything else is sold. The party goes home by
  Town Portal after 2 levels, or after it loses a hero, if it has a portal. In
  town it sells, hires recruits for the fallen, then buys a portal, potions and
  gear upgrades.

`balance_lord` and `balance_d3` in `tools/scenes.json` stage two fights in the
real game, and `staged_fights` simulates the same fights. See
[Harness check](#harness-check).

### Targets

These were set before the tuning started:

1. A first-time party in a fresh town usually reaches depth 3 to 5 and rarely
   wins (less than 10% of runs).
2. A party with a levelled town wins 20% to 35% of runs.
3. No depth is a cliff. At no depth do more than a third of the parties that
   arrive die there.
4. Every class pulls its weight: each does at least 15% of the party's damage
   and healing.
5. The first sector level-up comes within two or three runs, in at least 85%
   of campaigns.
6. Recruits stay affordable: a party that comes home short of a hero can hire
   one almost every time.

### Before and after

Measured with the simulator: 200 campaigns before, 300 after.

| Measure | Before | After |
| --- | --- | --- |
| First run: wins | 100% | 8.0% |
| First run: mean deepest depth | 12.0 | 6.8 |
| First run: survival per depth, lowest | 100% (all) | 71% (depth 9) |
| Levelled town (runs 6 to 10): wins | 100% | 20.9% |
| Levelled town: survival per depth, lowest | 100% (all) | 81% (depth 12) |
| Demon Lord guard beaten (levelled town) | 100% | 87% |
| Mean party level on arrival at depth 2, 6, 12 | 4.8, 14.1, 27.1 | 1.5, 3.4, 5.8 |
| Hostiles per level (mean over depths) | about 53 | about 32 |
| Gold per first run (pickups and sales) | 72,712 | 7,987 |
| Damage and healing: fighter, ranger, mage, cleric | 38%, 6%, 34%, 23% | 28%, 19%, 33%, 20% |
| First sector level-up by run 1, 2, 3 | 100%, 100%, 100% | 60%, 84%, 97% |
| Recruits hired for heroes found fallen in town | none fell | 1,933 of 1,934 |
| Staged: level-1 party against a Demon Lord, wins | 0% | 11% |

Before the pass the party could not lose. Two things made it immune. Flat
armour: a hero in found tier-1 gear took no damage from depth-1 monsters, and
at depth 12 a full set gave 13 armour against a Demon Lord's 14 to 16 per hit.
And the experience flood: about 50 hostiles a level, with 50 experience to a level,
took the party to level 27 by depth 12, and every level adds a fifth of the
class's hit points.

First run after the pass, per depth (measured):

| Depth | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Reach | 100% | 84% | 80% | 76% | 70% | 67% | 59% | 54% | 43% | 20% | 14% | 12% |
| Survive | 84% | 98% | 97% | 96% | 98% | 94% | 96% | 91% | 71% | 87% | 93% | 86% |
| Heroes lost | 0.72 | 0.13 | 0.15 | 0.28 | 0.14 | 0.38 | 0.32 | 0.61 | 1.37 | 0.93 | 0.51 | 1.06 |
| Mean level | 1.0 | 1.5 | 2.0 | 2.5 | 3.0 | 3.4 | 3.9 | 4.2 | 4.7 | 5.1 | 5.5 | 5.8 |

The climb back loses few parties: 89% to 100% survive each level going up.

Target status (measured): 2, 3, 4, 5 and 6 are met. Target 1 is half met. A
first run wins 8% of the time, but it reaches depth 6.8 on average, not 3 to 5.
The reason is structural: every run starts with four level-1 heroes and no
gold. So the first trip of a run in a levelled town is the same as the first
trip of the first run, and the town can only help after the first return. Any
data that kills first runs at depth 3 to 5 also kills levelled-town runs
there, and target 2 is lost. Closing the gap needs a rule: for example, a new
party could start with gold from the town's treasury, or at the recruits'
level. That is the owner's call.

### What changed and why

All measured with the simulator in rounds; each round's report is reproducible
with the command above.

- **Experience per level** (`experience::XP_STEP`): 50 to 800. Level 2 now
  needs 800 experience, level 3 2,400, level 4 4,800. At 50 the party was level
  5 when it reached depth 2.
- **Spawns per room** (`spawner`): a room rolled 1 to 4 + depth points for
  monsters and 1 to 2 + depth for items. Now it rolls 1 to 2 + depth / 3 and 1
  to 1 + depth / 3 (`MAX_MONSTERS` 2, `MAX_ITEMS` 1, `DEPTHS_PER_SPAWN` 3). This
  cut hostiles per level from about 53 to about 32, and the loot and gold with
  them. Builders that spawn by region are unchanged.
- **Found gear is a third as common**: the spawn weight of every entry that is
  not a weapon, a shield or an armour is three times what it was; gear weights
  are unchanged. With found gear common the town's shops did not matter.
- **Armour on items**: Tower Shield 3 to 2, Kite Shield 4 to 2, Ward of the
  Faithful 4 to 2, Spellguard Buckler 5 to 3, Leather Armour 2 to 1, Studded
  Leather 3 to 2, Scale Mail 3 to 2, Spidersilk Robe 4 to 3, Plate Armour 4 to
  3, Mithril Plate 5 to 3, Elven Cloak 2 to 1, Shadow Cloak 3 to 2. A full set
  now gives 8, not 13. Armour comes off every hit, so it counts many times over.
- **Monster power** (damage per hit): Rat 1 to 7, Kobold 2 to 8, Giant Bat 3 to
  9, Giant Spider 4 to 10, Goblin 4 to 11, Orc 4 to 12, Goblin Archer 1 to 7,
  Hobgoblin 5 to 14, Orc Shaman 7 to 13, Orc Warrior 6 to 18, Skeleton 7 to 18,
  Wraith 9 to 18, Lich 11 to 19, Imp 9 to 19, Hellhound 12 to 22, Demon Lord 15
  to 25. Damage is power less armour, so power must sit above the armour of
  the gear found at that depth.
- **Monster hit points**: Hobgoblin 20 to 26, Orc Warrior 24 to 32, Orc Shaman
  18 to 24, Skeleton 26 to 34, Wraith 30 to 39, Lich 45 to 58, Imp 24 to 32,
  Hellhound 40 to 52, Demon Lord 70 to 90.
- **Monster armour**: Skeleton 3 to 2, Wraith 3 to 2, Lich 4 to 2, Imp 3 to 2,
  Hellhound 4 to 3, Demon Lord 6 to 4. Hero damage does not grow with level,
  so at 6 armour a longsword did almost nothing to the Demon Lord.
- **Ranger power**: 2 to 6. The shortbow is the only bow, so the ranger's
  share fell to 6% before; now 19%.

Not changed, because the measurements already met the targets: hit chances
(`DEFAULT_AIM` 65, cover 20 and 40, the range penalty), sector thresholds
(250, 600, 1,200, 2,000; raising the first to 400 moved the first level-up
by run 1 only from 61% to 56%), sell and buy percentages, item prices and the
recruit price (60 gold a level).

### Harness check

The staged fights in the real game (`tools/scenes.py balance_lord
balance_d3`; level-1 heroes, godmode off, every hero presses F) against
`staged_fights` (2,000 trials):

- `balance_xp`: kill_all on whole levels in the real game gave 24 to 444
  experience at depth 1 (six seeds) and 12 to 804 at depth 6 (five seeds), before
  the pass.
  The simulator's levels give the same spread: a fifth of levels have under 20
  hostiles and most have 40 to 100 (before the pass).
- `balance_lord`, before the pass: after about 10 rounds the real Demon Lord
  had taken about 16 damage and killed Dagny; the simulator expected about 1.5
  damage a round from the party and a wipe every time. After the pass the
  simulator gives the party 11% to win, 3.8 heroes lost, about 11 rounds. The
  one real fight (seed 42, 850 frames) ran 10 Demon Lord turns: it killed Dagny
  and Aldric and took 83 of its 90 hit points, about 8 a round against the
  simulator's 7. One fight is one sample; it agrees in kind.
- `balance_d3`: a hobgoblin and an orc beside the party. The simulator
  expects the party to win every time in about 4 rounds, losing 0.1 heroes.
  The real fight was won in 3 rounds with no hero lost.

Hit chances and damage in the real logs match the formulas the simulator
calls (for example "Demon Lord attacks Dagny: hit for 13 (65%)").

### Where the simulator is weakest

- No map: cover, flanking and who reaches whom in round 1 are fixed draws. The
  real cover share is not measured.
- Full rest between fights and a full clear of each level are a careful
  player's choices. A careless player dies sooner.
- Hunger, traps, scrolls in fights, overwatch and hunkering are left out.

---

## Characters

### Monsters

| Category   | Examples                      | Depth Range |
| ---------- | ----------------------------- | ----------- |
| Vermin     | Rats, bats, spiders           | 1-3         |
| Goblinoids | Goblins, hobgoblins           | 2-5         |
| Orcs       | Orc warriors, shamans         | 4-7         |
| Undead     | Skeletons, wraiths, liches    | 6-9         |
| Demons     | Imps, hellhounds, demon lords | 8-12        |
| Bosses     | Unique per-level bosses       | Various     |

### NPCs

- Townsfolk: barkeep, patron, priest, blacksmith, clothier, alchemist, mom,
  peasant. Some are vendors.
- Prisoners to rescue in the dungeon (optional quests)
- Ghosts with hints and lore

---

## Items & Equipment

Every item has a value and a tier (E01). Higher tiers come from deeper levels
and from a town whose sectors have levelled up.

### Categories

**Weapons**

- Melee: swords, axes, maces, daggers
- Ranged: bows, crossbows, throwing weapons
- Magical: wands, staves

**Armour**

- Head, chest, hands, feet slots
- Light, medium and heavy armour types
- Magical enchantments

**Consumables**

- Health potions, mana potions
- Scrolls (fireball, teleport, identify, Town Portal, etc.)
- Food (prevents starvation)

**Artefacts**

- Unique powerful items
- Often have drawbacks
- Limited per run

**Gold**

- Found in the dungeon and dropped by monsters
- Held in a party purse and shown in the HUD

### Item Identification

Magical items start unidentified. Players must:

- Use scrolls of identify
- Try the item (risky)
- Ask a vendor

---

## Hunger & Survival

**Default:** hunger is tracked per party member.

### Hunger States

- **Satiated**: No penalties
- **Hungry**: Warning state
- **Starving**: HP loss per turn, no natural healing
- **Death**: Starvation kills

### Food Sources

- Found food items
- Monster corpses (some edible)
- Vendors in town sell rations

---

## Victory & Defeat

### Victory Condition

Return to the surface while carrying the Heart of the Abyss.

### Defeat

- Every party member is dead, from wounds or starvation
- The run ends and a new party sets out
- The town keeps everything it gained during the run

### Scoring (Optional)

- Depth reached
- Monsters killed
- Gold collected
- Turns taken

---

## Controls

- **Left click**: move to a tile, attack a hostile in reach, pick up an item
  underfoot, take the stairs, select a party member
- **Tab**: select the next party member
- **WASD**: pan the camera
- **Scroll wheel**: zoom
- A key recentres the camera on the party
- A path preview is drawn on hover
- HJKL, arrow and numpad movement are removed

The full binding list is set by F05 and C08.

---

## Presentation

### Camera

An orthographic isometric camera follows the party. Walls nearer the camera
that would hide the floor or a party member are cut down to a low stub or
faded. Town roofs hide while a party member is inside the building (F06).

### Lighting

The dungeon has near-black ambient light. The party carries a torch. Candles
and wall torches are point lights that cast shadows. Distance fog, tonemapping
and bloom on flames complete the look. The town is outdoors under a moody low
sun. Explored but unseen tiles stay readable, but dim and desaturated (F07).

### Art Style

Stylised low-poly models made in Blender by us. Flat or lightly textured PBR
materials, high roughness, a muted palette and strong silhouettes that read
from the isometric angle.

### Animation

**Default:** characters animate by code tweens at first: they slide along
their path, lunge on melee and fall on death (C09). Skeletal animation comes
late (A09).

---

## Technical Details

### Engine

- **Language**: Rust
- **Framework**: Bevy ECS
- **Rendering**: Bevy 3D (PBR meshes, glTF models, real lights and shadows).
  The current ASCII renderer is deleted once 3D reaches parity (F09). The map
  generation visualiser stays 2D.

### Persistence

- The run is saved to `savegame.json` and deleted by permadeath.
- The town is saved to `town.json` and is never deleted by permadeath.

### Platforms

- Desktop (Windows, macOS, Linux)
- Web (WASM): dropped in F09, the build fails (see `CLAUDE.md`).

### Status

What is done and the older backlog live in [ROADMAP.md](ROADMAP.md). The
overhaul to the game described here is tracked in [TODO.md](TODO.md).
