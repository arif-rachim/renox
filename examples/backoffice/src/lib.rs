//! Example: the back office of a small business (an UMKM), the kind of app
//! Filament's demo shows, built from Renox's parts:
//!
//! - a dashboard: figures with their change, revenue by day, overdue
//!   invoices and products running low (`renox::chart`, the kit's `stats`);
//! - data grids for customers, products, invoices, the stock ledger, staff
//!   and the activity log (`renox::grid`), with in-place edits, bulk
//!   actions, summaries and exports;
//! - invoices with line items (the kit's `repeater`), issued from stock,
//!   printed, and paid in cash or through a Midtrans or Xendit payment page
//!   whose webhook marks them paid;
//! - a stock ledger: every change to a product's stock is a row, written in
//!   the same transaction, and stock never goes below zero;
//! - products imported from a CSV file in the browser, a savepoint per line;
//! - exports of the filtered invoices made in the background, with a
//!   notification (the bell) holding the link when the file is ready;
//! - staff with roles and permissions (admin, cashier, warehouse), added by
//!   an admin (no sign-up), who verify their email before they start;
//! - the activity log (`Audit`), company settings, and sign-in pages in the
//!   company's colours.
//!
//! ```text
//! cargo run -- migrate
//! cargo run -- db:seed    # data, and admin@example.com / password123
//! cargo run
//! ```

pub mod app;

use renox::audit::Audit;
use renox::auth::{Permissions, permissions};
use renox::prelude::*;

pub use app::settings::Settings;

pub fn app() -> App {
    App::new()
        .embed(renox::embedded!())
        .migrations(renox::migrations!())
        // Staff are added by an admin (`/staff`), so no `/register`; they
        // confirm their email before using the back office
        // (`require_verified` in `app::routes`). The bell in the layout
        // holds export links and payments.
        .module(
            Auth::new()
                .without_registration()
                .verify_email()
                .account()
                .notifications(),
        )
        .module(Permissions)
        .module(Audit)
        .module(app::BackOffice)
        // The company's name, address and colours on every page, the
        // sign-in pages too (`renox/auth/layout.html`).
        .share("company", |ctx| async move {
            Settings::load(&ctx.state.db).await
        })
        .seeder(app::seed::run)
}

/// What each role may do. Viewing needs only a verified login; changing
/// needs the permission (`require_permission` on the routes, `can(…)` in
/// templates).
pub const ROLES: &[(&str, &[&str])] = &[
    (
        "admin",
        &[
            CUSTOMERS, INVOICES, PRODUCTS, STOCK, EXPORTS, STAFF, SETTINGS, ACTIVITY,
        ],
    ),
    ("cashier", &[CUSTOMERS, INVOICES, EXPORTS]),
    ("warehouse", &[PRODUCTS, STOCK]),
];

/// Add and edit customers.
pub const CUSTOMERS: &str = "customers.manage";
/// Write, issue, void invoices and take payments.
pub const INVOICES: &str = "invoices.manage";
/// Add, edit and import products.
pub const PRODUCTS: &str = "products.manage";
/// Receive and count stock.
pub const STOCK: &str = "stock.adjust";
/// Export invoices.
pub const EXPORTS: &str = "reports.export";
/// Add staff and change their roles.
pub const STAFF: &str = "staff.manage";
/// Change the company settings.
pub const SETTINGS: &str = "settings.manage";
/// Read the activity log.
pub const ACTIVITY: &str = "activity.view";

/// Creates the roles (on every seed and for tests): `define_role` sets a
/// role's permissions to exactly these.
pub async fn define_roles(db: &Db) -> Result {
    for (role, grants) in ROLES {
        permissions::define_role(db, role, grants).await?;
    }
    Ok(())
}
