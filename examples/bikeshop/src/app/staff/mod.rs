//! The staff side: the back office's home and, later, staff, roles and the
//! stores' settings (#239).
//!
//! For now only the dashboard placeholder, so the staff layout
//! (`resources/views/layouts/staff.html`: the kit's `sidebar` in an
//! `rx-shell`) has a page. Like every staff route it goes through
//! `access::staff_routes`: a login, then `staff.access` in the active store
//! (`src/app/access`).

pub mod explain;
pub mod factories;
pub mod model;
pub mod two_factor;

use renox::prelude::*;

/// The staff area.
pub struct Staff;

impl Module for Staff {
    fn name(&self) -> &'static str {
        "staff"
    }

    fn routes(&self) -> Routes {
        // Logged in, and working in a store where a role grants
        // `staff.access` today (the active store, src/app/access).
        crate::app::access::staff_routes(
            Routes::new()
                .get("/staff", dashboard)
                .name("staff.dashboard"),
        )
    }

    fn register(&self, app: &mut Registry) {
        // Two-factor login is required for staff (two_factor.rs).
        app.listen(two_factor::on_logged_in)
            .listen(two_factor::on_enabled)
            .listen(two_factor::on_disabled);
    }
}

async fn dashboard() -> View {
    view("staff/dashboard.html", context! {})
}
