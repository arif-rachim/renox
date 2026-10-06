//! The staff side: the back office's home and, later, staff, roles and the
//! stores' settings (#239).
//!
//! For now only the dashboard placeholder, so the staff layout
//! (`resources/views/layouts/staff.html`: the kit's `sidebar` in an
//! `rx-shell`) has a page. The permission check (`staff.access` or similar)
//! comes with the access foundations (`src/app/access`); until then the
//! route only needs a login.

pub mod explain;

use renox::prelude::*;

/// The staff area.
pub struct Staff;

impl Module for Staff {
    fn name(&self) -> &'static str {
        "staff"
    }

    fn routes(&self) -> Routes {
        Routes::new()
            .get("/staff", dashboard)
            .name("staff.dashboard")
            .require_auth()
    }
}

async fn dashboard() -> View {
    view("staff/dashboard.html", context! {})
}
