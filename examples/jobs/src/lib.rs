//! Example: background work. Placing an order emits `OrderPlaced`. Its
//! listener notifies the shop's admins, by mail and in the database. Paying
//! queues a chain (charge the card, send the receipt, tell the warehouse)
//! on a `high` priority queue, with an encrypted payload, job middleware
//! and a `failed` hook. Staff send unique payment reminders and a batch of
//! monthly statements with a live progress bar. Scheduled tasks mail the
//! day's sales on weekday evenings and the week's on Monday mornings
//! (Jakarta time), guarded by a cache lock and with an alert when they fail.
//!
//! All of it runs inside `cargo run` (queue workers and the scheduler are
//! part of `serve`). With `MAIL_MAILER=log` the mails go to the log, and
//! while debugging they're listed at /_renox/mail. The queue dashboard is
//! at /_renox/queue for the admin.

use renox::prelude::*;

mod app;

pub use app::orders::{
    ChargePayment, NotifyWarehouse, Order, OrderStatus, RemindUnpaid, SendReceipt, SendStatement,
    StatementsSent, daily_sales, weekly_sales,
};

pub fn app() -> App {
    App::new()
        .embed(renox::embedded!())
        .migrations(renox::migrations!())
        .module(Auth::new())
        .module(app::orders::Orders)
        .module(renox::queue::Dashboard)
        .gate(renox::queue::DASHBOARD_GATE, |user| {
            user.email == "admin@example.com"
        })
        .seeder(|db| async move {
            User::register(&db, "Admin", "admin@example.com", "password123").await?;
            Ok(())
        })
}
