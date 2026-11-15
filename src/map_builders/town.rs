//! The starting town (depth 1).
//!
//! Not procedural chaos like the dungeons - a hand-shaped map: grass with a
//! river, a main road with a bridge, and a handful of buildings (pub,
//! temple, shops, hovels) sized by importance and linked to the road by
//! paths. The player starts at the pub; the "dungeon entrance" leads down.
//!
//! The town grows with `TownState` but its layout never moves. Generation
//! runs in two halves:
//!
//! 1. Layout, from the town's seed alone: terrain, walls, road, building
//!    footprints, doors, paths and the slots where stalls, lamps and
//!    townsfolk may later stand.
//! 2. Growth, from the levels: what each footprint is (`roles`), how it is
//!    furnished (`furnishing`, `SECTOR_GROWTH`) and how busy the street is
//!    (`STREETS`).
//!
//! Every stage and every building draws from its own sub-seed
//! (`stage_rng`), so a level that places one more candle in the smithy
//! never shifts anything anywhere else.

use std::collections::{BTreeMap, HashSet};

use rand::seq::SliceRandom;
use rand::RngExt;

use crate::economy::town::{Sector, TownState};
use crate::map::TileType;
use crate::rng::GameRng;

use super::{BuilderMap, InitialMapBuilder};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BuildingTag {
    Pub,
    Temple,
    Blacksmith,
    Clothier,
    Alchemist,
    PlayerHouse,
    Hovel,
    House,
    Enchanter,
    Abandoned,
}

/// x, y, width, height of a building, walls included.
type Footprint = (i32, i32, i32, i32);

/// The footprints from largest to smallest take these roles; the rest are
/// spare and grow with the town (`roles`).
const CORE: [BuildingTag; 6] = [
    BuildingTag::Pub,
    BuildingTag::Temple,
    BuildingTag::Blacksmith,
    BuildingTag::Clothier,
    BuildingTag::Alchemist,
    BuildingTag::PlayerHouse,
];

/// The town level at which each spare footprint, in size order, is repaired
/// into a hovel. A hovel becomes a house `HOUSE_AFTER` town levels later.
const REPAIRED_AT: [u32; 6] = [1, 4, 7, 10, 13, 16];
const HOUSE_AFTER: u32 = 3;

/// The arcana level that brings the enchanter to town (matches `STALLS`).
const ENCHANTER_ARCANA: u32 = 3;

/// Each sector's level furnishes its own building: two more of these per
/// level above 1. Candles are the lights F07 turns into real ones.
const SECTOR_GROWTH: [(BuildingTag, Sector, [&str; 8]); 6] = [
    (BuildingTag::Pub, Sector::Provisions, ["Candle", "Patron", "Table", "Chair", "Keg", "Patron", "Candle", "Table"]),
    (BuildingTag::Temple, Sector::Faith, ["Candle", "Chair", "Candle", "Chair", "Candle", "Candle", "Chair", "Candle"]),
    (BuildingTag::Blacksmith, Sector::Smithing, ["Candle", "Weapon Rack", "Armor Stand", "Anvil", "Candle", "Weapon Rack", "Water Trough", "Candle"]),
    (BuildingTag::Clothier, Sector::Textiles, ["Candle", "Loom", "Hide Rack", "Cabinet", "Candle", "Loom", "Hide Rack", "Candle"]),
    (BuildingTag::Alchemist, Sector::Alchemy, ["Candle", "Chemistry Set", "Cabinet", "Dead Thing", "Candle", "Chemistry Set", "Cabinet", "Candle"]),
    (BuildingTag::Enchanter, Sector::Arcana, ["Candle", "Cabinet", "Chemistry Set", "Candle", "Table", "Candle", "Cabinet", "Candle"]),
];

/// Street life by town level: (from town level, market stalls, townsfolk,
/// lamp posts). The last row at or below the town's level applies.
const STREETS: [(u32, usize, usize, usize); 5] = [
    (1, 0, 0, 0),
    (3, 2, 1, 2),
    (6, 4, 2, 4),
    (10, 6, 4, 6),
    (15, 8, 6, 8),
];

