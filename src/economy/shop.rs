//! Trading with a vendor: pure rules over the town, the purse's gold and a
//! raws item, and hiring recruits at the pub. The shop screen (`ui/menus/shop.rs`) moves the item entity into
//! or out of the hero's backpack when a trade here succeeds.

use super::town::{stocked_by, Sector, TownState, Vendor};
use super::trade::{buy_price, record_sale, sell_price};
use crate::party::PARTY_SIZE;
use crate::raws::Item;

/// The barkeep's recruits by class (a raws `classes` name), each with the
/// names they come under, the first not already in the party offered.
pub const RECRUITS: [(&str, [&str; 3]); 4] = [
    ("Fighter", ["Halvard", "Ingrid", "Torvald"]),
    ("Ranger", ["Sigrun", "Eirik", "Wren"]),
    ("Mage", ["Ysolde", "Merrick", "Oswin"]),
    ("Cleric", ["Hilde", "Anselm", "Maren"]),
];
/// Gold a recruit costs per level.
pub const RECRUIT_PRICE: i32 = 60;

/// A hero for hire at the pub.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Recruit {
    pub name: &'static str,
    pub class: &'static str,
    pub level: i32,
    pub price: i32,
}

/// One recruit per class, at the level of the pub's sector (provisions, 1 to
/// 5), each under a name nobody in the party has.
pub fn recruits(town: &TownState, party: &[&str]) -> Vec<Recruit> {
    let level = town.sectors[&Sector::Provisions].level as i32;
    RECRUITS
        .iter()
        .filter_map(|(class, names)| {
            let name = names.iter().find(|name| !party.contains(name))?;
            Some(Recruit { name, class, level, price: RECRUIT_PRICE * level })
        })
        .collect()
}

/// Hire `recruit` into the lowest party slot nobody holds (`taken`, the
/// living heroes' slots). Returns that slot and the message for the player.
pub fn hire(gold: &mut i32, taken: &[usize], recruit: &Recruit) -> Result<(usize, String), String> {
    let Some(slot) = (0..PARTY_SIZE).find(|slot| !taken.contains(slot)) else {
        return Err("The party is full.".to_string());
    };
    let Recruit { name, class, price, .. } = recruit;
    if *gold < *price {
        return Err(format!("You cannot afford {name} ({price} gold)."));
    }
    *gold -= price;
    Ok((slot, format!("{name} the {} joins the party for {price} gold.", class.to_lowercase())))
}

/// A vendor buys what it would stock: its stalls' sectors and tiers,
/// whatever the town's level. Gold has no sector, so nobody buys it.
pub fn buys(vendor: Vendor, item: &Item) -> bool {
    item.sector
        .is_some_and(|sector| stocked_by(sector, item.tier).any(|v| v == vendor))
}

/// Sell one `item` to `vendor`: the purse gains its sell price and the sale
/// joins the sector's stock. Returns the message for the player.
pub fn sell(town: &mut TownState, gold: &mut i32, vendor: Vendor, item: &Item) -> Result<String, String> {
    let sector = item
        .sector
        .filter(|_| buys(vendor, item))
        .ok_or_else(|| format!("The {} does not buy the {}.", vendor.name(), item.name))?;
    let price = sell_price(town, sector, item.base_value);
    *gold += price;
    record_sale(town, sector, item.base_value, 1);
    Ok(format!("Sold the {} for {price} gold.", item.name))
}

