use allocators::{AllocStrategy, RoundRobinAllocator};
use chrono::{DateTime, Local, NaiveDateTime, TimeZone};
use dialoguer::{theme::ColorfulTheme, Input, MultiSelect, Select, Confirm};
use console::style;
use itertools::Itertools;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::convert::From;
use std::fmt::{Debug, Display};

mod allocators;
mod errors;
mod filters;

use crate::errors::ManagerError;
use filters::{BanQuality, Filter, LimitItemQuantity, LimitOverSized};
use crate::allocators::GreedyAllocator;

// Note: keep MAX_INVENTORY_SIZE >= 3 for cargo tests to be valid
const MAX_INVENTORY_SIZE: usize = 3; // TODO: same for row/shelf/zone?

// TODO: implement safeguards to Slot::new (e.g. MAX_INVENTORY_SIZE checks)
#[derive(Hash, PartialEq, Eq, Copy, Clone)]
struct Slot {
    row: usize,
    shelf: usize,
    zone: usize,
}

impl Slot {
    fn new(row: usize, shelf: usize, zone: usize) -> Self {
        Self { row, shelf, zone }
    }
    fn as_tuple(&self) -> (usize, usize, usize) {
        (self.row, self.shelf, self.zone)
    }

    fn as_array(&self) -> [usize; 3] {
        [self.row, self.shelf, self.zone]
    }

    fn distance(&self) -> usize {
        // Manhattan distance
        self.as_array().iter().sum()
    }
}

impl Display for Slot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}|{}|{}]", self.row, self.shelf, self.zone)
    } // FIXME: choose a better string representation...
}

impl Debug for Slot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(self, f)
    }
}

impl From<(usize, usize, usize)> for Slot {
    fn from(value: (usize, usize, usize)) -> Self {
        Self {
            row: value.0,
            shelf: value.1,
            zone: value.2,
        }
    }
}

impl From<[usize; 3]> for Slot {
    fn from(value: [usize; 3]) -> Self {
        Self {
            row: value[0],
            shelf: value[1],
            zone: value[2],
        }
    }
}

#[derive(Clone, Eq, PartialEq)]
enum Quality {
    Fragile {
        expiration_date: DateTime<Local>,
        max_row: usize,
    },
    OverSized {
        size: usize,
    },
    Normal,
}

impl Display for Quality {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Quality::Fragile { .. } => write!(f, "Fragile"),
            Quality::OverSized { .. } => write!(f, "OverSized"),
            Quality::Normal => write!(f, "Normal"),
        }
    }
}

impl Debug for Quality {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Quality::Fragile {
                expiration_date,
                max_row: max_dist,
            } => write!(f, "Fragile ({}, {})", expiration_date, max_dist),
            Quality::OverSized { size } => write!(f, "OverSized ({})", size),
            Quality::Normal => write!(f, "Normal"),
        }
    }
}

#[derive(Clone)]
struct Item {
    id: usize,
    name: String,
    quantity: usize,
    quality: Quality,
    // additional fields
    timestamp: Option<DateTime<Local>>,
}

impl Item {
    fn new(id: usize, name: &str, quantity: usize, quality: Quality) -> Self {
        Self {
            id,
            name: name.to_string(),
            quantity,
            quality,
            timestamp: None,
        }
    }

    fn update_timestamp(&mut self) {
        self.timestamp = Some(Local::now());
    }
}

impl Display for Item {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let timestamp = self
            .timestamp
            .map(|t| t.format("%Y-%m-%d %H:%M:%S %:z").to_string())
            .unwrap_or_else(|| "???".to_string());

