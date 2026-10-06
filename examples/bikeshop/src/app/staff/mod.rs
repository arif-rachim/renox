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
}

/// `GET /staff` (`staff.dashboard`): for people who see reports in the
/// active store, its last 7 days (`reports::dashboard::overview`, #242).
async fn dashboard(State(state): State<AppState>, user: AuthUser) -> Result<View> {
    let overview = crate::app::reports::dashboard::overview(&state, &user).await?;
    Ok(view("staff/dashboard.html", context! { overview }))
}
