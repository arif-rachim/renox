//! Access: who may do what, and in which store (RBAC + ABAC).
//!
//! An empty area for now: the permission catalogue, the active store and the policy helpers every area checks against (the next part of #232, used by #239 and #245). It has no pages of its own: the staff pages for roles live in `staff`. Its routes go in `routes()`, its views
//! in `resources/views/access/`, its tests in `tests/access.rs`, and the
//! "About this page" entry of every GET route it adds in `explain.rs`.

pub mod explain;

/// The access area, registered in `src/lib.rs`.
pub struct Access;

impl renox::Module for Access {
    fn name(&self) -> &'static str {
        "access"
    }
}
