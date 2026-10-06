//! Cart, checkout, payments and counter sales.
//!
//! An empty area for now: #234. Its routes go in `routes()`, its views
//! in `resources/views/sales/`, its tests in `tests/sales.rs`, and the
//! "About this page" entry of every GET route it adds in `explain.rs`.

pub mod explain;

/// The sales area, registered in `src/lib.rs`.
pub struct Sales;

impl renox::Module for Sales {
    fn name(&self) -> &'static str {
        "sales"
    }
}
