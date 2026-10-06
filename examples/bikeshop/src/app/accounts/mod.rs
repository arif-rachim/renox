//! Customer accounts: profile, addresses, orders, rentals and bikes in one place.
//!
//! An empty area for now: #238. The sign-in, registration and account pages come from Renox's `Auth` module (registered in `src/lib.rs`); their "About this page" entries live in this area's `explain.rs`. Its routes go in `routes()`, its views
//! in `resources/views/accounts/`, its tests in `tests/accounts.rs`, and the
//! "About this page" entry of every GET route it adds in `explain.rs`.

pub mod explain;

/// The accounts area, registered in `src/lib.rs`.
pub struct Accounts;

impl renox::Module for Accounts {
    fn name(&self) -> &'static str {
        "accounts"
    }
}
