//! The workshop: customers' bikes, service bookings and work orders.
//!
//! For now the fleet repairs the rentals area asks for (#235); the
//! customers' and mechanics' pages are #236. Its views live in
//! `resources/views/workshop/`, its tests in `tests/workshop.rs`, and the
//! "About this page" entry of every GET route it adds in `explain.rs`.

pub mod explain;
pub mod factories;
pub mod fleet;
pub mod model;

use renox::prelude::*;

use crate::app::rentals::FleetRepairNeeded;

/// The workshop area, registered in `src/lib.rs`.
pub struct Workshop;

impl Module for Workshop {
    fn name(&self) -> &'static str {
        "workshop"
    }

    fn register(&self, app: &mut Registry) {
        // A damaged or worn rental bike: a fleet work order, billed to its owner store.
        app.listen(|event: FleetRepairNeeded, state: AppState| async move {
            fleet::open_repair(&state, &event).await?;
            Ok(())
        });
    }
}