        match self.quality {
            Quality::Fragile {
                expiration_date,
                max_row,
            } => {
                write!(
                    f,
                    "[Item {}: {}] [Qty: {}, {}] [Created at: {}] [Expires at: {}] [Must be stored at most at row {}]",
                    self.id,
                    self.name,
                    self.quantity,
                    self.quality,
                    timestamp,
                    expiration_date.format("%Y-%m-%d %H:%M:%S %:z"),
                    max_row
                )
            }
            Quality::OverSized { size } => {
                write!(
                    f,
                    "[Item {}: {}] [Qty: {}, {}] [Created at: {}] [Requires {} contiguous zones]",
                    self.id, self.name, self.quantity, self.quality, timestamp, size
                )
            }
            Quality::Normal => {
                write!(
                    f,
                    "[Item {}: {}] [Qty: {}, {}] [Created at: {}]",
                    self.id, self.name, self.quantity, self.quality, timestamp
                )
            }
        }
    }
}

impl Debug for Item {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(self, f)
    }
}

impl PartialEq for Item {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.name == other.name
            && self.quantity == other.quantity
            && self.quality == other.quality
    }
}

#[derive(Debug)]
struct Manager<A>
where
    A: AllocStrategy,
{
    inventory: HashMap<Slot, Item>,
    allocator: A,
    filters: Vec<Box<dyn Filter>>, // need dynamic dispatch to hold different impls of Filter

    // reverse-maps
    map_ids: HashMap<usize, usize>,       // id, count
    map_names: HashMap<String, usize>,    // name, count
    map_slots: HashMap<usize, Vec<Slot>>, // id, list of slots
    // only used for Quality::Fragile items
    map_dates: BTreeMap<DateTime<Local>, Vec<Slot>>, // date, list of ids
}

