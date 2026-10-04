//! Example: background work. Placing an order emits `OrderPlaced`. Its
//! listener notifies the shop's admins, by mail and in the database. Paying
//! queues a chain (charge the card, send the receipt, tell the warehouse)
//! on a `high` priority queue, with an encrypted payload, job middleware
//! and a `failed` hook. Staff send unique payment reminders and a batch of
//! monthly statements with a live progress bar. Scheduled tasks mail the
//! day's sales on weekday evenings and the week's on Monday mornings
//! (Jakarta time), guarded by a cache lock and with an alert when they fail.
//! Errors that need a person also go to a chat webhook (`App::report`).
//! Logged-in staff see order changes live: the jobs broadcast them to the
//! open pages (`state.broadcast`), and a failed charge arrives as a toast
//! whose "Reopen" button sends a request (`ToastAction::post`).
//!
//! All of it runs inside `cargo run` (queue workers and the scheduler are
//! part of `serve`). With `MAIL_MAILER=log` the mails go to the log, and
//! while debugging they're listed at /_renox/mail. The queue dashboard is
//! at /_renox/queue for the admin.

use renox::prelude::*;
use renox::report::ErrorReport;

mod app;

pub use app::orders::{
    ChargePayment, NotifyWarehouse, Order, OrderPlaced, OrderStatus, RemindUnpaid, SendReceipt,
    SendStatement, StatementsSent, daily_sales, weekly_sales,
};

pub fn app() -> App {
    App::new()
        .embed(renox::embedded!())
        .migrations(renox::migrations!())
        // Staff only: every user is an admin who gets the order mails, so
        // nobody signs themselves up (`db:seed` makes the admin).
        // `.notifications()`: the bell in the bar, and the stream that
        // carries `state.broadcast(…)` to the staff's open pages.
        .module(Auth::new().without_registration().notifications())
        .module(app::orders::Orders)
        .module(renox::queue::Dashboard)
        .gate(renox::queue::DASHBOARD_GATE, |user| {
            user.email == "admin@example.com"
        })
        // Every error a person should see (a 500, a job failed for good, a
        // failed scheduled task) also goes to the team's chat.
        .report(post_to_chat)
        // Sales reports' own mailer: REPORTS_MAILER=smtp, REPORTS_HOST=…
        // in .env; unset, it is the app's MAIL_MAILER (`log`, `memory` in tests).
        .mailer("reports", |config| {
            renox::mail::MailConfig::from_env(config, "REPORTS")
        })
        .seeder(|state| async move {
            let db = state.db;
            // Seeding twice is harmless: a seeded database stays as it is.
            if User::find_by_email(&db, "admin@example.com")
                .await?
                .is_some()
            {
                return Ok(());
            }
            User::register(&db, "Admin", "admin@example.com", "password123").await?;
            Ok(())
        })
}

/// Posts an error report to a chat webhook (Slack, Discord, Google Chat…)
/// when `ERROR_WEBHOOK_URL` is set. Errors are logged either way.
pub async fn post_to_chat(report: ErrorReport, state: AppState) {
    let Some(url) = state.config.var("ERROR_WEBHOOK_URL") else {
        return;
    };
    let text = format!(
        "[{}] {}{}",
        report.environment,
        report
            .source
            .as_deref()
            .map(|source| format!("{source}: "))
            .unwrap_or_default(),
        report.message
    );
    let sent = state
        .http
        .post(&url)
        .json(&json!({ "text": text }))
        .send()
        .await;
    if let Err(err) = sent {
        tracing::warn!("could not post the error report: {err:?}");
    }
}
