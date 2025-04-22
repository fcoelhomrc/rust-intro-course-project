use thiserror::Error;

use crate::{Item, Slot};
#[derive(Error, Debug)]
pub enum ManagerError {
    #[error("{item:?} was rejected by some filter: {filters:?}")]
    FilteredItem {
        item: Item,
        filters: Vec<String>, // FIXME: ideally, should be the triggered filter
    },
    #[error("{allocator:?} did not find a valid slot for {item:?}")]
    FailedAllocation {
        allocator: String,  // FIXME: ideally, should be a clone of Allocator with its internal state
        item: Item,
    },
    #[error("No items found in slot {slot:?}")]
    NotFound { slot: Slot },
}
