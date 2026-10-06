//! Bike rentals by the hour or the day, picked up and returned at a store.
//!
//! An empty area for now: #235. Its routes go in `routes()`, its views
//! in `resources/views/rentals/`, its tests in `tests/rentals.rs`, and the
//! "About this page" entry of every GET route it adds in `explain.rs`.

pub mod explain;

/// The rentals area, registered in `src/lib.rs`.
pub struct Rentals;

impl renox::Module for Rentals {
    fn name(&self) -> &'static str {
        "rentals"
    }
}
