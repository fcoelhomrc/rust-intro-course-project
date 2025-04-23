use crate::{Item, Quality, Slot};
use std::collections::HashMap;
use std::fmt::{Debug, Display};

// TODO: should be selectable AT RUN TIME
pub trait Filter: Display + Debug {
    // Using &mut self to allow for internal states
    fn filter(&self, item: &Item, inventory: &HashMap<Slot, Item>) -> bool;
}

#[derive(Debug)]
pub struct LimitOverSized {
    max_allowed: usize,
}

impl LimitOverSized {
    pub fn new(max_allowed: usize) -> Self {
        LimitOverSized { max_allowed }
    }
}

impl Filter for LimitOverSized {
    fn filter(&self, item: &Item, inventory: &HashMap<Slot, Item>) -> bool {
        if matches!(item.quality, Quality::Normal | Quality::Fragile { .. }) {
            return true;
        }
        let count = inventory
            .values()
            .filter(|item| matches!(item.quality, Quality::OverSized { .. }))
            .count();
        count < self.max_allowed
    }
}

impl Display for LimitOverSized {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "LimitOverSized({})", self.max_allowed)
    }
}

// TODO: Support a list of ids instead of a single Item id
// TODO: Use reverse map to find IDs instead of searching (more efficient)
#[derive(Debug)]
pub struct LimitItemQuantity {
    id: usize,
    max_allowed: usize,
}

impl LimitItemQuantity {
    pub fn new(id: usize, max_allowed: usize) -> Self {
        LimitItemQuantity { id, max_allowed }
    }
}

impl Filter for LimitItemQuantity {
    fn filter(&self, item: &Item, inventory: &HashMap<Slot, Item>) -> bool {
        if item.id != self.id {
            return true;
        };
        let total = inventory
            .values()
            .filter(|item| item.id == self.id)
            .map(|item| item.quantity)
            .sum::<usize>();
        total + item.quantity <= self.max_allowed
    }
}

impl Display for LimitItemQuantity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "LimitItemQuantity({}, {})", self.id, self.max_allowed)
    }
}

#[derive(Debug)]
pub struct BanQuality {
    quality: Quality,
}

impl BanQuality {
    pub fn new(quality: Quality) -> Self {
        BanQuality { quality }
    }
}

impl Filter for BanQuality {
    fn filter(&self, item: &Item, inventory: &HashMap<Slot, Item>) -> bool {
        match (&self.quality, &item.quality) {
            (q1, q2) if q1 == q2 => false,
            (_, _) => true,
        }
    }
}

impl Display for BanQuality {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "BanQuality({})", self.quality)
    }
}

#[cfg(test)]
mod tests {
    use super::{BanQuality, Filter, LimitItemQuantity, LimitOverSized};
    use crate::allocators::RoundRobinAllocator;
    use crate::errors::ManagerError;
    use crate::{Item, MAX_INVENTORY_SIZE, Manager, Quality, Slot};
    use chrono::{Local, NaiveDateTime, TimeZone};
    #[test]
    fn test_filters() {
        let mut filters = Vec::<Box<dyn Filter>>::new();
        filters.push(Box::from(LimitOverSized::new(1)));
        filters.push(Box::from(LimitItemQuantity::new(2, 50)));
        filters.push(Box::from(BanQuality::new(Quality::OverSized {
            size: MAX_INVENTORY_SIZE,
        })));

        let mut manager = Manager::new(RoundRobinAllocator::default(), filters);

        // LimitOverSized
        let allowed_item = Item::new(0, "A", 1, Quality::OverSized { size: 1 });

        let result = manager.insert_item(allowed_item);
        assert!(result.is_ok());

        let forbidden_item = Item::new(1, "B", 1, Quality::OverSized { size: 2 });
        let result = manager.insert_item(forbidden_item.clone());
        assert!(result.is_err_and(|err| matches!(
            err,
            ManagerError::FilteredItem { .. }
        )));

        manager.remove_item(0, 0, 0);

        // LimitItemQuantity
        let item = Item::new(2, "C", 10, Quality::Normal);
        for _ in 0..5 {
            let result = manager.insert_item(item.clone());
            assert!(result.is_ok()); // total quantity -> 50
        };
        let item = Item::new(2, "C", 1, Quality::Normal);
        let result = manager.insert_item(item.clone());
        println!("{:#?}", manager);
        assert!(result.is_err_and(|err| matches!(
            err,
            ManagerError::FilteredItem { .. }
        )));

        // BanQuality
        let forbidden_item = Item::new(3, "D", 1, Quality::OverSized { size: MAX_INVENTORY_SIZE });
        let result = manager.insert_item(forbidden_item.clone());
        assert!(result.is_err_and(|err| matches!(
            err,
            ManagerError::FilteredItem { .. }
        )));
    }
}