// Sub-seed stages. Layout stages must never read the levels.
const STAGE_PIERS: u64 = 0;
const STAGE_GATE: u64 = 1;
const STAGE_FOOTPRINTS: u64 = 2;
const STAGE_DOORS: u64 = 3;
const STAGE_LAMPS: u64 = 4;
const STAGE_FOLK: u64 = 5;
const STAGE_BUILDING: u64 = 100;

/// An independent stream per stage, so drawing more or fewer values in one
/// stage never shifts another.
fn stage_rng(seed: u64, stage: u64) -> GameRng {
    GameRng::seeded(seed ^ stage.wrapping_add(1).wrapping_mul(0x9E37_79B9_7F4A_7C15))
}

/// What the seed alone decides.
#[derive(Debug, Clone, PartialEq)]
struct Layout {
    gate_y: i32,
    footprints: Vec<Footprint>,
    door_idx: Vec<usize>,
    stall_slots: Vec<(i32, i32)>,
    lamp_slots: Vec<(i32, i32)>,
    folk_slots: Vec<(i32, i32)>,
}

pub struct TownBuilder {
    seed: u64,
    town_level: u32,
    sector_levels: BTreeMap<Sector, u32>,
    /// Filled by the build; read by the tests.
    layout: Option<Layout>,
    buildings: Vec<(Footprint, BuildingTag)>,
}

impl TownBuilder {
    /// Copies only what the town's look depends on: its seed and levels.
    pub fn new(town: &TownState) -> Self {
        TownBuilder {
            seed: town.seed,
            town_level: town.level(),
            sector_levels: town.sectors.iter().map(|(s, st)| (*s, st.level)).collect(),
            layout: None,
            buildings: Vec::new(),
        }
    }

    fn sector_level(&self, sector: Sector) -> u32 {
        self.sector_levels.get(&sector).copied().unwrap_or(1)
    }
}

impl InitialMapBuilder for TownBuilder {
    /// The run's rng is ignored: the town is drawn from its own seed.
    fn build_map(&mut self, _rng: &mut GameRng, build_data: &mut BuilderMap) {
        self.build_town(build_data);
    }
}

impl TownBuilder {
    fn build_town(&mut self, build_data: &mut BuilderMap) {
        let layout = self.layout(build_data);

        build_data.starting_position = Some(match layout.footprints.first() {
            // The pub, the largest building
            Some(&(x, y, w, h)) => (x + w / 2, y + h / 2),
            None => (15, layout.gate_y),
        });
        for &idx in &layout.door_idx {
            build_data.spawn_list.push((idx, "Door".to_string()));
        }

        let roles = roles(
            layout.footprints.len(),
            self.town_level,
            self.sector_level(Sector::Arcana),
        );
        self.buildings = layout.footprints.iter().copied().zip(roles).collect();
        for (i, &(footprint, tag)) in self.buildings.iter().enumerate() {
            let mut rng = stage_rng(self.seed, STAGE_BUILDING + i as u64);
            self.dress_building(&mut rng, build_data, footprint, tag, &layout.door_idx);
        }
        build_data.take_snapshot();

        self.street_life(build_data, &layout);
        build_data.take_snapshot();

        // Don't spawn monsters in town - use an empty rooms list
        build_data.rooms = Some(Vec::new());
        self.layout = Some(layout);
    }

    // ------------------------------------------------------------------
    // Layout: the seed alone
    // ------------------------------------------------------------------

