use thiserror::Error;

use crate::{Slot, Item, Filter, AllocStrategy};

// TODO: think about recovery + derive Error with macro
#[derive(Error, Debug)]
pub enum ManagerError<A: AllocStrategy> {
    #[error("{item:?} was rejected by some filter: {filters:?}")]
    FilteredItem {
        item: Item,
        filters: Box<Vec<dyn Filter>>,
    },
    #[error("{allocator:?} did not find a valid slot for {item:?}")]
    FailedAllocation{
        allocator: A,
        item: Item,
    },
    #[error("No items found in slot {slot:?}")]
    NotFound{
        slot: Slot,
    },
}