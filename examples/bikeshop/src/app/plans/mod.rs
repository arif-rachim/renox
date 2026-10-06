//! Service plans: subscriptions whose workshop visits are scheduled automatically.
//!
//! An empty area for now: #237. Its routes go in `routes()`, its views
//! in `resources/views/plans/`, its tests in `tests/plans.rs`, and the
//! "About this page" entry of every GET route it adds in `explain.rs`.

pub mod explain;
pub mod factories;
pub mod model;

/// The plans area, registered in `src/lib.rs`.
pub struct Plans;

impl renox::Module for Plans {
    fn name(&self) -> &'static str {
        "plans"
    }
}
