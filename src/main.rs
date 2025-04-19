use chrono::{DateTime, Local};
use itertools::iproduct;
use std::collections::HashMap;
use std::convert::From;
use std::fmt::{Debug, Display, write};

const MAX_INVENTORY_SIZE: usize = 3; // TODO: same for row/shelf/zone?

// TODO: implement safeguards to Slot::new (e.g. MAX_INVENTORY_SIZE checks)
// TODO: implement Slot::distance method (Manhattan?)
// FIXME: drop usize aliases and use arrays instead?
//        (tuples -> heterogeneous data, which is not the case)
#[derive(Hash, PartialEq, Eq)]
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
}

#[derive(Debug)]
struct RoundRobinAllocator {}
impl AllocStrategy for RoundRobinAllocator {
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
            if inventory.get(&slot).is_none() {
                return Some(slot);
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
    // Row -> Shelf -> Zone -> Option<Item>
    inventory: HashMap<Slot, Item>,
    allocator: A,
}

impl<A> Manager<A>
where
    A: AllocStrategy,
{
    fn new(allocator: A) -> Manager<A> {
        Manager {
            inventory: HashMap::new(),
            allocator,
        }
    }

    fn insert_item(&mut self, item: Item) {
        // FIXME: should return a Result (Err = failed to allocate, no valid positions)
        let opt: Option<_> = self.allocator.alloc(&item, &self.inventory);
        let slot = opt.unwrap();
        self._insert_item(slot, item)
    }

    fn _insert_item(&mut self, slot: Slot, item: Item) {
        self.inventory.entry(slot).or_insert(item);
    }

    // TODO: separate Manager::get_item internal impl from public API
    fn get_item(&self, row: usize, shelf: usize, zone: usize) -> Option<&Item> {
        self.inventory.get(&Slot::from((row, shelf, zone)))
    }

    // TODO: separate Manager::get_item internal impl from public API
    fn remove_item(&mut self, row: usize, shelf: usize, zone: usize) -> Option<Item> {
        self.inventory.remove(&Slot::from((row, shelf, zone)))
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
        self.inventory.values().filter(|item| item.id == id).count()
    }

    fn count_name(&self, name: &str) -> usize {
        // TODO: should also return a bool to indicate count > 0?
        // TODO: should return an Option or Result to indicate count = 0?
        self.inventory
            .values()
            .filter(|item| item.name == name)
            .count()
    }
}

fn main() {
    println!("Hello, world!");
    let mut inv = Manager::new(RoundRobinAllocator {});
    inv.insert_item(Item::new(0, "Bolts", 10, Quality::Normal));
    inv.insert_item(Item::new(0, "Nuts", 10, Quality::Normal));
    inv.insert_item(Item::new(0, "Screws", 10, Quality::Normal));
    inv.insert_item(Item::new(1, "Bars", 10, Quality::Normal));
    inv.insert_item(Item::new(1, "Bits", 10, Quality::Normal));
    println!("{:#?}", inv);
    let sorted_items = inv.ord_by_name(); // active immutable borrow!
    println!("{:#?}", sorted_items); // lifetime ends here (no further uses)
    inv.insert_item(Item::new(2, "Plates", 10, Quality::Normal));
    println!("{:#?}", inv.count_id(0));
    println!("{:#?}", inv.count_id(100));
    println!("{:#?}", inv.count_name("Bolts"));
    println!("{:#?}", inv.count_name("Monkey"));
}
