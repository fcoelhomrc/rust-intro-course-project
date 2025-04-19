use chrono::{DateTime, Local};
use itertools::iproduct;
use std::collections::HashMap;
use std::fmt::{write, Debug, Display};
use std::convert::{From};

const MAX_INVENTORY_SIZE: usize = 3; // TODO: same for row/shelf/zone?
type RowId = usize;
type ShelfId = usize;
type ZoneId = usize;


// TODO: implement Slot::as_tuple method
// TODO: implement trait std::convert::From<(RowId, ShelfId, ZoneId)>
// TODO: implement safeguards to Slot::new (e.g. MAX_INVENTORY_SIZE checks)
// TODO: implement Slot::distance method (Manhattan?)
#[derive(Hash, PartialEq, Eq)]
struct Slot {
    position: (RowId, ShelfId, ZoneId)
}

impl Slot {
    fn new(position: (RowId, ShelfId, ZoneId)) -> Self {
        Self { position }
    }
    fn as_tuple(&self) -> (RowId, ShelfId, ZoneId) {
        (self.position.0, self.position.1, self.position.2)
    }
}

impl Display for Slot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}|{}|{}]", self.position.0, self.position.1, self.position.2)
    } // FIXME: choose a better string representation...
}

impl Debug for Slot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(self, f)
    }
}

impl From<(RowId, ShelfId, ZoneId)> for Slot {
    fn from(value: (RowId, ShelfId, ZoneId)) -> Self {
        Self { position: value }
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
            Quality::Fragile { expiration_date, max_dist } => write!(f, "Fragile ({}, {})", expiration_date, max_dist),
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
    write!(f, "[Item {}: {}] [Qty: {}, {}]", self.id, self.name, self.quantity, self.quality)
    }
}

impl Debug for Item {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(self, f)
    }
}


#[derive(Debug)]
struct ItemInfo {
    position: (usize, usize, usize),
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
    fn alloc(
        &self,
        item: &Item,
        inventory: &HashMap<Slot, Item>,
    ) -> Option<Slot>;
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
        self.inventory
            .entry(slot)
            .or_insert(item);
    }

    // TODO: separate Manager::get_item internal impl from public API
    fn get_item(&self, row: usize, shelf: usize, zone: usize) -> Option<&Item> {
        self.inventory.get(&Slot::new((row, shelf, zone)))
    }

    // TODO: separate Manager::get_item internal impl from public API
    fn remove_item(&mut self, row: usize, shelf: usize, zone: usize) -> Option<Item> {
        self.inventory.remove(&Slot::new((row, shelf, zone)))
    }
}

fn main() {
    println!("Hello, world!");
    let mut inv = Manager::new(RoundRobinAllocator {});
    println!("{:#?}", inv);
    inv.insert_item(Item::new(0, "Bolts", 10, Quality::Normal));
    println!("{:#?}", inv);
    inv.insert_item(Item::new(0, "Bolts", 10, Quality::Normal));
    println!("{:#?}", inv);
    inv.insert_item(Item::new(0, "Bolts", 10, Quality::Normal));
    println!("{:#?}", inv);
    inv.insert_item(Item::new(0, "Bolts", 10, Quality::Normal));
    println!("{:#?}", inv);
    inv.insert_item(Item::new(0, "Bolts", 10, Quality::Normal));
    println!("{:#?}", inv);
}
