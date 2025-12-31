# The Abyss Below - Game Design Document

*A traditional roguelike built in Rust with Bevy*

---

## Overview

**The Abyss Below** is a 2D turn-based roguelike that captures the essence of classic dungeon crawlers. Players descend through procedurally-generated dungeons to retrieve the legendary **Heart of the Abyss**, then fight their way back to the surface to claim victory.

This is a living document that will evolve as the game develops.

---

## Story & Setting

### Theme
Classic fantasy dungeon crawling with orcs, goblins, dragons, and magical artifacts.

### Premise
Beneath the ruins of an ancient fortress lies the Abyss—a labyrinth of caverns, forgotten halls, and dark realms that descend into the depths of the world. At the very bottom rests the **Heart of the Abyss**, an artifact of immense power that has corrupted everything around it.

You are an adventurer seeking glory and riches. Your goal: descend to the bottom, claim the Heart, and escape alive.

### Tone
Dark but not humorless. Danger lurks around every corner, but moments of discovery and triumph provide relief.

---

## Core Gameplay

### Genre
- Turn-based, tile-based roguelike
- Permadeath (death ends the run)
- Procedurally generated levels
- No persistent progression between runs

### Core Loop
1. Explore the current dungeon level
2. Fight monsters, avoid traps, find loot
3. Manage resources (health, hunger, consumables)
4. Find the stairs and descend deeper
5. Retrieve the Heart of the Abyss at the bottom
6. Escape back to the surface

### Key Mechanics
- **Combat**: Tactical turn-based fighting with melee and ranged options
- **Exploration**: Fog of war, hidden secrets, varied dungeon layouts
- **Survival**: Hunger system, limited healing, resource scarcity
- **Items**: Equipment, consumables, and magical artifacts

---

## Level Progression

### Dungeon Depth: 12 Levels

| Depth | Name | Theme | Key Features |
|-------|------|-------|--------------|
| 1 | Ruined Cellars | Tutorial area | Easy enemies, basic loot |
| 2-3 | Limestone Caverns | Natural caves | Goblins, rats, bats |
| 4-5 | Forgotten Mines | Dwarven ruins | Orcs, traps, better equipment |
| 6-7 | Fungal Depths | Mushroom forest | Poison, confusion, strange creatures |
| 8-9 | Sunken Temple | Flooded ruins | Undead, curses, holy artifacts |
| 10-11 | The Dark Realm | Abyssal plane | Demons, elite monsters |
| 12 | Heart Chamber | Final level | Boss encounter, the Heart |

### Return Journey
After claiming the Heart, all levels increase in difficulty. New powerful enemies spawn, and the player must fight upward through 12 increasingly dangerous floors.

---

## Characters

### The Player
A lone adventurer with customizable playstyle through equipment choices:
- **Warrior path**: Heavy armor, melee weapons
- **Rogue path**: Light armor, stealth, ranged weapons
- **Mage path**: Robes, wands, scrolls, potions

### Monsters

| Category | Examples | Depth Range |
|----------|----------|-------------|
| Vermin | Rats, bats, spiders | 1-3 |
| Goblinoids | Goblins, hobgoblins | 2-5 |
| Orcs | Orc warriors, shamans | 4-7 |
| Undead | Skeletons, wraiths, liches | 6-9 |
| Demons | Imps, hellhounds, demon lords | 8-12 |
| Bosses | Unique per-level bosses | Various |

### NPCs
- Shopkeepers (in safe rooms)
- Prisoners to rescue (optional quests)
- Ghosts with hints/lore

---

## Items & Equipment

### Categories

**Weapons**
- Melee: Swords, axes, maces, daggers
- Ranged: Bows, crossbows, throwing weapons
- Magical: Wands, staves

**Armor**
- Head, chest, hands, feet slots
- Light/medium/heavy armor types
- Magical enchantments

**Consumables**
- Health potions, mana potions
- Scrolls (fireball, teleport, identify, etc.)
- Food (prevents starvation)

**Artifacts**
- Unique powerful items
- Often have drawbacks
- Limited per run

### Item Identification
Magical items start unidentified. Players must:
- Use scrolls of identify
- Try the item (risky)
- Find a shopkeeper

---

## Combat System

### Stats
- **HP**: Health points (death at 0)
- **Attack**: Damage dealt
- **Defense**: Damage reduction
- **Power**: Magical effectiveness

### Combat Flow
1. Player and monsters take turns
2. Attack by moving into enemy
3. Damage = Attack - Defense + modifiers
4. Status effects (poison, confusion, etc.)
5. Death drops loot

### Tactical Elements
- Positioning (doorways, corridors)
- Line of sight
- Ranged vs melee trade-offs
- Consumable usage timing

---

## Hunger & Survival

### Hunger States
- **Satiated**: No penalties
- **Hungry**: Warning state
- **Starving**: HP loss per turn, no natural healing
- **Death**: Starvation kills

### Food Sources
- Found food items
- Monster corpses (some edible)
- Shopkeepers sell rations

---

## Victory & Defeat

### Victory Condition
Return to the surface (Depth 0) while carrying the Heart of the Abyss.

### Defeat
- HP reaches 0 (death)
- Starvation
- Run ends, start fresh

### Scoring (Optional)
- Depth reached
- Monsters killed
- Gold collected
- Turns taken

---

## Technical Details

### Engine
- **Language**: Rust
- **Framework**: Bevy ECS
- **Rendering**: Text-based (ASCII/Unicode)

### Platforms
- Desktop (Windows, macOS, Linux)
- Web (WASM) - future goal

### Current Implementation Status
- ✅ Basic map generation (BSP, cellular automata, WFC, etc.)
- ✅ Player movement and input
- ✅ Field of view
- ✅ Monster AI and pathfinding
- ✅ Combat system
- ✅ Items and inventory
- ✅ Hunger system
- ✅ Ranged targeting
- ✅ Doors
- ✅ Camera system
- 🔲 Multiple dungeon themes
- 🔲 Data-driven content (JSON)
- 🔲 Save/Load improvements
- 🔲 Boss encounters
- 🔲 Return journey mechanics

---

## Next Steps

1. **Data-Driven Design** (Chapter 45): Move items, monsters, props to JSON
2. **Level Theming**: Different visuals/enemies per depth range
3. **Boss System**: Unique encounters at key depths
4. **Return Journey**: Difficulty scaling after Heart retrieval
5. **Polish**: Balance, variety, quality of life
