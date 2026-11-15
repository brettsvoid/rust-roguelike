//! What vendors have on the shelf: a pure function over `TownState` and the
//! raws items. Which vendor serves which sector is `town::STALLS`.

use std::collections::BTreeMap;

use super::town::{stocked_by, TownState};
use crate::raws::Item;

/// How many of one item a vendor puts out. Rarer tiers come in ones.
fn quantity(tier: i32) -> u32 {
    (4 - tier).max(1) as u32
}

/// Refill every open vendor's shelf from scratch: each stocks the items of
/// its stalls whose tier is at most the sector's level. Closed vendors lose
/// their shelf. Running it twice gives the same stock.
pub fn restock(town: &mut TownState, items: &[Item]) {
    let mut shelves: BTreeMap<_, BTreeMap<String, u32>> = town
        .open_vendors()
        .into_iter()
        .map(|v| (v, BTreeMap::new()))
        .collect();
    for item in items {
        let Some(sector) = item.sector else { continue };
        if item.tier as u32 > town.sectors[&sector].level {
            continue;
        }
        for vendor in stocked_by(sector, item.tier) {
            shelves
                .entry(vendor)
                .or_default()
                .insert(item.name.clone(), quantity(item.tier));
        }
    }
    town.vendor_stock = shelves;
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::economy::town::{Sector, Vendor};
    use crate::raws::Raws;

    fn shipped_items() -> Vec<Item> {
        serde_json::from_str::<Raws>(include_str!("../../raws/spawns.json"))
            .unwrap()
            .items
    }

    fn tier_of(name: &str) -> i32 {
        shipped_items()
            .into_iter()
            .find(|i| i.name == name)
            .unwrap()
            .tier
    }

    fn town_at(levels: &[(Sector, u32)]) -> TownState {
        let mut town = TownState::fresh(7);
        for &(s, level) in levels {
            town.sectors.get_mut(&s).unwrap().level = level;
        }
        restock(&mut town, &shipped_items());
        town
    }

    fn shelf(town: &TownState, vendor: Vendor) -> Vec<(&str, u32)> {
        town.vendor_stock[&vendor]
            .iter()
            .map(|(n, &q)| (n.as_str(), q))
            .collect()
    }

    #[test]
    fn a_fresh_town_stocks_only_tier_one() {
        let town = town_at(&[]);
        assert_eq!(
            town.vendor_stock.keys().copied().collect::<Vec<_>>(),
            [
                Vendor::Blacksmith,
                Vendor::Alchemist,
                Vendor::Clothier,
                Vendor::Barkeep,
                Vendor::Priest
            ]
        );
        assert_eq!(shelf(&town, Vendor::Blacksmith), [("Dagger", 3), ("Shield", 3)]);
        assert_eq!(
            shelf(&town, Vendor::Alchemist),
            [
                ("Confusion Scroll", 3),
                ("Health Potion", 3),
                ("Magic Missile Scroll", 3)
            ]
        );
        assert_eq!(
            shelf(&town, Vendor::Clothier),
            [("Padded Tunic", 3), ("Wool Cloak", 3)]
        );
        assert_eq!(shelf(&town, Vendor::Barkeep), [("Rations", 3)]);
        assert_eq!(
            shelf(&town, Vendor::Priest),
            [("Holy Water", 3), ("Prayer Scroll", 3)]
        );
        let tiers: BTreeSet<_> = town
            .vendor_stock
            .values()
            .flat_map(|s| s.keys())
            .map(|n| tier_of(n))
            .collect();
        assert_eq!(tiers, BTreeSet::from([1]));
    }

    #[test]
    fn a_levelled_town_stocks_higher_tiers_and_has_the_enchanter() {
        let town = town_at(&[(Sector::Smithing, 3), (Sector::Arcana, 3)]);
        assert_eq!(
            shelf(&town, Vendor::Blacksmith),
            [
                ("Battleaxe", 2),
                ("Chain Mail", 2),
                ("Dagger", 3),
                ("Kite Shield", 1),
                ("Longsword", 2),
                ("Scale Mail", 1),
                ("Shield", 3),
                ("Tower Shield", 2),
                ("Warhammer", 1)
            ]
        );
        assert_eq!(
            shelf(&town, Vendor::Enchanter),
            [("Runed Staff", 1), ("Scroll of Lightning", 1)]
        );
        assert_eq!(
            shelf(&town, Vendor::Alchemist),
            [
                ("Confusion Scroll", 3),
                ("Fireball Scroll", 2),
                ("Health Potion", 3),
                ("Magic Missile Scroll", 3),
                ("Scroll of Magic Mapping", 2)
            ],
            "alchemy is still level 1, but arcana 2 stays with the alchemist"
        );
        assert_eq!(shelf(&town, Vendor::Barkeep), [("Rations", 3)]);
    }

    #[test]
    fn the_enchanter_opens_at_arcana_level_three() {
        for (level, open) in [(1, false), (2, false), (3, true), (5, true)] {
            let town = town_at(&[(Sector::Arcana, level)]);
            assert_eq!(
                town.vendor_stock.contains_key(&Vendor::Enchanter),
                open,
                "arcana {level}"
            );
        }
    }

    #[test]
    fn restocking_twice_changes_nothing_and_drops_a_closed_vendor() {
        let mut town = town_at(&[(Sector::Arcana, 4)]);
        let once = town.clone();
        restock(&mut town, &shipped_items());
        assert_eq!(town, once);

        town.sectors.get_mut(&Sector::Arcana).unwrap().level = 1;
        town.vendor_stock
            .get_mut(&Vendor::Barkeep)
            .unwrap()
            .insert("Rations".into(), 0);
        restock(&mut town, &shipped_items());
        assert_eq!(town, town_at(&[]));
    }

    #[test]
    fn every_item_with_a_sector_is_stocked_by_exactly_its_vendors() {
        let town = town_at(&Sector::ALL.map(|s| (s, 5)));
        for item in shipped_items() {
            let Some(sector) = item.sector else { continue };
            let stocking: Vec<_> = town
                .vendor_stock
                .iter()
                .filter(|(_, shelf)| shelf.contains_key(&item.name))
                .map(|(&v, _)| v)
                .collect();
            let mapped: Vec<_> = stocked_by(sector, item.tier).collect();
            assert_eq!(stocking.len(), 1, "{} has one vendor", item.name);
            assert_eq!(stocking, mapped, "{}", item.name);
        }
    }
}
