# Renox

A batteries-included web framework for Rust, inspired by Laravel.

**Stack:** Axum + HTMX + Alpine.js + SQLite

> ⚠️ Early development. Renox is not ready for real applications yet. See [ROADMAP.md](ROADMAP.md).

## Quick start

```bash
cargo install --git https://github.com/arif-rachim/renox renox-cli   # installs `rnx`
rnx new my-app
cd my-app
rnx serve
```

Open http://127.0.0.1:3000. Templates in `resources/views` reload on refresh; Rust changes rebuild
and restart the app.

```rust
use renox::prelude::*;
use serde::Serialize;

#[derive(Model, Serialize, Default)]
#[model(table = "products", soft_deletes)]
struct Product {
    id: i64,
    name: String,
    price: i64,
    created_at: Option<DateTime>,
    updated_at: Option<DateTime>,
    deleted_at: Option<DateTime>,
}

struct Products;

impl Module for Products {
    fn name(&self) -> &'static str { "products" }

    fn routes(&self) -> Routes {
        Routes::new()
            .get("/products", index).name("products.index")
            .post("/products", store).name("products.store")
    }
}

async fn index(State(db): State<Db>, Page(page): Page) -> Result<View> {
    let products = Product::query().latest().paginate(&db, page, 20).await?;
    Ok(view("products/index.html", context! { products }).fragment("list"))
}

async fn store(State(db): State<Db>, session: Session, back: Back) -> Result<Back> {
    Product::create(&db, Product { name: "Kopi".into(), price: 18_000, ..Default::default() }).await?;
    session.flash("status", "Saved!")?;
    Ok(back)
}

fn main() -> renox::Result {
    App::new()
        .migrations(renox::migrations!())
        .module(Products)
        .run()
}
```

Migrations live in `migrations/` and run with `rnx migrate`:

```bash
rnx make:migration create_products_table
rnx migrate
rnx migrate:rollback
rnx migrate:fresh --seed
```

See [`examples/hello`](examples/hello) for a guestbook using SQLite, sessions, CSRF, flash messages,
pagination and HTMX fragments.

## Planned

- Routing, middleware, sessions, CSRF, flash messages
- SQLite with migrations, seeders, factories and a lightweight model layer
- Templates with layouts and components, first-class HTMX and Alpine.js helpers
- Validation, authentication and authorization
- Queues, scheduler, mail, notifications, events
- Cache, file storage, i18n, testing helpers
- `rnx` CLI: `new`, `serve`, `make:*`, `migrate`, `queue:work`, ...

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
