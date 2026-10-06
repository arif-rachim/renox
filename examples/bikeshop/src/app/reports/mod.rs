//! Dashboards, reports and exports.
//!
//! An empty area for now: #242. Its routes go in `routes()`, its views
//! in `resources/views/reports/`, its tests in `tests/reports.rs`, and the
//! "About this page" entry of every GET route it adds in `explain.rs`.

pub mod explain;

/// The reports area, registered in `src/lib.rs`.
pub struct Reports;

impl renox::Module for Reports {
    fn name(&self) -> &'static str {
        "reports"
    }
}
