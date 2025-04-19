use chrono::{DateTime, Local};
use std::collections::HashMap;

const MAX_INVENTORY_SIZE: usize = 3; // TODO: same for row/shelf/zone?
type RowId = usize;
type ShelfId = usize;
type ZoneId = usize;

#[derive(Debug)]
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

#[derive(Debug)]
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
        inventory: &HashMap<RowId, HashMap<ShelfId, HashMap<ZoneId, Item>>>,
    ) -> Option<(RowId, ShelfId, ZoneId)>;
}

#[derive(Debug)]
struct Allocator {}
impl AllocStrategy for Allocator {
    fn alloc(
        &self,
        item: &Item,  // TODO: handle different variants of Quality
        inventory: &HashMap<RowId, HashMap<ShelfId, HashMap<ZoneId, Item>>>,
    ) -> Option<(RowId, ShelfId, ZoneId)> {
        // round-robin
        for row in 0..MAX_INVENTORY_SIZE {
            for shelf in 0..MAX_INVENTORY_SIZE {
                for zone in 0..MAX_INVENTORY_SIZE {
                    match inventory
                        .get(&row)
                        .and_then(|shelf_map| shelf_map.get(&shelf))
                        .and_then(|zone_map| zone_map.get(&zone))
                    {
                        Some(_) => continue,
                        None => {
                            return Some((row, shelf, zone));
                        }
                    }
                }
            }
        }
        None
    }
}

// struct Allocator2 {}
// impl AllocStrategy for Allocator2 {}

// TODO: should be selectable AT RUN TIME
trait Filter {}

#[derive(Debug)]
struct Manager<A>
where
    A: AllocStrategy,
{
    // Row -> Shelf -> Zone -> Option<Item>
    inventory: HashMap<RowId, HashMap<ShelfId, HashMap<ZoneId, Item>>>,
    item_map: HashMap<usize, Vec<ItemInfo>>,
    allocator: A,
}

impl<A> Manager<A>
where
    A: AllocStrategy,
{
    fn new(allocator: A) -> Manager<A> {
        Manager {
            inventory: HashMap::new(),
            item_map: HashMap::new(),
            allocator,
        }
    }

    fn insert_item(&mut self, item: Item) {
        // FIXME: should return a Result (Err = failed to allocate, no valid positions)
        let opt: Option<_> = self.allocator.alloc(&item, &self.inventory);
        let (row, shelf, zone) = opt.unwrap();
        self._insert_item(row, shelf, zone, item)
    }

    fn _insert_item(&mut self, row: usize, shelf: usize, zone: usize, item: Item) {
        self.inventory
            .entry(row)
            .or_default()
            .entry(shelf)
            .or_default()
            .insert(zone, item);
    }

    fn get_item(&self, row: usize, shelf: usize, zone: usize) -> Option<&Item> {
        self.inventory
            .get(&row)
            .and_then(|shelf_map| shelf_map.get(&shelf))
            .and_then(|zone_map| zone_map.get(&zone))
    }

    fn remove_item(&mut self, row: usize, shelf: usize, zone: usize) -> Option<Item> {
        self.inventory
            .get_mut(&row)
            .and_then(|shelf_map| shelf_map.get_mut(&shelf))
            .and_then(|zone_map| zone_map.remove(&zone))
    }
}

fn main() {
    println!("Hello, world!");
    let mut inv = Manager::new(Allocator {});
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
