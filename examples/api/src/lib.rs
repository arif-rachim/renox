//! Example: a JSON API for a mobile app or a separate front end.
//!
//! - `POST /api/tokens` trades an email and password for an API token.
//! - Every other `/api` route needs `Authorization: Bearer <token>`. It
//!   needs no CSRF token: API tokens can't be sent by another site.
//! - Tokens carry abilities (`products:read`, `products:write`) and expire
//!   after 30 days; a token without the route's ability gets 403. Expired
//!   tokens are pruned nightly.
//! - The product list is cursor-paginated (`?cursor=<next_cursor>`).
//! - Bad input gets `422 {"message", "errors"}`; guests get 401.
//! - Browsers on `https://app.example.com` may call it (CORS), and each
//!   caller gets at most 60 requests a minute.
//!
//! Run it: `cargo run -- migrate`, `cargo run -- db:seed`, `cargo run`, then
//! `curl -X POST localhost:3000/api/tokens -H 'content-type: application/json'
//!  -d '{"email":"demo@example.com","password":"password123","device":"curl"}'`.

use renox::prelude::*;

mod app;

pub use app::products::Product;

pub fn app() -> App {
    App::new()
        .embed(renox::embedded!())
        .migrations(renox::migrations!())
        // Users and the personal_access_tokens table; no pages needed here.
        .module(Auth::new().without_registration())
        .module(app::products::Products)
        // The Auth module adds the `tokens:prune` command; run the same
        // cleanup every night so expired tokens don't pile up.
        .schedule(|s| {
            s.daily_at("03:00", "prune-expired-tokens", |state| async move {
                let day = std::time::Duration::from_secs(24 * 60 * 60);
                renox::auth::prune_expired_tokens(&state.db, day).await?;
                Ok(())
            });
        })
        .seeder(|db| async move {
            User::register(&db, "Demo", "demo@example.com", "password123").await?;
            for (name, price) in [("Kopi", 18_000), ("Teh", 9_000)] {
                let product = Product {
                    name: name.into(),
                    price,
                    ..Default::default()
                };
                Product::create(&db, product).await?;
            }
            Ok(())
        })
}
