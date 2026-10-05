//! Example: an admin panel generated from the models (`renox-admin`, #148),
//! the back office of a small shop with no page written by hand:
//!
//! - three resources, each declared once in `src/resources/`: its grid
//!   columns, its form fields, its filters and actions, and its model's
//!   `Policy`;
//! - lists with search, column filters, named filters as tabs, sorting,
//!   bulk and row actions, and CSV, Excel (with `--features renox/xlsx`)
//!   and print exports;
//! - create and edit forms checked by the forms' `Validate` rules (errors
//!   stay in the form), view pages, deletes that go to the trash for
//!   products (soft deletes) and come back from it;
//! - two roles (the `Permissions` module): an admin may do everything, an
//!   editor may add and change products but delete nothing and only look
//!   at customers;
//! - one page of the panel replaced by the app: the products list's
//!   "Stock" cells (`resources/views/renox-admin/products/cells.html`).
//!
//! Made with `rnx new admin`, then `rnx make:model Category`,
//! `rnx make:model Product`, `rnx make:model Customer` and their
//! migrations; the resources are written by hand (no generator yet).
//!
//! ```text
//! cargo run -- migrate
//! cargo run -- db:seed    # admin@ and editor@example.com / password123
//! cargo run               # http://127.0.0.1:3000/admin
//! ```

pub mod resources;
pub mod seed;

use renox::auth::{Permissions, permissions};
use renox::prelude::*;
use renox_admin::Admin;

pub fn app() -> App {
    App::new()
        .embed(renox::embedded!())
        .migrations(renox::migrations!())
        // Staff are seeded (or added by an admin): no sign-up page.
        .module(Auth::new().without_registration().redirect_to("/admin"))
        .module(Permissions)
        .module(panel())
        .module(Home)
        .seeder(seed::run)
}

/// The panel: who may open it, and its resources in the order of its
/// navigation.
pub fn panel() -> Admin {
    Admin::new()
        .title("Corner Shop")
        .authorize(|user| ROLES.iter().any(|(role, _)| user.has_role(role)))
        .resource(resources::ProductResource)
        .resource(resources::CategoryResource)
        .resource(resources::CustomerResource)
}

/// The roles, and the permissions each holds. The models' policies ask
/// for the permissions (`user.has_permission`).
pub const ROLES: &[(&str, &[&str])] = &[
    ("admin", &[CATALOG, DELETE, CUSTOMERS]),
    ("editor", &[CATALOG]),
];

/// Add and change products and categories.
pub const CATALOG: &str = "catalog.manage";
/// Delete, restore and purge records.
pub const DELETE: &str = "records.delete";
/// Add and change customers.
pub const CUSTOMERS: &str = "customers.manage";

/// Creates the roles (on every seed and for tests).
pub async fn define_roles(db: &Db) -> Result {
    for (role, grants) in ROLES {
        permissions::define_role(db, role, grants).await?;
    }
    Ok(())
}

/// `/` leads to the panel.
struct Home;

impl Module for Home {
    fn name(&self) -> &'static str {
        "home"
    }

    fn routes(&self) -> Routes {
        Routes::new().redirect("/", "/admin").name("home")
    }
}
