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
    fn every_item_but_gold_belongs_to_a_sector() {
        use crate::economy::town::Sector::*;
        let sectors: Vec<_> = shipped_raws()
            .items
            .into_iter()
            .map(|i| (i.name, i.sector))
            .collect();
        let expected = [
            ("Gold", None),
            ("Health Potion", Some(Alchemy)),
            ("Rations", Some(Provisions)),
            ("Magic Missile Scroll", Some(Arcana)),
            ("Fireball Scroll", Some(Arcana)),
            ("Confusion Scroll", Some(Arcana)),
            ("Scroll of Magic Mapping", Some(Arcana)),
            ("Dagger", Some(Smithing)),
            ("Longsword", Some(Smithing)),
            ("Battleaxe", Some(Smithing)),
            ("Shield", Some(Smithing)),
            ("Tower Shield", Some(Smithing)),
        ]
        .map(|(n, s)| (n.to_string(), s));
        assert_eq!(sectors, expected);
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
