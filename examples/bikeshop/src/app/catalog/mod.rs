//! The public catalogue: bikes, gear and spare parts, product pages and search.
//!
//! An empty area for now: #233. Its routes go in `routes()`, its views
//! in `resources/views/catalog/`, its tests in `tests/catalog.rs`, and the
//! "About this page" entry of every GET route it adds in `explain.rs`.

pub mod explain;
pub mod factories;
pub mod model;

/// The catalog area, registered in `src/lib.rs`.
pub struct Catalog;

impl renox::Module for Catalog {
    fn name(&self) -> &'static str {
        "catalog"
    }
}
