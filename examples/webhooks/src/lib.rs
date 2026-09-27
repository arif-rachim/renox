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
        .seeder(|db| async move {
            for (code, amount) in [("INV-1", 150_000), ("INV-2", 75_000), ("INV-3", 20_000)] {
                Order::create(&db, Order::new(code, amount)).await?;
            }
            Ok(())
        })
}
