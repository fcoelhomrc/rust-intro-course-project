use std::collections::HashMap;
use std::fmt::{Debug, Display};
use crate::{Item, Quality, Slot};

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