/// Buy one `item` off `vendor`'s shelf, if it has one and the purse can pay.
/// Returns the message for the player.
pub fn buy(town: &mut TownState, gold: &mut i32, vendor: Vendor, item: &Item) -> Result<String, String> {
    let on_shelf = town
        .vendor_stock
        .get(&vendor)
        .and_then(|shelf| shelf.get(&item.name))
        .is_some_and(|&q| q > 0);
    let (true, Some(sector)) = (on_shelf, item.sector) else {
        return Err(format!("The {} has no {} left.", vendor.name(), item.name));
    };
    let price = buy_price(town, sector, item.base_value);
    if *gold < price {
        return Err(format!("You cannot afford the {} ({price} gold).", item.name));
    }
    *gold -= price;
    let shelf = town.vendor_stock.get_mut(&vendor).unwrap();
    let left = shelf.get_mut(&item.name).unwrap();
    *left -= 1;
    if *left == 0 {
        shelf.remove(&item.name);
    }
    Ok(format!("Bought the {} for {price} gold.", item.name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::economy::stock::restock;
    use crate::raws::Raws;

    fn shipped_items() -> Vec<Item> {
        serde_json::from_str::<Raws>(include_str!("../../raws/spawns.json"))
            .unwrap()
            .items
    }

    fn item(name: &str) -> Item {
        shipped_items().into_iter().find(|i| i.name == name).unwrap()
    }

    fn stocked_town() -> TownState {
        let mut town = TownState::fresh(7);
        restock(&mut town, &shipped_items());
        town
    }

    #[test]
    fn the_pub_offers_one_recruit_a_class_at_its_level() {
        let mut town = TownState::fresh(7);
        let offered = recruits(&town, &["Aldric", "Halvard"]);
        assert_eq!(
            offered.iter().map(|r| (r.name, r.class, r.level, r.price)).collect::<Vec<_>>(),
            vec![
                ("Ingrid", "Fighter", 1, 60),
                ("Sigrun", "Ranger", 1, 60),
                ("Ysolde", "Mage", 1, 60),
                ("Hilde", "Cleric", 1, 60),
            ]
        );
        town.sectors.get_mut(&Sector::Provisions).unwrap().level = 3;
        assert_eq!((recruits(&town, &[])[0].level, recruits(&town, &[])[0].price), (3, 180));
    }

    #[test]
    fn every_recruit_class_is_in_the_raws() {
        let raws: Raws = serde_json::from_str(include_str!("../../raws/spawns.json")).unwrap();
        for (class, _) in RECRUITS {
            assert!(raws.classes.iter().any(|c| c.name == class), "{class}");
        }
    }

    #[test]
    fn a_hire_fills_the_first_empty_slot_for_its_price() {
        let recruit = recruits(&TownState::fresh(7), &[])[2].clone();
        let mut gold = 100;
        assert_eq!(
            hire(&mut gold, &[0, 2, 3], &recruit),
            Ok((1, "Ysolde the mage joins the party for 60 gold.".to_string()))
        );
        assert_eq!(gold, 40);
    }

    #[test]
    fn a_hire_is_refused_with_a_full_party_or_a_short_purse() {
        let recruit = recruits(&TownState::fresh(7), &[])[0].clone();
        let mut gold = 1000;
        assert_eq!(hire(&mut gold, &[0, 1, 2, 3], &recruit), Err("The party is full.".to_string()));
        let mut gold = 59;
        assert_eq!(hire(&mut gold, &[1, 2, 3], &recruit), Err("You cannot afford Halvard (60 gold).".to_string()));
        assert_eq!(gold, 59);
    }

    #[test]
    fn each_vendor_buys_its_own_sectors_and_tiers() {
        let cases = [
            ("Dagger", Vendor::Blacksmith, true),
            ("Dagger", Vendor::Alchemist, false),
            ("Health Potion", Vendor::Alchemist, true),
            ("Fireball Scroll", Vendor::Alchemist, true),
            ("Fireball Scroll", Vendor::Enchanter, false),
            ("Runed Staff", Vendor::Alchemist, false),
            ("Runed Staff", Vendor::Enchanter, true),
            ("Rations", Vendor::Barkeep, true),
            ("Gold", Vendor::Barkeep, false),
        ];
        for (name, vendor, expected) in cases {
            assert_eq!(buys(vendor, &item(name)), expected, "{vendor:?} buys {name}");
        }
    }

    #[test]
    fn selling_pays_the_purse_and_records_the_sale() {
        let mut town = stocked_town();
        let mut gold = 10;
        assert_eq!(
            sell(&mut town, &mut gold, Vendor::Blacksmith, &item("Dagger")),
            Ok("Sold the Dagger for 6 gold.".to_string())
        );
        assert_eq!(gold, 16);
        assert_eq!(town.sectors[&Sector::Smithing].stock, 15);
    }

    #[test]
    fn a_vendor_refuses_what_it_does_not_buy() {
        let mut town = stocked_town();
        let mut gold = 10;
        assert_eq!(
            sell(&mut town, &mut gold, Vendor::Priest, &item("Dagger")),
            Err("The Priest does not buy the Dagger.".to_string())
        );
        assert_eq!((gold, town), (10, stocked_town()));
    }

    #[test]
    fn buying_takes_gold_and_one_off_the_shelf() {
        let mut town = stocked_town();
        let mut gold = 40;
        assert_eq!(
            buy(&mut town, &mut gold, Vendor::Blacksmith, &item("Dagger")),
            Ok("Bought the Dagger for 15 gold.".to_string())
        );
        assert_eq!(gold, 25);
        assert_eq!(town.vendor_stock[&Vendor::Blacksmith]["Dagger"], 2);
    }

    #[test]
    fn buying_the_last_one_empties_the_shelf_slot() {
        let mut town = stocked_town();
        let mut gold = 100;
        for _ in 0..3 {
            buy(&mut town, &mut gold, Vendor::Blacksmith, &item("Dagger")).unwrap();
        }
        assert!(!town.vendor_stock[&Vendor::Blacksmith].contains_key("Dagger"));
        assert_eq!(
            buy(&mut town, &mut gold, Vendor::Blacksmith, &item("Dagger")),
            Err("The Blacksmith has no Dagger left.".to_string())
        );
        assert_eq!(gold, 55);
    }

    #[test]
    fn a_purchase_the_purse_cannot_cover_is_refused() {
        let mut town = stocked_town();
        let mut gold = 14;
        assert_eq!(
            buy(&mut town, &mut gold, Vendor::Blacksmith, &item("Dagger")),
            Err("You cannot afford the Dagger (15 gold).".to_string())
        );
        assert_eq!((gold, town), (14, stocked_town()));
    }

    #[test]
    fn nothing_is_sold_that_is_not_on_this_vendors_shelf() {
        let mut town = stocked_town();
        let mut gold = 1000;
        assert_eq!(
            buy(&mut town, &mut gold, Vendor::Priest, &item("Dagger")),
            Err("The Priest has no Dagger left.".to_string())
        );
        assert_eq!(
            buy(&mut town, &mut gold, Vendor::Blacksmith, &item("Longsword")),
            Err("The Blacksmith has no Longsword left.".to_string())
        );
        assert_eq!(gold, 1000);
    }
}
