use chrono::{DateTime, Local};
use itertools::iproduct;
use std::collections::HashMap;
use std::convert::From;
use std::fmt::{Debug, Display};
use std::usize::MAX;
// NOTE: Quality::Fragile is handled as follows:
//       The slot distance (Manhattan distance) must be <= Quality::Fragile { max_dist, .. }
// TODO: How should we represent Slot occupied by a Quality::OverSized Item?

const MAX_INVENTORY_SIZE: usize = 3; // TODO: same for row/shelf/zone?

// TODO: implement safeguards to Slot::new (e.g. MAX_INVENTORY_SIZE checks)
// TODO: implement Slot::distance method (Manhattan?)
// FIXME: drop usize aliases and use arrays instead?
//        (tuples -> heterogeneous data, which is not the case)
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

enum Quality {
    Fragile {
        expiration_date: String,
        max_dist: usize,
    }, // TODO: what distance? euclidean? manhattan?
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
                max_dist,
            } => write!(f, "Fragile ({}, {})", expiration_date, max_dist),
            Quality::OverSized { size } => write!(f, "OverSized ({})", size),
            Quality::Normal => write!(f, "Normal"),
        }
    }
}

struct Item {
    id: usize,
    name: String,
    quantity: usize,
    quality: Quality,
    // additional fields
    info: Option<ItemInfo>,
}

impl Item {
    fn new(id: usize, name: &str, quantity: usize, quality: Quality) -> Self {
        Self {
            id,
            name: name.to_string(),
            quantity,
            quality,
            info: None,
        }
    }

    fn set_info(&mut self, info: ItemInfo) {
        self.info = Some(info);
    }
}

impl Display for Item {
    // FIXME: add self.info (need to impl Display for ItemInfo)
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[Item {}: {}] [Qty: {}, {}]",
            self.id, self.name, self.quantity, self.quality
        )
    }
}

impl Debug for Item {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(self, f)
    }
}

#[derive(Debug)]
struct ItemInfo {
    position: (usize, usize, usize), // FIXME: is this necessary?
    timestamp: DateTime<Local>,
}

impl ItemInfo {
    fn new(position: (usize, usize, usize)) -> Self {
        Self {
            position,
            timestamp: Local::now(),
        }
    }
}

// TODO: should be selectable AT COMPILE TIME
trait AllocStrategy {
    fn alloc(&self, item: &Item, inventory: &HashMap<Slot, Item>) -> Option<Slot>;

    fn is_slot_available(
        &self, slot: &Slot, item: &Item, inventory: &HashMap<Slot, Item>) -> bool
    {
        let size = self.get_item_size(item);
        let end = std::cmp::min(slot.zone + size, MAX_INVENTORY_SIZE);
        // check if there are enough free zones from current position onwards
        let is_blocked_forward = (slot.zone..end)
            .any(|z| inventory.contains_key(&Slot::from((slot.row, slot.shelf, z))));
        if is_blocked_forward { return false; }
        // check if there are previous items blocking current position
        let is_blocked_backward = (0..slot.zone)
            // skip empty positions and return refs to items of occupied positions
            .filter_map(|zone| {
                inventory.get(&Slot::from((slot.row, slot.shelf, zone))).map(|item| (item, zone))
            })
            // extract Item size of occupied positions
            .map(|(item, zone)| {
                (self.get_item_size(item), zone)
            })
            // check if it blocks current position
            .any(|(size, zone)| {
                size + zone > slot.zone
            });
        !is_blocked_backward  // -> is_available
    }

    fn get_item_size(&self, item: &Item) -> usize {
        match item.quality {
            Quality::Normal | Quality::Fragile { .. } => 1,
            Quality::OverSized { size } => size,
        }
    }
}

#[derive(Debug)]
struct RoundRobinAllocator {}

