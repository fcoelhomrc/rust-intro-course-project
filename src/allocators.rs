use crate::{Item, MAX_INVENTORY_SIZE, Quality, Slot};
use itertools::{Itertools, iproduct};
use std::collections::HashMap;
use std::fmt::{Debug, Display};

// TODO: should be selectable AT COMPILE TIME
pub trait AllocStrategy: Display + Debug {
    // FIXME: I don't like to require alloc to be &mut self,
    //        but using an internal state in RoundRobin requires it
    //        (otherwise we'd need to update internal state in a separate call,
    //        which might break the abstraction as GreedyAllocator doesn't need internal state)
    fn alloc(&mut self, item: &Item, inventory: &HashMap<Slot, Item>) -> Option<Slot>;

    fn is_slot_available(&self, slot: &Slot, item: &Item, inventory: &HashMap<Slot, Item>) -> bool {
        let size = self.get_item_size(item);
        if slot.zone + size > MAX_INVENTORY_SIZE {
            return false;
        }

        let end = slot.zone + size;
        // let end = std::cmp::min(slot.zone + size, MAX_INVENTORY_SIZE);
        // check if there are enough free zones from current position onwards
        let is_blocked_forward = (slot.zone..end)
            .any(|z| inventory.contains_key(&Slot::from((slot.row, slot.shelf, z))));
        if is_blocked_forward {
            return false;
        }
        // check if there are previous items blocking current position
        let is_blocked_backward = (0..slot.zone)
            // skip empty positions and return refs to items of occupied positions
            .filter_map(|zone| {
                inventory
                    .get(&Slot::from((slot.row, slot.shelf, zone)))
                    .map(|item| (item, zone))
            })
            // extract Item size of occupied positions
            .map(|(item, zone)| (self.get_item_size(item), zone))
            // check if it blocks current position
            .any(|(size, zone)| size + zone > slot.zone);
        !is_blocked_backward // -> is_available
    }

    fn get_item_size(&self, item: &Item) -> usize {
        match item.quality {
            Quality::Normal | Quality::Fragile { .. } => 1,
            Quality::OverSized { size } => size,
        }
    }
}

#[derive(Debug)]
pub struct RoundRobinAllocator {
    prev_alloc: Option<Slot>,
}

impl RoundRobinAllocator {
    fn get_prev_alloc(&self) -> &Option<Slot> {
        &self.prev_alloc
    }

    fn set_prev_alloc(&mut self, new_alloc: Option<Slot>) {
        self.prev_alloc = new_alloc;
    }

    fn get_start_pos(&self) -> (usize, usize, usize) {
        match &self.prev_alloc {
            Some(slot) => slot.as_tuple(),
            None => (0, 0, 0),
        }
    }
}

impl Default for RoundRobinAllocator {
    fn default() -> Self {
        Self { prev_alloc: None }
    }
}

impl Display for RoundRobinAllocator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "RoundRobinAllocator {{ prev_alloc: {:?} }}",
            self.prev_alloc
        )
    }
}

impl AllocStrategy for RoundRobinAllocator {
    fn alloc(&mut self, item: &Item, inventory: &HashMap<Slot, Item>) -> Option<Slot> {
        // round-robin
        let (row_start, shelf_start, zone_start) = self.get_start_pos();
        for (row, shelf, zone) in iproduct!(
            row_start..MAX_INVENTORY_SIZE,
            shelf_start..MAX_INVENTORY_SIZE,
            zone_start..MAX_INVENTORY_SIZE
        ) {
            let slot = Slot::from((row, shelf, zone));
            // handles interactions with Quality::OverSized
            if !self.is_slot_available(&slot, item, inventory) {
                continue;
            }
            match &item.quality {
                Quality::Normal | Quality::OverSized { .. } => {
                    self.set_prev_alloc(Some(slot)); // Slot is Copy
                    return Some(slot);
                }
                Quality::Fragile { max_row, .. } if &slot.row <= max_row => {
                    self.set_prev_alloc(Some(slot)); // Slot is Copy
                    return Some(slot);
                }
                _ => continue,
            }
        }
        self.set_prev_alloc(None); // failed alloc, reset search indexing
        None
    }
}

#[derive(Debug)]
struct GreedyAllocator {}

impl GreedyAllocator {
    // I think this implementation is a bit messy, but it was the best I could come up with.
    // My main worry was ensuring lazy-evaluation,
    // because the number of possibilities are combinatorial with dist.
    // For a given distance, return an iterator over all possible tuples with that distance
    // where distance is defined as the Manhattan distance d(x,y,z) = x + y + z
    // First, we generate all sets of numbers summing to dist
    // Then, we generate all possible permutations (itertools)
    // Then, we filter out repeated permutations (itertools)
    // Finally, we return each Slot
    fn slots_by_distance(dist: usize) -> impl Iterator<Item = Slot> {
        (0..=dist)
            .flat_map(move |i| {
                (0..=dist - i).flat_map(move |j| {
                    let k = dist - i - j;
                    [i, j, k]
                        .iter()
                        .copied()
                        .permutations(3)
                        .collect::<Vec<_>>()
                })
            })
            .unique()
            .filter(|perm| {
                perm[0] < MAX_INVENTORY_SIZE
                    && perm[1] < MAX_INVENTORY_SIZE
                    && perm[2] < MAX_INVENTORY_SIZE
            })
            .map(|perm| Slot::from((perm[0], perm[1], perm[2])))
    }
}

impl Display for GreedyAllocator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "GreedyAllocator")
    }
}