    fn layout(&self, build_data: &mut BuilderMap) -> Layout {
        // Start with grass everywhere
        for tile in build_data.map.tiles.iter_mut() {
            *tile = TileType::Grass;
        }
        build_data.take_snapshot();

        self.water_and_coastline(&mut stage_rng(self.seed, STAGE_PIERS), build_data);
        build_data.take_snapshot();

        let (mut available, gate_y) =
            self.town_walls(&mut stage_rng(self.seed, STAGE_GATE), build_data);
        build_data.take_snapshot();

        let footprints = self.footprints(
            &mut stage_rng(self.seed, STAGE_FOOTPRINTS),
            build_data,
            &mut available,
        );
        build_data.take_snapshot();

        let (door_idx, outside_doors) = self.add_doors(
            &mut stage_rng(self.seed, STAGE_DOORS),
            build_data,
            &footprints,
            gate_y,
        );
        self.add_paths(build_data, &outside_doors, gate_y);

        // The dungeon entrance at the river end of the main road
        let exit_idx = build_data.map.xy_idx(12, gate_y);
        build_data.map.tiles[exit_idx] = TileType::DownStairs;
        build_data.take_snapshot();

        // Stalls stand on the grass either side of the main road, inside the
        // walls first, then spilling out past the gate.
        let stall_slots = (15..=29)
            .step_by(2)
            .chain((33..=45).step_by(2))
            .flat_map(|x| [(x, gate_y - 1), (x, gate_y + 1)])
            .filter(|&(x, y)| build_data.map.tiles[build_data.map.xy_idx(x, y)] == TileType::Grass)
            .collect();
        // Lamps on the even road tiles, townsfolk on the odd ones.
        let mut lamp_slots: Vec<_> = (14..=28).step_by(2).map(|x| (x, gate_y)).collect();
        lamp_slots.shuffle(&mut stage_rng(self.seed, STAGE_LAMPS).0);
        let mut folk_slots: Vec<_> = (15..=29).step_by(2).map(|x| (x, gate_y)).collect();
        folk_slots.shuffle(&mut stage_rng(self.seed, STAGE_FOLK).0);

        Layout {
            gate_y,
            footprints,
            door_idx,
            stall_slots,
            lamp_slots,
            folk_slots,
        }
    }

    fn water_and_coastline(&self, rng: &mut GameRng, build_data: &mut BuilderMap) {
        let map = &mut build_data.map;

        // Use sine wave for natural coastline variation
        for y in 0..map.height {
            // Base coastline at x=10, with sine wave variation
            let wave = (y as f32 * 0.15).sin() * 3.0;
            let coastline = 10 + wave as i32;

            for x in 0..coastline {
                let idx = map.xy_idx(x, y);
                // Shallow water near coast, deep water further out
                if x < coastline - 2 {
                    map.tiles[idx] = TileType::DeepWater;
                } else {
                    map.tiles[idx] = TileType::ShallowWater;
                }
            }
        }

        // Add some piers extending into the water
        let num_piers = rng.0.random_range(2..=4);
        for _ in 0..num_piers {
            let pier_y = rng.0.random_range(5..map.height - 5);
            let pier_start = 12; // Start from land
            let pier_end = rng.0.random_range(2..6);

            for x in pier_end..pier_start {
                let idx = map.xy_idx(x, pier_y);
                map.tiles[idx] = TileType::Bridge;
            }
        }
    }

    fn town_walls(
        &self,
        rng: &mut GameRng,
        build_data: &mut BuilderMap,
    ) -> (HashSet<usize>, i32) {
        let map = &mut build_data.map;
        let mut available_building_tiles = HashSet::new();

        // Wall runs vertically at x=30
        let wall_x = 30;
        let gate_y = rng.0.random_range(map.height / 4..map.height * 3 / 4);

        for y in 0..map.height {
            // Gate gap in the middle
            if y >= gate_y - 2 && y <= gate_y + 2 {
                let idx = map.xy_idx(wall_x, y);
                map.tiles[idx] = TileType::Road;
            } else {
                let idx = map.xy_idx(wall_x, y);
                map.tiles[idx] = TileType::Wall;
            }
        }

        // The main road runs from the piers, through the gate and out of town
        for x in 12..map.width {
            let idx = map.xy_idx(x, gate_y);
            map.tiles[idx] = TileType::Road;
        }

        // Mark tiles available for building (inside walls, not water, not roads)
        for y in 2..map.height - 2 {
            for x in 13..wall_x - 2 {
                let idx = map.xy_idx(x, y);
                if map.tiles[idx] == TileType::Grass {
                    available_building_tiles.insert(idx);
                }
            }
        }

        (available_building_tiles, gate_y)
    }

