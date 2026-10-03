//! Example: one app on PostgreSQL *and* SQLite. The code is the same; only
//! `DATABASE_URL` and one migration file differ.
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
        // `cargo run -- db:seed`: a few tasks, one of them overdue.
        .seeder(|db| async move {
            if Task::query().exists(&db).await? {
                return Ok(()); // seeded already
            }
            let today = renox::db::now().date_naive();
            for (title, due_in_days, done) in [
                ("Write the release notes", Some(3), false),
                ("Pay the hosting bill", Some(-2), false),
                ("Set up PostgreSQL", None, true),
            ] {
                let task = Task {
                    title: title.into(),
                    done,
                    due_on: due_in_days.map(|d| today + renox::chrono::Duration::days(d)),
                    ..Default::default()
                };
                Task::create(&db, task).await?;
            }
            Ok(())
        })
}