impl AllocStrategy for GreedyAllocator {
    fn alloc(&mut self, item: &Item, inventory: &HashMap<Slot, Item>) -> Option<Slot> {
        for dist in 0..=3 * (MAX_INVENTORY_SIZE - 1) {
            for slot in GreedyAllocator::slots_by_distance(dist) {
                if !self.is_slot_available(&slot, item, inventory) {
                    continue;
                }
                match &item.quality {
                    Quality::Normal | Quality::OverSized { .. } => return Some(slot),
                    Quality::Fragile { max_row, .. } if &slot.row <= max_row => {
                        return Some(slot);
                    }
                    _ => continue,
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::{GreedyAllocator, RoundRobinAllocator};
    use crate::errors::ManagerError;
    use crate::{Item, MAX_INVENTORY_SIZE, Manager, Quality, Slot};
    use chrono::{Local, NaiveDateTime, TimeZone};
    #[test]
    fn test_round_robin_allocator() {
        let mut manager = Manager::new(
            RoundRobinAllocator::default(),
            Vec::new(), // no filters
        );

        let result = manager.insert_item(Item::new(
            0,
            "A",
            1,
            Quality::OverSized {
                size: MAX_INVENTORY_SIZE,
            },
        ));
        assert!(result.is_ok());

        let result = manager.insert_item(Item::new(1, "B", 1, Quality::Normal));
        assert!(result.is_ok());

        let result = manager.insert_item(Item::new(2, "C", 1, Quality::Normal));
        assert!(result.is_ok());

        assert_eq!(
            manager.get_item(0, 0, 0),
            Some(&Item::new(
                0,
                "A",
                1,
                Quality::OverSized {
                    size: MAX_INVENTORY_SIZE
                }
            ))
        );
        assert_eq!(
            manager.get_item(0, 1, 0),
            Some(&Item::new(1, "B", 1, Quality::Normal))
        );
        assert_eq!(
            manager.get_item(0, 1, 1),
            Some(&Item::new(2, "C", 1, Quality::Normal))
        );

        assert!(manager.allocator.prev_alloc.is_some());
        assert_eq!(manager.allocator.prev_alloc.unwrap(), Slot::from((0, 1, 1)));

        let result = manager.insert_item(Item::new(3, "D", 1, Quality::Normal));
        manager.remove_item(0, 1, 0);
        manager.remove_item(0, 1, 1);

        println!("{:#?}", &manager);
        assert!(result.is_ok());
        assert_eq!(
            manager.get_item(0, 1, 2),
            Some(&Item::new(3, "D", 1, Quality::Normal))
        );

        let result = manager.insert_item(Item::new(
            4,
            "E",
            1,
            Quality::OverSized {
                size: MAX_INVENTORY_SIZE + 1,
            },
        ));

        assert!(result.is_err()); // failed alloc -> reset prev_alloc
        assert!(manager.allocator.prev_alloc.is_none());

        let result = manager.insert_item(Item::new(5, "F", 1, Quality::OverSized { size: 2 })); // fills spot opened by the two removals

        assert!(result.is_ok());
        println!("{:#?}", &manager);
        assert_eq!(
            manager.get_item(0, 1, 0),
            Some(&Item::new(5, "F", 1, Quality::OverSized { size: 2 }))
        );
    }

    #[test]
    fn test_greedy_allocator() {
        let mut manager = Manager::new(
            GreedyAllocator {},
            Vec::new(), // no filters
        );

        let result = manager.insert_item(Item::new(
            0,
            "A",
            1,
            Quality::OverSized {
                size: MAX_INVENTORY_SIZE,
            },
        ));
        assert!(result.is_ok());

        let result = manager.insert_item(Item::new(1, "B", 1, Quality::Normal));
        assert!(result.is_ok());

        let result = manager.insert_item(Item::new(2, "C", 1, Quality::Normal));
        assert!(result.is_ok());

        assert_eq!(
            manager.get_item(0, 0, 0),
            Some(&Item::new(
                0,
                "A",
                1,
                Quality::OverSized {
                    size: MAX_INVENTORY_SIZE
                }
            ))
        );
        assert_eq!(
            manager.get_item(0, 1, 0),
            Some(&Item::new(1, "B", 1, Quality::Normal))
        );
        assert_eq!(
            manager.get_item(1, 0, 0),
            Some(&Item::new(2, "C", 1, Quality::Normal))
        );

        let result = manager.insert_item(Item::new(3, "D", 1, Quality::Normal));

        assert!(result.is_ok());
        assert_eq!(
            manager.get_item(0, 2, 0),
            Some(&Item::new(3, "D", 1, Quality::Normal))
        );

        let exp_date =
            NaiveDateTime::parse_from_str("2020-01-01 14:30:00", "%Y-%m-%d %H:%M:%S").unwrap();
        let exp_date = Local.from_local_datetime(&exp_date).unwrap(); // DateTime<Local>

        let result = manager.insert_item(Item::new(
            4,
            "E",
            1,
            Quality::Fragile {
                expiration_date: exp_date.clone(),
                max_row: 1,
            },
        ));

        assert!(result.is_ok());
        assert_eq!(
            manager.get_item(0, 1, 1),
            Some(&Item::new(
                4,
                "E",
                1,
                Quality::Fragile {
                    expiration_date: exp_date.clone(),
                    max_row: 1
                }
            ))
        );

        let result = manager.insert_item(Item::new(5, "F", 1, Quality::Normal));

        assert!(result.is_ok());
        assert_eq!(
            manager.get_item(2, 0, 0),
            Some(&Item::new(5, "F", 1, Quality::Normal))
        );

        let result = manager.insert_item(Item::new(6, "G", 1, Quality::Normal));

        assert!(result.is_ok());
        assert_eq!(
            manager.get_item(1, 0, 1),
            Some(&Item::new(6, "G", 1, Quality::Normal))
        );
    }
}