    /// Places up to 12 buildings, largest first in the result.
    fn footprints(
        &self,
        rng: &mut GameRng,
        build_data: &mut BuilderMap,
        available: &mut HashSet<usize>,
    ) -> Vec<Footprint> {
        let mut buildings: Vec<Footprint> = Vec::new();
        let map = &mut build_data.map;

        for _ in 0..100 {
            if buildings.len() == 12 {
                break;
            }
            let w = rng.0.random_range(4..8);
            let h = rng.0.random_range(4..8);

            // Try random positions
            for _ in 0..50 {
                let x = rng.0.random_range(14..28 - w);
                let y = rng.0.random_range(3..map.height - 3 - h);

                let can_place = (y..y + h)
                    .all(|by| (x..x + w).all(|bx| available.contains(&map.xy_idx(bx, by))));

                if can_place {
                    buildings.push((x, y, w, h));

                    // Draw walls and floor
                    for by in y..y + h {
                        for bx in x..x + w {
                            let idx = map.xy_idx(bx, by);
                            if by == y || by == y + h - 1 || bx == x || bx == x + w - 1 {
                                map.tiles[idx] = TileType::Wall;
                            } else {
                                map.tiles[idx] = TileType::WoodFloor;
                            }
                            available.remove(&idx);
                        }
                    }

                    // Also remove tiles around building to create spacing
                    for by in (y - 1).max(0)..=(y + h).min(map.height - 1) {
                        for bx in (x - 1).max(0)..=(x + w).min(map.width - 1) {
                            let idx = map.xy_idx(bx, by);
                            available.remove(&idx);
                        }
                    }

                    break;
                }
            }
            // A building that does not fit is skipped; a smaller one may.
        }

        // Stable sort, so equal sizes keep their placement order
        buildings.sort_by_key(|b| std::cmp::Reverse(b.2 * b.3));
        buildings
    }

    /// Returns each door's tile and the tile just outside it.
    fn add_doors(
        &self,
        rng: &mut GameRng,
        build_data: &mut BuilderMap,
        buildings: &[Footprint],
        main_road_y: i32,
    ) -> (Vec<usize>, Vec<(i32, i32)>) {
        let map = &mut build_data.map;
        let mut door_idx = Vec::new();
        let mut door_positions = Vec::new();

        for &(x, y, w, h) in buildings {
            // Calculate building center
            let center_y = y + h / 2;

            // Place door on the side closest to the main road
            let (door_x, door_y) = if main_road_y < y {
                // Road is above building, put door on top
                (x + rng.0.random_range(1..w - 1), y)
            } else if main_road_y >= y + h {
                // Road is below building, put door on bottom
                (x + rng.0.random_range(1..w - 1), y + h - 1)
            } else {
                // Road is at same level, prefer left side (toward road start)
                (x, center_y.clamp(y + 1, y + h - 2))
            };

            let idx = map.xy_idx(door_x, door_y);
            map.tiles[idx] = TileType::Floor; // Door frame is a floor tile
            door_idx.push(idx);

            // Store position just outside the door for path generation
            let outside_door = if door_y == y {
                (door_x, door_y - 1) // Door on top, path starts above
            } else if door_y == y + h - 1 {
                (door_x, door_y + 1) // Door on bottom, path starts below
            } else {
                (door_x - 1, door_y) // Door on left, path starts left
            };
            door_positions.push(outside_door);
        }

        (door_idx, door_positions)
    }

    fn add_paths(&self, build_data: &mut BuilderMap, door_positions: &[(i32, i32)], main_road_y: i32) {
        let map = &mut build_data.map;

        // Walk from each door straight to the main road's row
        for &(px, start_y) in door_positions {
            let mut py = start_y;
            let dy = if main_road_y > py { 1 } else { -1 };
            while py != main_road_y {
                let idx = map.xy_idx(px, py);
                // Only place road on grass tiles (don't overwrite buildings, water, etc.)
                if map.tiles[idx] == TileType::Grass {
                    map.tiles[idx] = TileType::Road;
                }
                py += dy;
            }
        }
    }

    // ------------------------------------------------------------------
    // Growth: what the levels make of the layout
    // ------------------------------------------------------------------

