# Renox

A batteries-included web framework for Rust, inspired by Laravel.

**Stack:** Axum + HTMX + Alpine.js + SQLite (or PostgreSQL)

> ⚠️ Early development. Renox is not ready for real applications yet. See [ROADMAP.md](ROADMAP.md).

## Quick start

```bash
cargo install --git https://github.com/arif-rachim/renox renox-cli   # installs `rnx`
rnx new my-app                    # or: rnx new my-app --database postgres
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

Mail uses MiniJinja templates with a text version, SMTP in production and a preview page at
`/_renox/mail` while developing; notifications go by mail and/or to the database:

```rust
let mail = state.mail_view(&user.email, "Your receipt", "mail/receipt", context! { order })?;
state.queue_mail(mail).await?;
state.notify(&user, &OrderShipped { order_id }).await?;
```

Caching, rate limits, maintenance mode and a health check are built in too:

```rust
let menu = state.cache.remember("menu", Duration::from_secs(600), || load_menu(&state.db)).await?;
Routes::new().get("/search", search).throttle(60, Duration::from_secs(60));
```

```bash
my-app down --secret letmein   # 503 for everyone else; visit /letmein to get in
my-app up
curl localhost:3000/health     # {"status":"ok","database":"ok","queue":{...},"maintenance":false}
```

File uploads are ordinary form fields, checked by content and stored locally or on S3/R2:

```rust
#[derive(Deserialize)]
struct ProductForm { name: String, photo: Option<Upload> }

// in `rules`: v.field("photo", &self.photo).image().max(2048);   // KB
let key = photo.store_public(&state.storage, "products").await?;  // <img src="{{ storage_url(key) }}">
```

Texts can be translated per visitor from `resources/lang/{en,id,…}.json`, including Renox's own
validation messages and auth pages:

```html
<h1>{{ t('products.title') }}</h1>  <p>{{ t('products.count', count=total) }}</p>
```

Apps are tested like Laravel apps, in memory:

```rust
use renox::testing::TestApp;

#[renox::test]
async fn creating_a_product() {
    let app = TestApp::new(my_app::app()).await;       // in-memory DB, migrated; fake mail and queue
    app.acting_as(&user)
        .post("/products", &[("name", "Kopi"), ("price", "18000")])   // CSRF handled for you
        .await
        .assert_redirect("/products");
    app.assert_database_has("products", &[("name", &"Kopi")]).await;
}
```

Deploy one file: views, translations and public files are compiled into the release binary.

```bash
rnx build          # dist/my-app (plus a .env on the server)
rnx make:deploy    # Dockerfile, systemd unit, Litestream backups, deploy/README.md
```

The `Auth` module also handles password reset and email verification by email (`MAIL_MAILER=log`
prints the links while developing), and API tokens for mobile apps and integrations:

```rust
let token = user.create_token(&db, "mobile", None).await?;  // send token.plain as `Authorization: Bearer ...`
```

Generators and tools, like artisan:

```bash
rnx make:module products          # routes, a view, registered in main.rs
rnx make:model Product -m         # model + create_product_table migration
rnx make:job SendReceipt --module products
rnx migrate                       # also migrate:rollback, migrate:fresh --seed, db:seed
rnx route:list                    # every route with its name, module and guards
rnx db:shell                      # SQL prompt on the app's database
```

While `rnx serve` runs, the browser reloads by itself when a view, lang or public file changes and
after each rebuild.

SQLite is the default and suits an app on one server. PostgreSQL (the `postgres` feature) is there
for apps that outgrow it, with the same models, queries and commands: see
[docs/postgresql.md](docs/postgresql.md).

**Patterns at a glance:** [CHEATSHEET.md](CHEATSHEET.md) (every Rust snippet is compiled in CI).
Coding agents: start from [llms.txt](llms.txt); apps made by `rnx new` include an `AGENTS.md`.

See [`examples/crud`](examples/crud) for a complete CRUD module (policy, soft deletes, pagination)
and [`examples/hello`](examples/hello) for a guestbook using SQLite, validation, login and
registration, sessions, CSRF, flash messages, pagination and HTMX fragments.

## Planned

- Routing, middleware, sessions, CSRF, flash messages
- SQLite or PostgreSQL with migrations, seeders, factories and a lightweight model layer
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
