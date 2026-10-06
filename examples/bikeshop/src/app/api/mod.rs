//! The JSON API for kiosks and a customer app.
//!
//! An empty area for now: #241. Its routes go in `routes()`, its views
//! in `resources/views/api/`, its tests in `tests/api.rs`, and the
//! "About this page" entry of every GET route it adds in `explain.rs`.

pub mod explain;

/// The api area, registered in `src/lib.rs`.
pub struct Api;

impl renox::Module for Api {
    fn name(&self) -> &'static str {
        "api"
    }
}
