//! Stock, consignment between stores, suppliers and purchasing.
//!
//! An empty area for now: #240. Its routes go in `routes()`, its views
//! in `resources/views/stock/`, its tests in `tests/stock.rs`, and the
//! "About this page" entry of every GET route it adds in `explain.rs`.

pub mod explain;

/// The stock area, registered in `src/lib.rs`.
pub struct Stock;

impl renox::Module for Stock {
    fn name(&self) -> &'static str {
        "stock"
    }
}
