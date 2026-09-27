//! Example: every kind of form field, from the browser to the database and
//! back into the edit form, on SQLite and PostgreSQL. See the table in
//! docs/types.md in the Renox repository.

use renox::prelude::*;

mod app;

pub use app::products::{Product, Size};

pub fn app() -> App {
    App::new()
        .embed(renox::embedded!())
        .migrations(renox::migrations!())
        .module(app::products::Products)
}
