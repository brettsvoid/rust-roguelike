//! Prefab (hand-made) map pieces.
//!
//! Not everything has to be generated. This module drops hand-drawn ASCII
//! templates into the map: small "vaults" (a trap room, a monster den)
//! scattered wherever they fit, or bigger "sections" pinned to a map edge.
//! Template characters spell out both tiles and spawns - 'g' goblin,
//! 'o' orc, '!' health potion, '^' bear trap, '#' wall, and so on.
//!
//! Good for: set-piece moments inside otherwise procedural levels.

use bevy::prelude::*;
use rand::Rng;

use crate::map::{Map, TileType, MAP_HEIGHT, MAP_WIDTH};
use crate::raws::{spawn_named_entity, RAWS};
use crate::rng::GameRng;

use super::{BuilderMap, MetaMapBuilder};

// ============================================================================
// Placement Enums
// ============================================================================

// Toolbox: only Right/Top are used today (CORNER_FORT), the rest are here
// for future prefab sections.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub enum HorizontalPlacement {
    Left,
    Center,
    Right,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub enum VerticalPlacement {
    Top,
    Center,
    Bottom,
}

// ============================================================================
// Prefab Definitions
// ============================================================================

/// Pure visual definition - just the ASCII art and dimensions
#[derive(Clone, Copy)]
pub struct PrefabTemplate {
    pub template: &'static str,
    pub width: usize,
    pub height: usize,
}

/// Placement constraints - when/where a prefab can appear
#[derive(Clone, Copy)]
pub struct VaultConstraints {
    pub min_depth: i32,
    pub max_depth: i32,
    pub spawn_chance: f32,
    pub min_floor_percent: u8,
}

impl Default for VaultConstraints {
    fn default() -> Self {
        Self {
            min_depth: 1,
            max_depth: i32::MAX,
            spawn_chance: 1.0,
            min_floor_percent: 80,
        }
    }
}

/// A vault combines a template with its constraints
#[derive(Clone, Copy)]
pub struct PrefabVault {
    pub template: PrefabTemplate,
    pub constraints: VaultConstraints,
}

/// A larger section placed at map edges
#[derive(Clone)]
pub struct PrefabSection {
    pub template: PrefabTemplate,
    pub placement: (HorizontalPlacement, VerticalPlacement),
}

// ============================================================================
// Example Prefabs
// ============================================================================

// Templates (pure visual definitions)
pub const TOTALLY_NOT_A_TRAP_TEMPLATE: PrefabTemplate = PrefabTemplate {
    template: "

 ^^^
 ^!^
 ^^^

",
    width: 5,
    height: 5,
};

pub const MONSTER_DEN_TEMPLATE: PrefabTemplate = PrefabTemplate {
    template: "

 gggg
 g!!g
 gggg

",
    width: 6,
    height: 5,
};

pub const CHECKERBOARD_TRAP_TEMPLATE: PrefabTemplate = PrefabTemplate {
    template: "

 ^.^.^
 .^.^.
 ^.^.^

",
    width: 7,
    height: 5,
};

pub const CORNER_FORT_TEMPLATE: PrefabTemplate = PrefabTemplate {
    template: "
#########
#.......#
#.ggggg.#
#.g!!!g.#
#.g!o!g.#
#.g!!!g.#
#.ggggg.#
#.......#
###+#####
",
    width: 9,
    height: 9,
};

// Vaults (template + constraints)
pub const TOTALLY_NOT_A_TRAP: PrefabVault = PrefabVault {
    template: TOTALLY_NOT_A_TRAP_TEMPLATE,
    constraints: VaultConstraints {
        min_depth: 1,
        max_depth: i32::MAX,
        spawn_chance: 1.0,
        min_floor_percent: 80,
    },
};

pub const MONSTER_DEN: PrefabVault = PrefabVault {
    template: MONSTER_DEN_TEMPLATE,
    constraints: VaultConstraints {
        min_depth: 2,          // Only appears on level 2+
        max_depth: i32::MAX,
        spawn_chance: 0.7,     // 70% chance to spawn
        min_floor_percent: 80,
    },
};

pub const CHECKERBOARD_TRAP: PrefabVault = PrefabVault {
    template: CHECKERBOARD_TRAP_TEMPLATE,
    constraints: VaultConstraints {
        min_depth: 1,
        max_depth: 5,          // Only on early levels
        spawn_chance: 0.5,     // 50% chance
        min_floor_percent: 80,
    },
};

/// All available vaults
pub const VAULTS: &[PrefabVault] = &[TOTALLY_NOT_A_TRAP, MONSTER_DEN, CHECKERBOARD_TRAP];

// Sections (template + placement)
pub const CORNER_FORT: PrefabSection = PrefabSection {
    template: CORNER_FORT_TEMPLATE,
    placement: (HorizontalPlacement::Right, VerticalPlacement::Top),
};

