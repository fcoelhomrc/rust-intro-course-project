use thiserror;

pub enum ManagerError {
    FilteredItem,
    FailedAllocation,
    NotFound,
}