impl<A> Manager<A>
where
    A: AllocStrategy,
{
    fn new(allocator: A, filters: Vec<Box<dyn Filter>>) -> Manager<A> {
        Manager {
            inventory: HashMap::new(),
            allocator,
            filters,

            map_ids: HashMap::new(),
            map_names: HashMap::new(),
            map_slots: HashMap::new(),
            map_dates: BTreeMap::new(),
        }
    }

    fn set_filters(&mut self, filters: Vec<Box<dyn Filter>>) {
        self.filters = filters;
    }

    fn insert_filter(&mut self, filter: Box<dyn Filter>) {
        self.filters.push(filter);
    }

    fn is_allowed_by_filters(&self, item: &Item) -> bool {
        self.filters.iter().all(|f| f.filter(item, &self.inventory)) // short-circuits
    }

    fn insert_item(&mut self, item: Item) -> Result<(), ManagerError> {
        if !self.is_allowed_by_filters(&item) {
            return Err(ManagerError::FilteredItem {
                item,
                filters: self.filters.iter().map(|v| v.to_string()).collect(),
            }); // short-circuit if some filter is triggered
        }

        let slot = self
            .allocator
            .alloc(&item, &self.inventory)
            .ok_or_else(|| ManagerError::FailedAllocation {
                allocator: self.allocator.to_string(),
                item: item.clone(),
            })?;

        self._update_maps_on_insert(&slot, &item);
        self._insert_item(slot, item);
        Ok(())
    }

    fn _insert_item(&mut self, slot: Slot, mut item: Item) {
        item.update_timestamp();
        self.inventory.entry(slot).or_insert(item);
    }

    fn _update_maps_on_insert(&mut self, slot: &Slot, item: &Item) {
        *self.map_ids.entry(item.id).or_insert(0) += 1;
        *self.map_names.entry(item.name.clone()).or_insert(0) += 1;
        self.map_slots.entry(item.id).or_insert(vec![]).push(*slot);

        match item.quality {
            Quality::Fragile {
                expiration_date, ..
            } => {
                self.map_dates
                    .entry(expiration_date.clone())
                    .or_insert(vec![])
                    .push(*slot);
            }
            _ => {}
        }
    }

    fn get_item(&self, row: usize, shelf: usize, zone: usize) -> Option<&Item> {
        let slot = Slot::from((row, shelf, zone));
        self._get_item(&slot)
    }

    fn _get_item(&self, slot: &Slot) -> Option<&Item> {
        self.inventory.get(slot)
    }

    fn remove_item(&mut self, row: usize, shelf: usize, zone: usize) {
        let slot = Slot::from((row, shelf, zone));
        if let Some(item) = self._remove_item(&slot) {
            self._update_maps_on_remove(&slot, &item)
        }
    }

    fn _remove_item(&mut self, slot: &Slot) -> Option<Item> {
        self.inventory.remove(slot)
    }

    fn _update_maps_on_remove(&mut self, slot: &Slot, item: &Item) {
        self.map_ids.entry(item.id).and_modify(|count| *count -= 1);
        self.map_names
            .entry(item.name.clone())
            .and_modify(|count| *count -= 1);
        self.map_slots
            .entry(item.id)
            .and_modify(|vec| vec.retain(|s| *s != *slot));

        match item.quality {
            Quality::Fragile {
                expiration_date, ..
            } => {
                self.map_dates
                    .entry(expiration_date)
                    .and_modify(|vec| vec.retain(|s| *s != *slot));
            }
            _ => {}
        }

        // clean-up empty entries
        // FIXME: inefficient, because iterates over HashMap when at most a single entry needs to
        //        be cleaned up. Instead, it would be better to clean-up using Entry API just after
        //        we are done updating the HashMaps
        self.map_ids.retain(|_, count| *count != 0);
        self.map_names.retain(|_, count| *count != 0);
        self.map_slots.retain(|_, vec| !vec.is_empty());
        self.map_dates.retain(|_, vec| !vec.is_empty());
    }

    fn ord_by_name(&self) -> Vec<&Item> {
        // convert to Vec for O(N log(N)) sorting
        let mut items: Vec<&Item> = self.inventory.values().collect();
        items.sort_by(|a, b| a.name.cmp(&b.name));
        items // sort refs to avoid copying (low memory footprint)
    }

    fn count_id(&self, id: usize) -> usize {
        // TODO: should also return a bool to indicate count > 0?
        // TODO: should return an Option or Result to indicate count = 0?
        self.map_ids.get(&id).map_or(0, |v| *v)
    }

    fn count_name(&self, name: &str) -> usize {
        // TODO: should also return a bool to indicate count > 0?
        // TODO: should return an Option or Result to indicate count = 0?
        self.map_names.get(name).map_or(0, |v| *v)
    }

    fn find_id(&self, id: usize) -> Option<&Vec<Slot>> {
        // TODO: should also return a bool to indicate count > 0?
        // TODO: should return an Option or Result to indicate count = 0?
        self.map_slots.get(&id)
    }

    fn find_expired(&self, date: DateTime<Local>) -> Vec<Item> {
        self.map_dates
            .range(..=date)
            .flat_map(|(_, ids)| ids)
            .copied()
            .map(|s| s.as_tuple())
            .map(|(row, shelf, zone)| self.get_item(row, shelf, zone))
            .filter(|opt| opt.is_some())
            .map(|opt| opt.unwrap()) // safe to unwrap
            .cloned()
            .collect::<Vec<_>>()
    }
}

#[cfg(test)]
mod tests {
    use super::{Item, MAX_INVENTORY_SIZE, Manager, Quality, Slot};
    use crate::allocators::{RoundRobinAllocator, GreedyAllocator};
    use crate::errors::ManagerError;
    use chrono::{Local, NaiveDateTime, TimeZone};
    use itertools::Itertools;