impl AllocStrategy for RoundRobinAllocator {
    // FIXME: O(N³), but can be improved by starting search from the last allocated position.
    //        This optimization needs to consider that removing items frees previous positions.
    // FIXME: Normal item needs to check if slot is occupied by a previous OverSized item
    fn alloc(
        &self,
        item: &Item, // TODO: handle different variants of Quality
        inventory: &HashMap<Slot, Item>,
    ) -> Option<Slot> {
        // round-robin
        for (row, shelf, zone) in iproduct!(
            0..MAX_INVENTORY_SIZE,
            0..MAX_INVENTORY_SIZE,
            0..MAX_INVENTORY_SIZE
        ) {
            let slot = Slot::from((row, shelf, zone));
            // handles interactions with Quality::OverSized
            if !self.is_slot_available(&slot, item, inventory) {
                continue;
            }
            match &item.quality {
                Quality::Normal | Quality::OverSized { .. } => return Some(slot),
                Quality::Fragile { max_dist, .. } if slot.distance() <= *max_dist => {
                    return Some(slot);
                }
                _ => continue,
            }
        }
        None
    }
}

// TODO: implement GreedyAllocator (shortest distance)
struct GreedyAllocator {}
// impl AllocStrategy for Allocator2 {}

// TODO: should be selectable AT RUN TIME
trait Filter {}

#[derive(Debug)]
struct Manager<A>
where
    A: AllocStrategy,
{
    inventory: HashMap<Slot, Item>,
    allocator: A,

    // reverse-maps
    map_ids: HashMap<usize, usize>,       // id, count
    map_names: HashMap<String, usize>,    // name, count
    map_slots: HashMap<usize, Vec<Slot>>, // id, list of slots
}

impl<A> Manager<A>
where
    A: AllocStrategy,
{
    fn new(allocator: A) -> Manager<A> {
        Manager {
            inventory: HashMap::new(),
            allocator,

            map_ids: HashMap::new(),
            map_names: HashMap::new(),
            map_slots: HashMap::new(),
        }
    }

    fn insert_item(&mut self, item: Item) {
        // FIXME: should return a Result (Err = failed to allocate, no valid positions)
        let opt: Option<_> = self.allocator.alloc(&item, &self.inventory);
        let slot = opt.unwrap();
        self._update_maps_on_insert(&slot, &item);
        self._insert_item(slot, item)
    }

    fn _insert_item(&mut self, slot: Slot, item: Item) {
        self.inventory.entry(slot).or_insert(item);
    }

    fn _update_maps_on_insert(&mut self, slot: &Slot, item: &Item) {
        *self.map_ids.entry(item.id).or_insert(0) += 1;
        *self.map_names.entry(item.name.clone()).or_insert(0) += 1;
        self.map_slots.entry(item.id).or_insert(vec![]).push(*slot);
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
        *self.map_ids.entry(item.id).or_insert(0) += 1;
        *self.map_names.entry(item.name.clone()).or_insert(0) += 1;
        self.map_slots.entry(item.id).or_insert(vec![]).push(*slot);
    }

    fn ord_by_name(&self) -> Vec<&Item> {
        // convert to Vec for O(N log(N)) sorting
        let mut items: Vec<&Item> = self.inventory.values().collect();
        items.sort_by(|a, b| a.name.cmp(&b.name));
        items // sort refs to avoid copying (low memory footprint)
    }

    // FIXME: counting methods will fail depending on how we implement over-sized items
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
}

fn main() {
    let mut inv = Manager::new(RoundRobinAllocator {});
    inv.insert_item(Item::new(
        0,
        "Bolts",
        10,
        Quality::OverSized {
            size: 2,
        },
    ));
    inv.insert_item(Item::new(0, "Bolts", 10, Quality::Normal));
    inv.insert_item(Item::new(1, "Screws", 10, Quality::Normal));
    inv.insert_item(Item::new(
        3,
        "Bits",
        10,
        Quality::Fragile {
            expiration_date: "20".to_string(),
            max_dist: 1,
        },
    ));
    println!("{:#?}", inv);
    let sorted_items = inv.ord_by_name(); // active immutable borrow!
    println!("{:#?}", sorted_items); // lifetime ends here (no further uses)
    inv.insert_item(Item::new(2, "Plates", 10, Quality::Normal));
    println!("{:#?}", inv.count_id(0));
    println!("{:#?}", inv.count_name("Bolts"));
}