// ============================================================================
// Prefab Mode
// ============================================================================

#[derive(Clone)]
pub enum PrefabMode {
    /// Insert random vaults into the map
    RoomVaults,
    /// Insert a section at a specific position
    Sectional { section: PrefabSection },
}

// ============================================================================
// Template parsing
// ============================================================================

/// Parse a prefab template string into a flat character grid.
fn read_template(template: &str, width: usize, height: usize) -> Vec<char> {
    let mut chars: Vec<char> = Vec::with_capacity(width * height);

    // Skip leading newline if present
    let template = template.strip_prefix('\n').unwrap_or(template);

    for line in template.lines().take(height) {
        // Pad or truncate each line to the expected width
        let line_chars: Vec<char> = line.chars().collect();
        for x in 0..width {
            if x < line_chars.len() {
                chars.push(line_chars[x]);
            } else {
                chars.push(' ');
            }
        }
    }

    // Pad remaining rows if template is shorter than expected
    while chars.len() < width * height {
        chars.push(' ');
    }

    chars
}

// ============================================================================
// Spawn by name helper
// ============================================================================

pub fn spawn_by_name(
    commands: &mut Commands,
    font: &TextFont,
    x: i32,
    y: i32,
    name: &str,
    monster_id: &mut usize,
) {
    let raws = RAWS.lock().unwrap();

    // Check if it's a monster (needs monster_id)
    let is_monster = raws.mob_index.contains_key(name);

    if is_monster {
        spawn_named_entity(&raws, commands, font, name, x, y, Some(*monster_id));
        *monster_id += 1;
    } else {
        spawn_named_entity(&raws, commands, font, name, x, y, None);
    }
}

// ============================================================================
// MetaMapBuilder Implementation
// ============================================================================

/// Standalone prefab builder for use with BuilderChain
pub struct PrefabMetaBuilder {
    mode: PrefabMode,
}

impl PrefabMetaBuilder {
    pub fn vaults() -> Box<Self> {
        Box::new(Self {
            mode: PrefabMode::RoomVaults,
        })
    }

    pub fn sectional(section: PrefabSection) -> Box<Self> {
        Box::new(Self {
            mode: PrefabMode::Sectional { section },
        })
    }

    /// Convert a prefab character to map tile and/or spawn entry
    fn char_to_map(
        ch: char,
        idx: usize,
        map: &mut Map,
        spawn_list: &mut Vec<(usize, String)>,
        starting_position: &mut Option<(i32, i32)>,
    ) {
        match ch {
            '#' => map.tiles[idx] = TileType::Wall,
            '.' | ' ' => map.tiles[idx] = TileType::Floor,
            '+' => map.tiles[idx] = TileType::Floor, // Door (just floor for now)
            '>' => map.tiles[idx] = TileType::DownStairs,
            '@' => {
                map.tiles[idx] = TileType::Floor;
                let x = (idx % MAP_WIDTH) as i32;
                let y = (idx / MAP_WIDTH) as i32;
                *starting_position = Some((x, y));
            }
            // Monsters
            'g' => {
                map.tiles[idx] = TileType::Floor;
                spawn_list.push((idx, "Goblin".to_string()));
            }
            'o' => {
                map.tiles[idx] = TileType::Floor;
                spawn_list.push((idx, "Orc".to_string()));
            }
            // Items
            '!' => {
                map.tiles[idx] = TileType::Floor;
                spawn_list.push((idx, "Health Potion".to_string()));
            }
            '%' => {
                map.tiles[idx] = TileType::Floor;
                spawn_list.push((idx, "Rations".to_string()));
            }
            ')' => {
                map.tiles[idx] = TileType::Floor;
                spawn_list.push((idx, "Magic Missile Scroll".to_string()));
            }
            // Traps
            '^' => {
                map.tiles[idx] = TileType::Floor;
                spawn_list.push((idx, "Bear Trap".to_string()));
            }
            // Equipment
            '/' => {
                map.tiles[idx] = TileType::Floor;
                spawn_list.push((idx, "Dagger".to_string()));
            }
            '(' => {
                map.tiles[idx] = TileType::Floor;
                spawn_list.push((idx, "Shield".to_string()));
            }
            _ => map.tiles[idx] = TileType::Floor,
        }
    }

