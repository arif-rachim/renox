# Renox

A batteries-included web framework for Rust, inspired by Laravel.

**Stack:** Axum + HTMX + Alpine.js + SQLite

> ⚠️ Early development. Renox is not ready for real applications yet. See [ROADMAP.md](ROADMAP.md).

## Quick start

```bash
cargo install --git https://github.com/arif-rachim/renox renox-cli
renox new my-app
cd my-app
renox serve
```

Open http://127.0.0.1:3000. Templates in `resources/views` reload on refresh; Rust changes rebuild
and restart the app.

```rust
use renox::prelude::*;

struct Products;

impl Module for Products {
    fn name(&self) -> &'static str { "products" }

    fn routes(&self) -> Routes {
        Routes::new()
            .get("/products", index).name("products.index")
            .post("/products", store).name("products.store")
    }
}

async fn index() -> View {
    view("products/index.html", context! { products => ["Kopi", "Teh"] }).fragment("list")
}

async fn store(session: Session, back: Back) -> Result<Back> {
    session.flash("status", "Saved!")?;
    Ok(back)
}

fn main() -> renox::Result {
    App::new().module(Products).run()
}
```

See [`examples/hello`](examples/hello) for a guestbook using sessions, CSRF, flash messages and
HTMX fragments.

## Planned

- Routing, middleware, sessions, CSRF, flash messages
- SQLite with migrations, seeders, factories and a lightweight model layer
- Templates with layouts and components, first-class HTMX and Alpine.js helpers
- Validation, authentication and authorization
- Queues, scheduler, mail, notifications, events
- Cache, file storage, i18n, testing helpers
- `renox` CLI: `new`, `serve`, `make:*`, `migrate`, `queue:work`, ...

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
