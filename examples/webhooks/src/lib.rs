//! Example: payment gateway webhooks. Orders are marked paid when Midtrans,
//! Xendit or Stripe calls back. Each call is verified with that provider's
//! signature, stored once per event, and processed by a queue worker.
//!
//! Run it with the secrets in `.env` (`MIDTRANS_SERVER_KEY`,
//! `XENDIT_CALLBACK_TOKEN`, `STRIPE_WEBHOOK_SECRET`): `cargo run -- migrate`,
//! `cargo run -- db:seed`, `cargo run`. Point the provider's dashboard at
//! `https://<your host>/webhooks/<provider>`; while developing, expose your
//! machine with a tunnel such as `cloudflared tunnel --url http://localhost:3000`.

use renox::prelude::*;

mod app;

pub use app::payments::Order;

pub fn app() -> App {
    App::new()
        .embed(renox::embedded!())
        .migrations(renox::migrations!())
        .module(app::payments::Payments)
        .seeder(|state| async move {
            let db = state.db;
            // Seeding twice is harmless: a seeded database stays as it is.
            if Order::query().exists(&db).await? {
                return Ok(());
            }
            for (code, amount) in [("INV-1", 4_999), ("INV-2", 2_500), ("INV-3", 999)] {
                Order::create(&db, Order::new(code, amount)).await?;
            }
            Ok(())
        })
}