    fn dress_building(
        &self,
        rng: &mut GameRng,
        build_data: &mut BuilderMap,
        (x, y, w, h): Footprint,
        tag: BuildingTag,
        doors: &[usize],
    ) {
        let map = &mut build_data.map;
        let centre_idx = map.xy_idx(x + w / 2, y + h / 2);
        let mut interior = Vec::new();
        for by in (y + 1)..(y + h - 1) {
            for bx in (x + 1)..(x + w - 1) {
                interior.push(map.xy_idx(bx, by));
            }
        }
        interior.shuffle(&mut rng.0);

        match tag {
            BuildingTag::Abandoned => {
                // Overgrown floor and holes in the walls
                for &idx in &interior {
                    map.tiles[idx] = TileType::Grass;
                }
                for by in y..y + h {
                    for bx in x..x + w {
                        let idx = map.xy_idx(bx, by);
                        if map.tiles[idx] == TileType::Wall
                            && !doors.contains(&idx)
                            && rng.0.random_bool(0.3)
                        {
                            map.tiles[idx] = TileType::Grass;
                        }
                    }
                }
                let num_rats = rng.0.random_range(1..=3);
                for &idx in interior.iter().take(num_rats) {
                    build_data.spawn_list.push((idx, "Rat".to_string()));
                }
                return;
            }
            BuildingTag::Hovel => {
                for &idx in &interior {
                    map.tiles[idx] = TileType::Floor;
                }
            }
            _ => {}
        }

        let (keeper, base) = furnishing(tag);
        // The player starts on the pub's centre tile, so its keeper takes a furniture spot.
        let (centred, displaced) = if tag == BuildingTag::Pub {
            (None, keeper)
        } else {
            (keeper, None)
        };
        if let Some(keeper) = centred {
            build_data.spawn_list.push((centre_idx, keeper.to_string()));
        }
        let extras = SECTOR_GROWTH
            .iter()
            .find(|(t, _, _)| *t == tag)
            .map_or(&[][..], |(_, sector, extras)| {
                let gained = self.sector_level(*sector).saturating_sub(1) as usize;
                &extras[..(gained * 2).min(extras.len())]
            });
        let spots = interior.into_iter().filter(|&idx| idx != centre_idx);
        for (idx, name) in spots.zip(displaced.iter().chain(base).chain(extras)) {
            build_data.spawn_list.push((idx, name.to_string()));
        }
    }

    fn street_life(&self, build_data: &mut BuilderMap, layout: &Layout) {
        let &(_, stalls, folk, lamps) = STREETS
            .iter()
            .rev()
            .find(|row| row.0 <= self.town_level)
            .expect("STREETS starts at level 1");
        let map = &mut build_data.map;

        let mut stall_slots = layout.stall_slots.iter();
        // With no spare footprint, the enchanter trades from the first stall.
        if self.sector_level(Sector::Arcana) >= ENCHANTER_ARCANA
            && !self.buildings.iter().any(|b| b.1 == BuildingTag::Enchanter)
        {
            if let Some(&(x, y)) = stall_slots.next() {
                let idx = map.xy_idx(x, y);
                map.tiles[idx] = TileType::WoodFloor;
                build_data.spawn_list.push((idx, "Enchanter".to_string()));
            }
        }
        for &(x, y) in stall_slots.take(stalls) {
            let idx = map.xy_idx(x, y);
            map.tiles[idx] = TileType::WoodFloor;
            build_data.spawn_list.push((idx, "Market Stall".to_string()));
        }
        for &(x, y) in layout.lamp_slots.iter().take(lamps) {
            build_data.spawn_list.push((map.xy_idx(x, y), "Lamp Post".to_string()));
        }
        for &(x, y) in layout.folk_slots.iter().take(folk) {
            build_data.spawn_list.push((map.xy_idx(x, y), "Peasant".to_string()));
        }
    }
}

/// What each of `count` footprints (largest first) is at these levels.
fn roles(count: usize, town_level: u32, arcana: u32) -> Vec<BuildingTag> {
    // The enchanter takes the smallest spare footprint, leaving the
    // repair order of the others untouched.
    let enchanter = (arcana >= ENCHANTER_ARCANA && count > CORE.len()).then(|| count - 1);
    (0..count)
        .map(|i| {
            if let Some(&core) = CORE.get(i) {
                return core;
            }
            let repaired_at = REPAIRED_AT[i - CORE.len()];
            if Some(i) == enchanter {
                BuildingTag::Enchanter
            } else if town_level >= repaired_at + HOUSE_AFTER {
                BuildingTag::House
            } else if town_level >= repaired_at {
                BuildingTag::Hovel
            } else {
                BuildingTag::Abandoned
            }
        })
        .collect()
}

