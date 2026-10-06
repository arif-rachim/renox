//! Multi-store operations: staff helping other stores, bikes and goods placed at other stores, intercompany books.
//!
//! An empty area for now: #245. Its routes go in `routes()`, its views
//! in `resources/views/multistore/`, its tests in `tests/multistore.rs`, and the
//! "About this page" entry of every GET route it adds in `explain.rs`.

pub mod audit;
pub mod books;
pub mod explain;
pub mod factories;
pub mod model;

/// The multistore area, registered in `src/lib.rs`.
pub struct Multistore;

impl renox::Module for Multistore {
    fn name(&self) -> &'static str {
        "multistore"
    }
}
