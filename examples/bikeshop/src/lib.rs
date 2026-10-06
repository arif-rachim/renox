//! Example: a bike shop with three stores, Renox's flagship example (#231).
//! Customers buy bikes and gear, rent a bike by the hour or the day, and
//! have their bikes serviced, once or on a plan; the three stores share
//! staff, bikes and goods, and settle with each other monthly.
//!
//! Every page explains itself: an "About this page" panel says what the
//! page is for, who uses it, which Renox features it uses and why, and
//! links to the guide and the source files (`src/explain.rs`). `/about/pages`
//! lists every page by feature and by role.
//!
//! How it was made: `rnx new bikeshop` wrote the app (lib + bin, the
//! `home` module, the layout, `.env.example`), then it moved into the
//! workspace like the other examples (`renox.workspace = true` in
//! `Cargo.toml`). The areas in `src/app/` name the `rnx make:*` commands
//! that made their parts.
//!
//! Run it from this directory:
//!
//! ```text
//! cargo run -- migrate
//! cargo run          # http://127.0.0.1:3000
//! ```

pub mod app;
pub mod explain;
pub mod seed;

use renox::prelude::*;

/// The application: its modules, migrations and seeders. `main.rs` runs it,
/// and tests boot it with `renox::testing::TestApp`.
pub fn app() -> App {
    App::new()
        .embed(renox::embedded!())
        .migrations(renox::migrations!())
        // A visitor's browser language wins when the shop has it (en, es),
        // until they pick one in the language menu.
        .detect_locale()
        // The "About this page" panel: `about_page(request.route, request.path)` in the layouts,
        // hidden with BIKESHOP_EXPLAIN=false.
        .templates(explain::register)
        .share(
            "explain_panels",
            |ctx: renox::view::ViewContext| async move { Ok(explain::enabled(&ctx.state.config)) },
        )
        .module(renox::auth::Auth::new().account()) // login, register, /account
        // Roles given per store, with dates (#244); the catalogue is src/app/access.
        .module(renox::auth::permissions::Permissions)
        // `db:seed`: a small shop in seconds (`demo:seed --size large` for Pagila's volume).
        .seeder(seed::run)
        .typed_command::<seed::DemoSeed>()
        // Service plans billed as subscriptions (#237): renox-billing's
        // plans, gateways (Stripe, Xendit, the demo) and webhooks.
        .module(app::plans::billing())
        // --- Areas (alphabetical; add new ones in order) ---
        .module(app::about::About)
        .module(app::access::Access)
        .module(app::accounts::Accounts)
        .module(app::api::Api)
        .module(app::catalog::Catalog)
        .module(app::home::Home)
        .module(app::multistore::Multistore)
        .module(app::plans::Plans)
        .module(app::rentals::Rentals)
        .module(app::reports::Reports)
        .module(app::sales::Sales)
        .module(app::staff::Staff)
        .module(app::stock::Stock)
        .module(app::workshop::Workshop)
    // --- end of areas ---
}
