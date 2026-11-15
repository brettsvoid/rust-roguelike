mod item_structs;
mod mob_structs;
mod prop_structs;
mod rawmaster;
mod spawn_table_structs;

pub use item_structs::*;
pub use mob_structs::*;
pub use prop_structs::*;
pub use rawmaster::*;
pub use spawn_table_structs::*;

use serde::Deserialize;
use std::sync::{LazyLock, Mutex};

#[derive(Deserialize, Debug, Clone)]
pub struct Raws {
    pub items: Vec<Item>,
    pub mobs: Vec<Mob>,
    pub props: Vec<Prop>,
    pub spawn_table: Vec<SpawnTableEntry>,
}

pub static RAWS: LazyLock<Mutex<RawMaster>> = LazyLock::new(|| Mutex::new(RawMaster::empty()));

pub fn load_raws() {
    let raw_string = include_str!("../../raws/spawns.json");
    let raws: Raws = serde_json::from_str(raw_string).expect("Failed to parse spawns.json");
    RAWS.lock().unwrap().load(raws);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shipped_raws() -> Raws {
        serde_json::from_str(include_str!("../../raws/spawns.json")).unwrap()
    }

    #[test]
    fn every_item_has_a_value_and_tier() {
        for item in shipped_raws().items {
            assert!(item.base_value > 0, "{} has no value", item.name);
            assert!(item.tier >= 1, "{} has no tier", item.name);
        }
    }

    #[test]
    fn every_prop_but_doors_states_its_cover() {
        for prop in shipped_raws().props {
            // A closed door blocks sight, so `cover::cover_at` makes it a wall.
            assert_eq!(prop.cover.is_some(), prop.door_open.is_none(), "{}", prop.name);
        }
    }

    #[test]
    fn every_item_but_gold_belongs_to_a_sector() {
        let sectorless: Vec<_> = shipped_raws()
            .items
            .into_iter()
            .filter(|i| i.sector.is_none())
            .map(|i| i.name)
            .collect();
        assert_eq!(sectorless, ["Gold"]);
    }

    /// Item names per sector and tier, so a sector can always level and
    /// every vendor gains stock at every level.
    #[test]
    fn every_sector_has_items_at_every_tier() {
        use crate::economy::town::Sector;
        use std::collections::BTreeMap;
        let mut count: BTreeMap<(Sector, i32), usize> = BTreeMap::new();
        for item in shipped_raws().items {
            assert!((1..=5).contains(&item.tier), "{} tier {}", item.name, item.tier);
            if let Some(s) = item.sector {
                *count.entry((s, item.tier)).or_default() += 1;
            }
        }
        let table: Vec<[usize; 5]> = Sector::ALL
            .map(|s| [1, 2, 3, 4, 5].map(|t| count.get(&(s, t)).copied().unwrap_or(0)))
            .into();
        // Smithing, alchemy, textiles, provisions, faith, arcana.
        assert_eq!(
            table,
            [
                [3, 4, 3, 2, 2],
                [1, 2, 1, 1, 1],
                [2, 1, 2, 1, 1],
                [1, 1, 1, 1, 1],
                [2, 1, 1, 1, 2],
                [3, 2, 2, 2, 2],
            ]
        );
    }

    #[test]
    fn every_spawn_table_name_resolves_to_a_raws_entry() {
        let raws = shipped_raws();
        let missing: Vec<_> = raws
            .spawn_table
            .iter()
            .filter(|e| {
                !raws.items.iter().any(|i| i.name == e.name)
                    && !raws.mobs.iter().any(|m| m.name == e.name)
                    && !raws.props.iter().any(|p| p.name == e.name)
            })
            .map(|e| e.name.clone())
            .collect();
        assert_eq!(missing, Vec::<String>::new());
        // And the other way: every item and every hostile mob can spawn.
        let spawnable = |name: &str| raws.spawn_table.iter().any(|e| e.name == name);
        let unspawned: Vec<_> = raws
            .items
            .iter()
            .map(|i| &i.name)
            .chain(raws.mobs.iter().filter(|m| m.ai.is_none()).map(|m| &m.name))
            .filter(|n| !spawnable(n))
            .collect();
        assert_eq!(unspawned, Vec::<&String>::new());
    }

    /// Loot is tiered by depth. Depth 0 is the town, the dungeon is 1 to 12.
    /// Gold, health potions, rations and Town Portals are staples found at every depth.
    #[test]
    fn deeper_levels_roll_higher_tier_loot() {
        use std::collections::BTreeSet;
        let raws = shipped_raws();
        let staples = ["Gold", "Health Potion", "Rations", "Town Portal"];
        let tiers_at = |depth: i32| -> BTreeSet<i32> {
            raws.spawn_table
                .iter()
                .filter(|e| (e.min_depth..=e.max_depth).contains(&depth))
                .filter(|e| !staples.contains(&e.name.as_str()))
                .filter_map(|e| raws.items.iter().find(|i| i.name == e.name))
                .map(|i| i.tier)
                .collect()
        };
        let cases = [
            (1, vec![1]),
            (3, vec![1, 2]),
            (5, vec![1, 2, 3]),
            (7, vec![2, 3, 4]),
            (9, vec![3, 4, 5]),
            (11, vec![4, 5]),
            (12, vec![5]),
        ];
        for (depth, tiers) in cases {
            assert_eq!(tiers_at(depth), tiers.into_iter().collect(), "depth {depth}");
        }
    }

    #[test]
    fn deeper_levels_spawn_the_design_bands_monsters() {
        let raws = shipped_raws();
        let mobs_at = |depth: i32| -> Vec<&str> {
            raws.spawn_table
                .iter()
                .filter(|e| (e.min_depth..=e.max_depth).contains(&depth))
                .filter(|e| raws.mobs.iter().any(|m| m.name == e.name))
                .map(|e| e.name.as_str())
                .collect()
        };
        assert_eq!(mobs_at(1), ["Kobold", "Rat", "Giant Bat", "Goblin", "Orc"]);
        assert_eq!(
            mobs_at(5),
            ["Goblin", "Hobgoblin", "Orc", "Orc Warrior", "Orc Shaman"]
        );
        assert_eq!(mobs_at(8), ["Skeleton", "Wraith", "Lich", "Imp"]);
        assert_eq!(mobs_at(12), ["Imp", "Hellhound", "Demon Lord"]);
    }

    #[test]
    fn gold_is_an_item_monsters_carry_and_the_spawn_table_rolls() {
        let raws = shipped_raws();
        let gold = raws.items.iter().find(|i| i.name == "Gold").unwrap();
        assert_eq!(gold.gold, Some((5, 25)));
        let goblin = raws.mobs.iter().find(|m| m.name == "Goblin").unwrap();
        assert_eq!(goblin.gold, Some((2, 8)));
        assert!(raws.spawn_table.iter().any(|e| e.name == "Gold"));
    }

    #[test]
    fn an_item_without_a_value_is_rejected() {
        let json = r#"{"items":[{"name":"Rock","tier":1}],"mobs":[],"props":[],"spawn_table":[]}"#;
        assert!(serde_json::from_str::<Raws>(json).is_err());
    }
}
