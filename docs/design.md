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
level (C11). The experience curve and recruit prices are open until C11 and
the balance pass (P04).

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
sight decides who can be targeted. The formula and its numbers are open until
C04 and the balance pass (P04).

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
thresholds are open until E04 and the balance pass (P04).

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

After the Heart is claimed, all levels increase in difficulty. New powerful
enemies spawn, and the party must fight upward through 12 increasingly
dangerous floors. Whether a Town Portal works while carrying the Heart is open.

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
- Web (WASM): kept only if 3D lighting works there. F09 decides.

### Status

What is done and the older backlog live in [ROADMAP.md](ROADMAP.md). The
overhaul to the game described here is tracked in [TODO.md](TODO.md).