/// The keeper standing at the centre and the base furniture, before any
/// sector growth.
fn furnishing(tag: BuildingTag) -> (Option<&'static str>, &'static [&'static str]) {
    match tag {
        BuildingTag::Pub => (Some("Barkeep"), &["Patron", "Table", "Chair", "Keg"]),
        BuildingTag::Temple => (Some("Priest"), &["Candle", "Candle"]),
        BuildingTag::Blacksmith => (
            Some("Blacksmith"),
            &["Anvil", "Water Trough", "Weapon Rack", "Armor Stand"],
        ),
        BuildingTag::Clothier => (Some("Clothier"), &["Loom", "Hide Rack", "Cabinet"]),
        BuildingTag::Alchemist => (Some("Alchemist"), &["Chemistry Set", "Dead Thing", "Cabinet"]),
        BuildingTag::PlayerHouse => (Some("Mom"), &["Bed", "Cabinet"]),
        BuildingTag::Hovel => (Some("Peasant"), &["Bed"]),
        BuildingTag::House => (Some("Peasant"), &["Bed", "Table", "Chair", "Cabinet", "Candle"]),
        BuildingTag::Enchanter => (Some("Enchanter"), &["Candle", "Cabinet"]),
        BuildingTag::Abandoned => (None, &[]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Twelve footprints, so six spare ones to grow into.
    const SEED: u64 = 72;
    /// Only six footprints, so no spare one for the enchanter.
    const CRAMPED_SEED: u64 = 52;

    const ALL_FIVE: [(Sector, u32); 6] = [
        (Sector::Smithing, 5),
        (Sector::Alchemy, 5),
        (Sector::Textiles, 5),
        (Sector::Provisions, 5),
        (Sector::Faith, 5),
        (Sector::Arcana, 5),
    ];

    fn town_at(seed: u64, levels: &[(Sector, u32)]) -> TownState {
        let mut town = TownState::fresh(seed);
        for &(sector, level) in levels {
            town.sectors.get_mut(&sector).unwrap().level = level;
        }
        town
    }

    fn town(levels: &[(Sector, u32)]) -> TownState {
        town_at(SEED, levels)
    }

    fn build(town: &TownState) -> (TownBuilder, BuilderMap) {
        let mut builder = TownBuilder::new(town);
        let mut data = BuilderMap::new(1);
        // A different run rng each time: the town must not depend on it.
        builder.build_map(&mut GameRng::seeded(rand::random()), &mut data);
        (builder, data)
    }

    fn count(builder: &TownBuilder, tag: BuildingTag) -> usize {
        builder.buildings.iter().filter(|b| b.1 == tag).count()
    }

    fn tally(data: &BuilderMap, name: &str) -> usize {
        data.spawn_list.iter().filter(|s| s.1 == name).count()
    }

    fn inside((x, y, w, h): Footprint, idx: usize, width: i32) -> bool {
        let (px, py) = (idx as i32 % width, idx as i32 / width);
        px >= x && px < x + w && py >= y && py < y + h
    }

    fn spawns_in(data: &BuilderMap, footprint: Footprint) -> Vec<(usize, String)> {
        data.spawn_list
            .iter()
            .filter(|(idx, _)| inside(footprint, *idx, data.map.width))
            .cloned()
            .collect()
    }

    #[test]
    fn same_seed_and_levels_give_the_same_map() {
        let grown = town(&[(Sector::Smithing, 3), (Sector::Arcana, 4)]);
        let (_, a) = build(&grown);
        let (_, b) = build(&grown);
        assert_eq!(a.map.tiles, b.map.tiles);
        assert_eq!(a.spawn_list, b.spawn_list);
        assert_eq!(a.starting_position, b.starting_position);
    }

    #[test]
    fn growth_keeps_the_layout() {
        let (small, small_map) = build(&town(&[]));
        let (big, big_map) = build(&town(&ALL_FIVE));
        let layout = small.layout.unwrap();
        assert_eq!(layout.footprints.len(), 12);
        assert_eq!(Some(&layout), big.layout.as_ref());
        assert_eq!(small_map.starting_position, big_map.starting_position);

        // Tiles only change inside footprints and on stall slots.
        let width = small_map.map.width;
        let grown: Vec<usize> = (0..small_map.map.tiles.len())
            .filter(|&idx| small_map.map.tiles[idx] != big_map.map.tiles[idx])
            .collect();
        assert!(!grown.is_empty(), "the advanced town looks different");
        for idx in grown {
            let (x, y) = (idx as i32 % width, idx as i32 / width);
            let in_footprint = layout.footprints.iter().any(|&f| inside(f, idx, width));
            assert!(in_footprint || layout.stall_slots.contains(&(x, y)), "({x}, {y}) moved");
        }
    }

    #[test]
    fn town_level_repairs_and_upgrades_the_spare_buildings() {
        // (town level, abandoned, hovels, houses), growing every sector but
        // arcana so the enchanter stays away.
        let at = |gain: u32| {
            let sectors = [Sector::Smithing, Sector::Alchemy, Sector::Textiles, Sector::Provisions, Sector::Faith];
            let mut t = town(&[]);
            for i in 0..gain as usize {
                t.sectors.get_mut(&sectors[i % 5]).unwrap().level += 1;
            }
            let (b, data) = build(&t);
            let furnished = b.buildings.len() - count(&b, BuildingTag::Abandoned);
            assert_eq!(tally(&data, "Rat") > 0, count(&b, BuildingTag::Abandoned) > 0);
            (
                t.level(),
                count(&b, BuildingTag::Abandoned),
                count(&b, BuildingTag::Hovel),
                count(&b, BuildingTag::House),
                furnished,
            )
        };
        assert_eq!(at(0), (1, 5, 1, 0, 7));
        assert_eq!(at(3), (4, 4, 1, 1, 8));
        assert_eq!(at(9), (10, 2, 1, 3, 10));
        assert_eq!(at(20), (21, 0, 0, 6, 12));
    }

    #[test]
    fn the_enchanter_arrives_at_arcana_three() {
        for (arcana, expected) in [(1, 0), (2, 0), (3, 1), (5, 1)] {
            let (builder, data) = build(&town(&[(Sector::Arcana, arcana)]));
            assert_eq!(count(&builder, BuildingTag::Enchanter), expected, "arcana {arcana}");
            assert_eq!(tally(&data, "Enchanter"), expected, "arcana {arcana}");
        }
    }

    #[test]
    fn a_cramped_town_gives_the_enchanter_a_stall() {
        let (builder, data) = build(&town_at(CRAMPED_SEED, &[(Sector::Arcana, 3)]));
        assert_eq!(builder.buildings.len(), 6);
        assert_eq!(count(&builder, BuildingTag::Enchanter), 0);
        let (idx, _) = data.spawn_list.iter().find(|s| s.1 == "Enchanter").unwrap();
        let w = data.map.width;
        let slot = (*idx as i32 % w, *idx as i32 / w);
        assert_eq!(Some(&slot), builder.layout.unwrap().stall_slots.first());
        assert_eq!(data.map.tiles[*idx], TileType::WoodFloor);
    }

    #[test]
    fn a_sector_furnishes_only_its_own_building() {
        let (base, base_map) = build(&town(&[]));
        let (raised, smith_map) = build(&town(&[(Sector::Smithing, 4)]));
        let mut grew = Vec::new();
        for (&(footprint, tag), &(_, raised_tag)) in base.buildings.iter().zip(&raised.buildings) {
            // The town level rose too; that may repair a spare footprint.
            if tag != raised_tag {
                continue;
            }
            let before = spawns_in(&base_map, footprint);
            let after = spawns_in(&smith_map, footprint);
            if before != after {
                assert_eq!(after[..before.len()], before[..], "the old furniture stays put");
                grew.push((tag, before.len(), after.len()));
            }
        }
        // Door, smith and 4 props, then 3 levels gained x 2 more
        assert_eq!(grew, [(BuildingTag::Blacksmith, 6, 12)]);
    }

    #[test]
    fn an_advanced_town_has_more_lights_stalls_and_people() {
        let (_, small) = build(&town(&[]));
        let (_, big) = build(&town(&ALL_FIVE));
        let lights = |d: &BuilderMap| tally(d, "Candle") + tally(d, "Lamp Post");
        assert_eq!((lights(&small), lights(&big)), (2, 27));
        assert_eq!((tally(&small, "Market Stall"), tally(&big, "Market Stall")), (0, 8));
        assert_eq!((tally(&small, "Peasant"), tally(&big, "Peasant")), (1, 11));
    }
}
