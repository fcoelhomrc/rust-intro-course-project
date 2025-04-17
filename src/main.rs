use chrono::{Date, DateTime, Local};
use std::collections::HashMap;

const INVENTORY_SIZE: usize = 50;

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

struct Manager {
    // Rows -> Shelf -> Zone -> Item
    inventory: [[[Option<Item>; INVENTORY_SIZE]; INVENTORY_SIZE]; INVENTORY_SIZE],
    item_map: HashMap<usize, Vec<ItemInfo>>,
}

impl Manager {
    fn new() -> Self {
        let inventory =
            std::array::from_fn(|_| std::array::from_fn(|_| std::array::from_fn(|_| None)));

        Self {
            inventory,
            item_map: HashMap::new(),
        }
    }
}

fn main() {
    println!("Hello, world!");
}