    #[test]
    fn test_manager() {
        // FIXME: tbh this is not a good unit test because it relies on RoundRobin correctness
        //        e.g. checking expected Slots after assigning items
        //        Proper testing would require manually setting up the items in the desired slots
        let exp_date =
            NaiveDateTime::parse_from_str("2020-01-01 14:30:00", "%Y-%m-%d %H:%M:%S").unwrap();
        let exp_date = Local.from_local_datetime(&exp_date).unwrap(); // DateTime<Local>

        // no filters, RoundRobin
        let mut manager = Manager::new(RoundRobinAllocator::default(), Vec::new());
        // let mut manager = Manager::new(GreedyAllocator {}, Vec::new());

        let item0 = Item::new(0, "Flour", 10, Quality::Normal);
        let item1 = Item::new(1, "Wood", 5, Quality::OverSized { size: 2 });
        let item2 = Item::new(2, "Glass", 2, Quality::Fragile { expiration_date: exp_date, max_row: 1 });

        manager.insert_item(item0.clone()).unwrap();  // Normal
        manager.insert_item(item0.clone()).unwrap();  // Normal
        manager.insert_item(item0.clone()).unwrap();  // Normal
        manager.insert_item(item0.clone()).unwrap();  // Normal
        manager.insert_item(item1.clone()).unwrap();  // OverSized
        manager.insert_item(item0.clone()).unwrap();  // Normal
        manager.insert_item(item2.clone()).unwrap();  // Fragile
        manager.insert_item(item2.clone()).unwrap();  // Fragile
        manager.insert_item(item0.clone()).unwrap();  // Normal
        manager.insert_item(item2.clone()).unwrap();  // Fragile

        {
            let item0 = Item::new(0, "Flour", 10, Quality::Normal);
            let item1 = Item::new(1, "Wood", 5, Quality::OverSized { size: 2 });
            let item2 = Item::new(2, "Glass", 2, Quality::Fragile { expiration_date: exp_date, max_row: 1 });
            let ordered = manager.ord_by_name();
            assert_eq!(ordered.len(), 10);
            assert!(&ordered[0..6].iter().all_equal());
            assert!(&ordered[0..6].iter().all(|x| **x == item0));
            assert!(&ordered[6..9].iter().all_equal());
            assert!(&ordered[6..9].iter().all(|x| **x == item2));
            assert!(&ordered[9..9].iter().all_equal());
            assert!(&ordered[9..9].iter().all(|x| **x == item1));
        }

        assert_eq!(manager.count_id(0), 6);
        assert_eq!(manager.count_id(1), 1);
        assert_eq!(manager.count_id(2), 3);

        assert_eq!(manager.count_name("Flour"), 6);
        assert_eq!(manager.count_name("Wood"), 1);
        assert_eq!(manager.count_name("Glass"), 3);

        let slot = manager.find_id(1).unwrap();
        assert_eq!(slot.len(), 1);
        let slot = slot[0];
        assert_eq!(slot, Slot::from((0, 1, 1)));

        let expired = manager.find_expired(Local::now());
        assert_eq!(expired.len(), 3);
        assert!(expired.iter().all(|item| item == &item2));
    }
}



