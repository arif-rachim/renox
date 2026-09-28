//! Example: background work. Placing an order emits `OrderPlaced`. Its
//! listener queues the customer's receipt email and notifies the shop's
//! admins, by mail and in the database. Scheduled tasks mail the day's
//! sales on weekday evenings and the week's on Monday mornings (Jakarta
//! time), guarded by a cache lock and with an alert when they fail.
//!
//! All of it runs inside `cargo run` (queue workers and the scheduler are
//! part of `serve`). With `MAIL_MAILER=log` the mails go to the log, and
//! while debugging they're listed at /_renox/mail.

use renox::prelude::*;

mod app;

pub use app::orders::{Order, daily_sales, weekly_sales};

pub fn app() -> App {
    App::new()
        .embed(renox::embedded!())
        .migrations(renox::migrations!())
        .module(Auth::new())
        .module(app::orders::Orders)
        .seeder(|db| async move {
            User::register(&db, "Admin", "admin@example.com", "password123").await?;
            Ok(())
        })
}