    /// Apply a vault at a specific position
    fn apply_vault(
        vault: &PrefabVault,
        start_x: i32,
        start_y: i32,
        build_data: &mut BuilderMap,
    ) {
        let template = &vault.template;
        let chars = read_template(template.template, template.width, template.height);

        for y in 0..template.height as i32 {
            for x in 0..template.width as i32 {
                let map_x = start_x + x;
                let map_y = start_y + y;

                if map_x >= 0
                    && map_x < MAP_WIDTH as i32
                    && map_y >= 0
                    && map_y < MAP_HEIGHT as i32
                {
                    let map_idx = build_data.map.xy_idx(map_x, map_y);
                    let char_idx = (y as usize) * template.width + (x as usize);
                    Self::char_to_map(
                        chars[char_idx],
                        map_idx,
                        &mut build_data.map,
                        &mut build_data.spawn_list,
                        &mut build_data.starting_position,
                    );
                }
            }
        }
    }

    /// Apply a section at its specified placement
    fn apply_section(section: &PrefabSection, build_data: &mut BuilderMap) {
        let template = &section.template;
        let start_x = match section.placement.0 {
            HorizontalPlacement::Left => 1,
            HorizontalPlacement::Center => (MAP_WIDTH as i32 / 2) - (template.width as i32 / 2),
            HorizontalPlacement::Right => MAP_WIDTH as i32 - template.width as i32 - 1,
        };

        let start_y = match section.placement.1 {
            VerticalPlacement::Top => 1,
            VerticalPlacement::Center => (MAP_HEIGHT as i32 / 2) - (template.height as i32 / 2),
            VerticalPlacement::Bottom => MAP_HEIGHT as i32 - template.height as i32 - 1,
        };

        let chars = read_template(template.template, template.width, template.height);

        // Apply the section
        for y in 0..template.height as i32 {
            for x in 0..template.width as i32 {
                let map_x = start_x + x;
                let map_y = start_y + y;

                if map_x >= 0
                    && map_x < MAP_WIDTH as i32
                    && map_y >= 0
                    && map_y < MAP_HEIGHT as i32
                {
                    let map_idx = build_data.map.xy_idx(map_x, map_y);
                    let char_idx = (y as usize) * template.width + (x as usize);
                    Self::char_to_map(
                        chars[char_idx],
                        map_idx,
                        &mut build_data.map,
                        &mut build_data.spawn_list,
                        &mut build_data.starting_position,
                    );
                }
            }
        }
    }

    /// Find suitable locations and apply random vaults
    fn apply_random_vaults(build_data: &mut BuilderMap, rng: &mut GameRng) {
        // Filter vaults by depth constraints
        let eligible_vaults: Vec<&PrefabVault> = VAULTS
            .iter()
            .filter(|vault| {
                build_data.depth >= vault.constraints.min_depth
                    && build_data.depth <= vault.constraints.max_depth
            })
            .collect();

        if eligible_vaults.is_empty() {
            return;
        }

        // Try to place 1-3 vaults
        let num_vaults = rng.0.gen_range(1..=3);

        for _ in 0..num_vaults {
            // Pick a random vault from eligible ones
            let vault_idx = rng.0.gen_range(0..eligible_vaults.len());
            let vault = eligible_vaults[vault_idx];

            // Check spawn probability
            if rng.0.gen::<f32>() > vault.constraints.spawn_chance {
                continue;
            }

            // Try to find a valid placement (up to 50 attempts)
            let template = &vault.template;
            for _ in 0..50 {
                let x = rng.0.gen_range(2..MAP_WIDTH as i32 - template.width as i32 - 2);
                let y = rng.0.gen_range(2..MAP_HEIGHT as i32 - template.height as i32 - 2);

                if Self::can_place_vault(&build_data.map, vault, x, y) {
                    Self::apply_vault(vault, x, y, build_data);
                    break;
                }
            }
        }
    }

    /// Check if a vault can be placed at a position (needs mostly floor tiles)
    fn can_place_vault(map: &Map, vault: &PrefabVault, start_x: i32, start_y: i32) -> bool {
        let template = &vault.template;
        let mut floor_count = 0;
        let total_tiles = template.width * template.height;

        for y in 0..template.height as i32 {
            for x in 0..template.width as i32 {
                let map_x = start_x + x;
                let map_y = start_y + y;

                if map_x < 0 || map_x >= MAP_WIDTH as i32 || map_y < 0 || map_y >= MAP_HEIGHT as i32
                {
                    return false;
                }

                let idx = map.xy_idx(map_x, map_y);
                if map.tiles[idx] == TileType::Floor {
                    floor_count += 1;
                }
            }
        }

        floor_count * 100 / total_tiles >= vault.constraints.min_floor_percent as usize
    }
}

impl MetaMapBuilder for PrefabMetaBuilder {
    fn build_map(&mut self, rng: &mut GameRng, build_data: &mut BuilderMap) {
        build_data.take_snapshot();

        // Apply prefabs based on mode
        match &self.mode {
            PrefabMode::RoomVaults => {
                Self::apply_random_vaults(build_data, rng);
            }
            PrefabMode::Sectional { section } => {
                Self::apply_section(section, build_data);
            }
        }

        build_data.take_snapshot();
    }
}