fn main() {

    // HARDCODED - CHANGE HERE
    let allocator = RoundRobinAllocator::default();
    // let allocator = GreedyAllocator::default();

    // PRESET FILTERS
    let tmp_string = format!("Ban over-sized items with size {MAX_INVENTORY_SIZE}");
    let tmp = tmp_string.as_str();
    let multiselected = &[
        "Max. 1 over-sized item allowed",
        "Max. 2 over-sized item allowed",
        "Max. 50 units of item ID:0",
        tmp,
    ];

    let mut filters = Vec::<Box<dyn Filter>>::new();
    filters.push(Box::from(LimitOverSized::new(1)));
    filters.push(Box::from(LimitOverSized::new(2)));
    filters.push(Box::from(LimitItemQuantity::new(0, 50)));
    filters.push(Box::from(BanQuality::new(Quality::OverSized {
        size: MAX_INVENTORY_SIZE,
    })));

    // CHOOSE FILTERS FROM PRESETS
    let defaults = &[false, false, false, false];
    let selections = MultiSelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Pick your filters")
        .items(&multiselected[..])
        .defaults(&defaults[..])
        .interact()
        .unwrap();

    if !selections.is_empty() {
        let keep: HashSet<usize> = selections.iter().copied().collect();
        let mut i = 0;
        filters.retain(|_| {
            let keep_it = keep.contains(&i);
            i += 1;
            keep_it
        });
    }

    // INIT MANAGER
    let mut manager = Manager::new(allocator, filters);

    loop {
        let selections = &[
            "Insert item",
            "Remove item",
            "Locate item",
            "Find items by ID",
            "Find items by name",
            "Find expired items",
            "List all items",
            "Quit",
        ];

        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Pick your allocation method")
            .default(0)
            .items(&selections[..])
            .interact()
            .unwrap();

        match selection {
            0 => {
                let id: usize = Input::with_theme(&ColorfulTheme::default())
                    .with_prompt("Input ID: ")
                    .interact_text()
                    .unwrap();
                let name: String = Input::with_theme(&ColorfulTheme::default())
                    .with_prompt("Input name: ")
                    .interact_text()
                    .unwrap();
                let quantity: usize = Input::with_theme(&ColorfulTheme::default())
                    .with_prompt("Input quantity: ")
                    .interact_text()
                    .unwrap();
                let quality_selections = &[
                    "Normal",
                    "Over-sized",
                    "Fragile",
                ];
                let quality_selection = Select::with_theme(&ColorfulTheme::default())
                    .with_prompt("Input quality: ")
                    .default(0)
                    .items(&quality_selections[..])
                    .interact()
                    .unwrap();
                let item = match quality_selection {
                    0 => Item::new(id, name.as_str(), quantity, Quality::Normal),
                    1 => {
                        let size: usize = Input::with_theme(&ColorfulTheme::default())
                            .with_prompt("Input size: ")
                            .interact_text()
                            .unwrap();
                        Item::new(id, name.as_str(), quantity, Quality::OverSized { size })
                    },
                    2 => {
                        let exp_date: String = Input::with_theme(&ColorfulTheme::default())
                            .with_prompt("Input expiration date (%Y-%m-%d %H:%M:%S): ")
                            .interact_text()
                            .unwrap();
                        let exp_date = NaiveDateTime::parse_from_str(
                            exp_date.as_str(), "%Y-%m-%d %H:%M:%S"
                        ).unwrap();
                        let exp_date = Local.from_local_datetime(&exp_date).unwrap();

                        let max_row: usize = Input::with_theme(&ColorfulTheme::default())
                            .with_prompt("Input max. row allowed: ")
                            .interact_text()
                            .unwrap();

                        Item::new(id, name.as_str(), quantity, Quality::Fragile { expiration_date: exp_date, max_row })
                    },
                    _ => todo!()
                };
                let result = manager.insert_item(item);
                match result {
                    Ok(_) => {
                        println!("{}", style("Item was inserted successfully!").green());
                    },
                    Err(ManagerError::FilteredItem { .. }) => {
                        println!("{}", style("Filters do not allow this item!").red());
                    },
                    Err(ManagerError::FailedAllocation { .. }) => {
                        println!("{}", style("Allocator could not find a suitable slot for this item!").red());
                    },
                    _ => todo!()
                }

            },
            1 => {
                let row: usize = Input::with_theme(&ColorfulTheme::default())
                    .with_prompt("Input row: ")
                    .interact_text()
                    .unwrap();
                let shelf: usize = Input::with_theme(&ColorfulTheme::default())
                    .with_prompt("Input shelf: ")
                    .interact_text()
                    .unwrap();
                let zone: usize = Input::with_theme(&ColorfulTheme::default())
                    .with_prompt("Input zone: ")
                    .interact_text()
                    .unwrap();
                manager.remove_item(row, shelf, zone);
            },
            2 => {
                let id: usize = Input::with_theme(&ColorfulTheme::default())
                    .with_prompt("Input ID: ")
                    .interact_text()
                    .unwrap();
                let result = manager.find_id(id);
                match result {
                    Some(vec) => {
                        println!("{} {:#?}", style("Found at: ").green(), vec);
                    },
                    None => {
                        println!("{}", style("Not found!").red());
                    }
                }
            }
            _ => unimplemented!()
        };


    }


}