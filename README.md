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
use serde::{Deserialize, Serialize};

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

#[derive(Deserialize)]
struct ProductForm {
    name: String,
    price: i64,
}

impl Validate for ProductForm {
    fn rules(&self, v: &mut Validator) {
        v.field("name", &self.name).required().max(100).unique("products", "name");
        v.field("price", &self.price).min(1_000);
    }
}

// Invalid input never reaches the handler: regular posts go back with errors and old input,
// HTMX posts get a 422 that the bundled script shows next to the fields.
async fn store(State(db): State<Db>, session: Session, back: Back, Valid(form): Valid<ProductForm>) -> Result<Back> {
    Product::create(&db, Product { name: form.name, price: form.price, ..Default::default() }).await?;
    session.flash("status", "Saved!")?;
    Ok(back)
}

fn main() -> renox::Result {
    App::new()
        .migrations(renox::migrations!())
        .module(Auth::new())            // /login, /register, /logout and the users table
        .module(Products)
        .run()
}
```

Protect routes and check permissions:

```rust
Routes::new()
    .get("/products/{id}/edit", edit)
    .require_auth();                    // guests go to /login and come back afterwards

async fn edit(auth: AuthUser, State(db): State<Db>, Path(id): Path<i64>) -> Result<View> {
    let product = Product::find_or_404(&db, id).await?;
    auth.authorize("update", &product)?; // 403 unless `impl Policy for Product` allows it
    Ok(view("products/edit.html", context! { product }))
}
```

Background jobs, events and scheduled tasks run inside the same binary:

```rust
#[derive(Serialize, Deserialize)]
struct SendReceipt { order_id: i64 }

impl Job for SendReceipt {
    const NAME: &'static str = "send-receipt";
    async fn handle(self, ctx: JobContext) -> Result { /* ... */ Ok(()) }
}

App::new()
    .job::<SendReceipt>()
    .listen(|e: OrderPlaced, state| async move { state.dispatch(SendReceipt { order_id: e.id }).await.map(|_| ()) })
    .schedule(|s| { s.daily_at("02:00", "close-day", close_day); })
```

The `Auth` module also handles password reset and email verification by email (`MAIL_MAILER=log`
prints the links while developing), and API tokens for mobile apps and integrations:

```rust
let token = user.create_token(&db, "mobile", None).await?;  // send token.plain as `Authorization: Bearer ...`
```

Migrations live in `migrations/` and run with `rnx migrate`:

```bash
rnx make:migration create_products_table
rnx migrate
rnx migrate:rollback
rnx migrate:fresh --seed
```

See [`examples/hello`](examples/hello) for a guestbook using SQLite, validation, login and
registration, sessions, CSRF, flash messages, pagination and HTMX fragments.

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
