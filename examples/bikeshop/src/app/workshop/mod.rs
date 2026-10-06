//! The workshop: customers' bikes, service bookings and work orders.
//!
//! An empty area for now: #236. Its routes go in `routes()`, its views
//! in `resources/views/workshop/`, its tests in `tests/workshop.rs`, and the
//! "About this page" entry of every GET route it adds in `explain.rs`.

pub mod explain;
pub mod factories;
pub mod model;

/// The workshop area, registered in `src/lib.rs`.
pub struct Workshop;

impl renox::Module for Workshop {
    fn name(&self) -> &'static str {
        "workshop"
    }
}
