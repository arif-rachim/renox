//! Example: one app on PostgreSQL *and* SQLite. The code is the same; only
//! `DATABASE_URL` and a few migration files differ.
//!
//! ```text
//! createdb tasks && createdb tasks_test
//! DATABASE_URL=postgres://postgres:postgres@localhost:5432/tasks cargo run -- migrate
//! DATABASE_URL=postgres://postgres:postgres@localhost:5432/tasks cargo run
//! TEST_DATABASE_URL=postgres://postgres:postgres@localhost:5432/tasks_test cargo test
//! ```
//!
//! Without those variables it runs on SQLite, tests included.
//! See docs/postgresql.md in the Renox repository.

use renox::prelude::*;

mod app;

pub use app::tasks::Task;

pub fn app() -> App {
    App::new()
        .embed(renox::embedded!())
        .migrations(renox::migrations!())
        .module(app::tasks::Tasks)
}
