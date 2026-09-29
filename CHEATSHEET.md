# Renox cheat-sheet

This page shows the most common patterns, one short example each, written the recommended way.
Every Rust block below is compiled by `cargo test --doc -p renox`, so none of them can drift out of
date. For whole apps, see [`examples/`](examples) (the list is in [llms.txt](llms.txt)).

## Commands

```bash
rnx new shop                         # or: --database postgres, --tailwind (Tailwind CSS, no Node)
rnx key:generate                     # APP_KEY into .env (made from .env.example if missing)
rnx serve                            # run, rebuild and reload on changes
rnx make:module products             # routes + view, registered in src/lib.rs
rnx make:module products --resource --fields "name:string price:money notes:text active:bool due_on:date"
                                     # model, migration, factory, form, 7 handlers, UI-kit views, tests
rnx make:model Product --module products --migration
rnx make:migration add_sku_to_products
rnx make:policy Product --module products
rnx make:job SendReceipt --module products
rnx make:command products:import --module products  # typed (clap); `rnx products:import --help`
rnx make:mail order_shipped
rnx make:factory Product --module products  # also make:seeder DemoData, make:test checkout
rnx make:notification OrderShipped --module orders  # also make:event, make:rule (--module)
rnx make:middleware StampRequests    # on every route (App::layer)
rnx make:component price_tag         # --ui copies the UI kit into the app
rnx migrate                          # migrate:status, migrate:fresh --seed, db:seed
rnx migrate:rollback --step 2        # the last 2 batches (default 1)
rnx route:list                       # db:shell, schedule:list, schedule:run NAME, cache:prune, session:prune
rnx queue:work --queue mail --workers 2  # --once: run what is queued, then stop
rnx queue:failed                     # queue:retry <id|all>, queue:flush (deletes them)
rnx schedule:work                    # tasks in their own process (SCHEDULER=false for serve)
rnx webhook:failed                   # webhook:retry <id>
rnx down --secret s3cret --retry 60  # 503 for everyone but visitors of /s3cret; `rnx up` ends it
rnx build                            # one release binary in dist/; rnx make:deploy
rnx tailwind --watch                 # resources/css/app.css -> public/css/app.css (serve/build do it)
```

`serve` already runs the queue workers and the scheduler. `queue:work` and `schedule:work` are for
running them in separate processes.

## App, module, routes

```rust
use renox::prelude::*;
use renox::clap;
use renox::command::AppCommand;
use std::time::Duration;

pub fn app() -> App {
    App::new()
        .embed(renox::embedded!())         // views, lang files, public/ inside the binary
        .migrations(renox::migrations!())  // migrations/*.sql
        .module(Auth::new())               // login, register, password reset
        .module(Products)
}

pub struct Products;

impl Module for Products {
    fn name(&self) -> &'static str {
        "products"
    }

    fn routes(&self) -> Routes {
        let public = Routes::new()
            .get("/products", index).name("products.index")
            .get("/products/{id}", show).name("products.show"); // axum 0.8: {id}, not :id; wildcard {*rest}
        let members = Routes::new()
            .post("/products", store)
            .name("products.store")
            .require_auth(); // covers the routes added before it
        let checkout = Routes::new()
            .post("/checkout", store).name("checkout")
            .require_verified()                     // logged in, with a verified email
            .throttle(10, Duration::from_secs(60)); // 10 a minute per user or IP, then 429
        let guests = Routes::new()
            .get("/welcome", index).name("welcome")
            .guest_only(); // logged-in users go to the `home` route
        public.merge(members).merge(checkout).merge(guests).group(
            "/admin",   // path prefix: starts with `/`, never ends with one
            "admin.",   // name prefix
            Routes::new()
                .get("/", index).name("dashboard")        // GET /admin, `admin.dashboard`
                .get("/products", index).name("products") // GET /admin/products, `admin.products`
                .require_auth(),                          // only the group's routes
        )
    }

    fn register(&self, app: &mut Registry) {
        // `my-app products:import file.csv --dry-run` (or `rnx products:import …`)
        app.typed_command::<ImportProducts>();
        // Untyped: app.command("name", "about", |state, args: renox::command::Args| async { … })
    }
}

async fn index() -> View {
    view("products/index.html", context! { title => "Products" })
}

async fn show(Path(id): Path<i64>) -> Result<View> {
    abort_if(id > 1_000_000, StatusCode::NOT_FOUND, "No such product.")?; // a status with a message
    Ok(view("products/show.html", context! { id }))
}

async fn store() -> Redirect {
    Redirect::to("/products")
}

/// Import products from a CSV file.     (the line in `my-app help`)
#[derive(clap::Parser)]
#[command(name = "products:import")]
struct ImportProducts {
    /// The CSV file; asked for when it's left out.
    file: Option<String>,
    #[arg(long)]
    dry_run: bool,
}

impl AppCommand for ImportProducts {
    async fn run(self, state: AppState) -> Result {
        let file = match self.file {
            Some(file) => file,
            None => renox::prompt::ask("CSV file").await?, // also ask_or, secret, confirm, choice
        };
        if !self.dry_run && !renox::prompt::confirm(&format!("Import {file}?"), true).await? {
            return Ok(());
        }
        let _ = state;
        Ok(())
    }
}

async fn download(order_paid: bool) -> Result<&'static str> {
    // Any status with a message visitors see (page or JSON `message`).
    abort_unless(order_paid, StatusCode::PAYMENT_REQUIRED, "Pay for the order first.")?;
    // or: return Err(abort(StatusCode::GONE, "This offer ended."));
    Ok("the file")
}
```

Wrong arguments print what's wrong and the usage; `--help` lists them. In tests:
`app.kernel().call("products:import", ["a.csv", "--dry-run"]).await`, and
`renox::prompt::answering(["a.csv", "yes"], app.kernel().call("products:import", [""; 0])).await`
answers the questions.

`src/main.rs` is only `fn main() -> renox::Result { shop::app().run() }`. The app lives in the
library, so tests can boot it.

When a handler won't compile as a route ("the trait `Handler<_, _>` is not implemented", or
"`Send` is not general enough"), look for a closure or an iterator that borrows local variables
and is still alive across an `.await`. Collect into a `Vec` first, then await. Also,
`.require_auth()` covers only the routes added before it: a route added after it is public.

## Views (MiniJinja)

```html
{% extends "layouts/app.html" %}
{% block content %}
<h1>{{ title }}</h1>
<a href="{{ route('products.edit', product.id) }}">Edit</a>   {# named route with parameters #}
<a href="{{ route('products.index', sort='price', q=q) }}">…</a> {# named arguments: ?q=…&sort=price (none is left out) #}
<img src="{{ asset('logo.png') }}">                           {# /logo.png?v=hash: cached a year, new URL on change #}
<p>{{ t('shop.welcome', name=auth.user.name) if auth.check }}</p>
{% if flash.status %}<p class="flash">{{ flash.status }}</p>{% endif %}
{% if can('admin') %}<a href="/admin">Admin</a>{% endif %}    {# gate #}
<p>{{ product.price | number }}</p>                            {# 75.000 (id) / 75,000 (en); number(2) #}
<p>{{ order.created_at | date('%d/%m/%Y %H:%M') }}</p>        {# in APP_TIMEZONE; default %Y-%m-%d #}
<span>{{ cart_count }}</span>                                  {# from App::share #}
{% endblock %}
```

With `APP_DEBUG` on, printing a variable that doesn't exist (`{{ prodcut.name }}`) is an error
page showing the request, the error and the template line; `{% if x %}` on a missing one is fine,
and so is `{{ flash.anything }}`.

Every view also gets: `request.path`, `request.query`, `request.htmx`, `app.name`, `app.env`,
`app.debug`, `app.url`, `app.locale`, `auth.check`, `auth.user`, `flash`, `errors`, `csrf_token`,
and the functions `old()`, `error()`, `csrf_field()`, `method_field()`, `route()`, `asset()`,
`storage_url()`, `t()`, `can()`, `page_url(n)`, `renox_head()`, `csp_nonce()`, `seo()`,
`renox_ui()` (the UI kit), `toasts()`, `once(key)`, `stack(name)` and, with `{% call %}`,
`push(name)` / `prepend(name)`.

Error pages are the app's own: `rnx new` writes `resources/views/errors/default.html`, which
extends the layout and gets every global above plus `status`, `reason` and `detail`;
`errors/404.html` (any status) wins for one status. Without them, Renox shows its own.

Renox's own pages are templates you can replace: create the same file under `resources/views/`,
e.g. `renox/error.html`, `renox/auth/login.html` (also
`register`, `forgot-password`, `reset-password`, `verify-email`, `layout`),
`renox/mail/layout.html` and `renox/pagination.html`. The `pagination` macro must be imported:
`{% from "renox/pagination.html" import pagination %}`.

```rust
use renox::prelude::*;
use renox::view::ViewContext;

fn view_extras(app: App) -> App {
    app.templates(|env| {
        // Your own filters and functions (MiniJinja's API).
        env.add_filter("rupiah", |n: i64| format!("Rp {}", renox::format_number(n as f64, 0, "id")));
    })
    // In every view, computed per request (cache what doesn't change per request).
    .share("cart_count", |ctx: ViewContext| async move {
        let Some(user) = ctx.user else { return Ok(0) };
        let n: i64 = renox::db::sql("SELECT COUNT(*) FROM cart_items WHERE user_id = ?")
            .bind(user.id)
            .scalar(&ctx.state.db)
            .await?;
        Ok(n)
    })
}
```

## Components and the UI kit (details in docs/ui.md)

```html
{# layout: {{ renox_head() }}{{ renox_ui() }} in <head>, <body class="rx-page">, {{ toasts() }} #}
{% from "renox/ui.html" import card, input, select, checkbox, button, confirm, table, form_errors %}
<form method="post" action="/products" data-live-validate novalidate>{{ csrf_field() }}
  {% call card(title="New product") %}
    {{ form_errors() }}
    {{ input("name", "Name", required=true, hint="Shown on the list") }}
    {{ select("size", "Size", [["s", "Small"], ["m", "Medium"]]) }}
    {{ checkbox("featured", "Featured", switch=true) }}
    {{ button("Create product") }}
  {% endcall %}
</form>
{{ confirm("del-7", "Delete", "/products/7", "Delete “Kopi”?", "It goes to the trash.") }}
{# Your own: rnx make:component price_tag -> components/price_tag.html, a macro that can use
   old(), error(), t(), can(), auth, csrf_field() like the page. rnx make:component --ui copies
   the kit into the app. {% if once('x') %} renders once per page. #}

{# Stacks: the layout has {{ stack('head') }} in <head> and {{ stack('scripts') }} before
   </body>; any page, block or component adds to them, even after the head was rendered: #}
{% call push('scripts', once='chart') %}<script src="{{ asset('chart.js') }}" nonce="{{ csp_nonce() }}"></script>{% endcall %}
{% call prepend('head') %}<meta name="robots" content="noindex">{% endcall %}
```

Tailwind: `rnx new shop --tailwind` (or create `resources/css/app.css` with
`@import "tailwindcss"; @source "../views";`). `rnx serve` rebuilds `public/css/app.css` as views
change, `rnx build` minifies it; link it with `{{ asset('css/app.css') }}`. The kit's `rx-*`
classes keep working next to the utilities.

## Your own shared values and middleware

```rust
use renox::prelude::*;
use renox::Provided;
use renox::axum::extract::Request;
use renox::axum::middleware::{Next, from_fn};

#[derive(Clone)]
struct Payments { api_key: String }

async fn pay(payments: Provided<Payments>) -> String {
    // In jobs, listeners, commands and tasks: state.provided::<Payments>()
    format!("key starts with {}", &payments.api_key[..3])
}

#[derive(Clone)]
struct Plan(&'static str);

async fn stamp(user: Option<AuthUser>, req: Request, next: Next) -> Response {
    renox::context::set(Plan(if user.is_some() { "member" } else { "guest" }));
    let mut res = next.run(req).await; // runs after the session and user are loaded
    res.headers_mut().insert("x-member", user.is_some().to_string().parse().unwrap());
    res
}

// A context value as an argument (500 if missing; `Option<Current<Plan>>` when optional).
async fn plan(renox::context::Current(plan): renox::context::Current<Plan>) -> &'static str {
    plan.0
}

fn wiring(app: App) -> App {
    let _ = plan;
    app.provide(Payments { api_key: "sk_test_123".into() })
        .layer(from_fn(stamp)) // every route of the app's modules
}
```

## Form + validation

```rust
use renox::prelude::*;
use serde::Deserialize;

#[derive(Deserialize)]
struct ProductForm {
    name: String,
    price: i64,
    email: Option<String>,
    sku: String,
    kind: String,
    brand: Option<String>,
    launch: renox::chrono::NaiveDate,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    photos: Vec<Upload>, // <input type="file" name="photos" multiple>
}

impl Validate for ProductForm {
    fn rules(&self, v: &mut Validator) {
        v.field("name", &self.name).required().max(100).unique("products", "name");
        v.field("price", &self.price).min(0);
        v.field("email", &self.email).email();
        v.field("sku", &self.sku).matches(r"^[A-Z]{2}-\d{4}$").none_of(&["XX-0000"]);
        v.field("brand", &self.brand).required_if(self.kind == "branded"); // also required_with, required_unless
        v.field("launch", &self.launch).after(renox::chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap());
        v.field("tags", &self.tags).max(5);
        v.each("tags", &self.tags, |tag| tag.required().max(20)); // errors on tags.0, tags.1, …
        v.each("photos", &self.photos, |photo| photo.image().max(2048));
        // Also: digits(n), digits_between(a, b), date(), before…, one_of, same, different,
        // alpha, alpha_num, alpha_dash, lowercase, uppercase, starts_with(&["08"]), ends_with,
        // uuid, ip, size(n), required_without(&other), prohibited_if(cond),
        // v.distinct("tags", &self.tags), v.nested("lines", &self.lines) for a Vec of structs,
        // .apply(&MyRule) with `validation::Rule`.
    }
}

// Invalid input never reaches the handler. A plain form post goes back with
// errors and old input; an HTMX post gets a 422 and the errors appear next to
// the fields. Every field's errors show at once, even when one doesn't parse.
async fn store(session: Session, Valid(form): Valid<ProductForm>) -> Result<Redirect> {
    let _ = form.name;
    session.flash("status", "Saved.")?;
    Ok(Redirect::to("/products"))
}
```

```html
<form method="post" action="{{ route('products.update', product.id) }}">
  {{ csrf_field() }}{{ method_field('PUT') }}   {# routed as PUT; also PATCH, DELETE #}
  <input name="name" value="{{ old('name', product.name) }}">
  <p class="error" data-error-for="name">{{ error('name') }}</p>
  <button>Save</button>
</form>
```

A form request (Laravel's `FormRequest`): three optional hooks around the rules.

```rust
use renox::prelude::*;
use renox::validation::FormContext;

#[derive(serde::Deserialize)]
struct Invite { email: String }

impl Validate for Invite {
    fn prepare(&mut self) { self.email = self.email.trim().to_lowercase(); } // first
    async fn authorize(&self, form: &FormContext<'_>) -> Result<bool> {    // false -> 403
        Ok(form.user.is_some_and(|u| u.has_role("owner")))
    }
    fn rules(&self, v: &mut Validator) { v.field("email", &self.email).required().email(); }
    async fn after(&self, form: &FormContext<'_>, errors: &mut Errors) -> Result { // rules passed
        if User::find_by_email(&form.state.db, &self.email).await?.is_none() {
            errors.add("email", "Nobody has signed up with this email yet."); // shown like a rule's
        }
        Ok(())
    }
}
```

The same form for create and edit, with the row's id in a hidden input:

```rust
use renox::prelude::*;
use serde::Deserialize;

#[derive(Deserialize)]
struct ProductForm {
    #[serde(default)]
    id: i64, // <input type="hidden" name="id" value="{{ product.id }}">; 0 when creating
    sku: String,
    category_id: i64,
}

impl Validate for ProductForm {
    fn rules(&self, v: &mut Validator) {
        v.field("sku", &self.sku).required().unique("products", "sku").ignore(self.id); // skip this row
        v.field("category_id", &self.category_id).exists("categories", "id").message("Pick a category.");
        // Also: label("SKU code"), url(), between(1, 10), confirmed(&self.password_confirmation),
        // accepted() for a checkbox, rule(self.qty % 6 == 0, "Sold by the half dozen.").
    }
}
```

## Model, migration, queries

```sql
-- migrations/20260101000000_create_products_table.up.sql
-- (PostgreSQL: *.postgres.up.sql with BIGINT GENERATED BY DEFAULT AS IDENTITY, TIMESTAMPTZ)
CREATE TABLE products (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    price INTEGER NOT NULL,
    created_at TEXT, updated_at TEXT, deleted_at TEXT
);
```

```rust
use renox::prelude::*;
use serde::Serialize;

#[derive(Model, Serialize, Default, Debug, Clone)]
#[model(table = "products", soft_deletes)]
struct Product {
    id: i64, // 0 = not saved yet
    user_id: i64,
    name: String,
    price: i64,
    created_at: Option<DateTime>,
    updated_at: Option<DateTime>,
    deleted_at: Option<DateTime>,
}

async fn queries(db: &Db) -> Result {
    let mut tea = Product::create(db, Product { name: "Tea".into(), price: 9_000, ..Default::default() }).await?;
    tea.price = 10_000;
    tea.save(db).await?; // UPDATE; sets updated_at
    let cheap = Product::query()
        .where_op("price", "<", 20_000)
        .where_like("name", "%tea%") // ignores case
        .order_by("name")
        .limit(10)
        .get(db)
        .await?;
    let one = Product::find_or_404(db, tea.id).await?; // missing row -> 404 page
    let total = Product::query().count(db).await?;
    let q = "kopi";
    let found = Product::query()
        .where_any(|any| any.where_like("name", format!("%{q}%")).where_op("price", "<", 5_000)) // (… OR …)
        .when(!q.is_empty(), |query| query.where_not_null("user_id"))
        .where_between("price", 1_000, 50_000)
        .get(db)
        .await?;
    let revenue = Product::query().sum::<i64, _>(db, "price").await?; // avg, min, max
    let names: Vec<String> = Product::query().order_by("name").pluck(db, "name").await?;
    Product::where_eq("user_id", 1).update(db, &[("price", &12_000)]).await?; // sets updated_at
    Product::where_eq("id", tea.id).increment(db, "price", 500).await?;
    let first = Product::where_eq("name", "Tea").first_or_404(db).await?;
    Product::insert_many(db, vec![Product { name: "Kopi".into(), ..Default::default() }]).await?;
    tea.delete(db).await?; // soft delete; .with_trashed() / .only_trashed() / restore()
    let _ = (cheap, one, total, found, revenue, names, first);
    Ok(())
}

async fn more_queries(db: &Db) -> Result {
    let today = Product::query()
        .where_raw("DATE(created_at) = DATE(?)", [renox::db::now()]) // SQL the builder lacks
        .order_by_raw("price DESC, id")
        .get(db)
        .await?;
    let per_owner: Vec<(i64, i64)> = Product::query() // a report, read into a tuple
        .group_by("user_id")
        .having_raw("COUNT(*) > ?", [1])
        .select_as(db, "user_id, COUNT(*)")
        .await?;
    let mut tea = Product::where_eq("name", "Tea")
        .update_or_create(db, || Product { name: "Tea".into(), ..Default::default() }, |p| p.price = 9_500)
        .await?; // also first_or_new
    tea.refresh(db).await?; // reload after someone else changed it
    let page = Product::query().simple_paginate(db, 1, 50).await?; // no COUNT: prev/next only
    let feed = Product::query().cursor_paginate(db, None, 50).await?; // .next_cursor for the next call
    let (sql, values) = Product::where_eq("price", 1).to_sql(db.dialect())?; // debugging
    let moved = db
        .transaction_retrying(3, |tx| {
            // committed on Ok, rolled back on Err, retried on SQLite busy / PostgreSQL conflicts
            Box::pin(async move { Product::where_eq("id", 1).lock_for_update().count(&mut *tx).await })
        })
        .await?;
    let _ = (today, per_owner, page, feed, sql, values, moved);
    Ok(())
}
```

Relations are explicit: a method for one related row, and loaders for a page of rows
(`relations::belongs_to`, `has_many`, `Pivot` for many-to-many, `count_many`/`sum_many` for
counts and totals per row; one query each, no N+1). Filter by related rows with
`.where_has(Review::where_eq("stars", 5), "product_id")` / `.where_doesnt_have(…)`.
For joins and reports, use `sql("…").fetch_as::<T>(&db)` with `#[derive(FromRow)]` or a tuple.
See [docs/relations.md](docs/relations.md): pivot columns (`attach_with`, `load_with_pivot`,
`toggle`) and polymorphic relations (`Morph`) are there too.

## Model hooks, partial saves, encrypted values

```rust
use renox::db::ModelHooks;
use renox::prelude::*;

#[derive(Model, serde::Serialize, Default, Clone)]
#[model(table = "posts", hooks)]
struct Post {
    id: i64,
    title: String,
    slug: String,
    views: i64,
    api_secret: String, // stored encrypted
}

impl ModelHooks for Post {
    // Before INSERT/UPDATE; an Err stops the save. Also `deleting` before a delete.
    fn saving(&mut self, _creating: bool) -> Result {
        self.slug = self.title.to_lowercase().replace(' ', "-");
        Ok(())
    }

    // After the write; also `deleted`. The current app, in a request, job, task or command:
    async fn saved(&self, _created: bool) -> Result {
        if let Some(state) = renox::context::app() {
            state.cache.forget("posts.latest").await?;
        }
        Ok(())
    }
}

async fn edit(state: &AppState, id: i64) -> Result {
    let db = &state.db;
    let original = Post::find_or_404(db, id).await?;
    let mut post = original.clone();
    post.title = "New title".into();
    post.save_only(db, &["title"]).await?; // only these columns (+ updated_at); `views` untouched
    post.save_changes(db, &original).await?; // the columns that differ; false if none
    post.api_secret = state.encrypt("sk_live_123"); // AES-256-GCM under APP_KEY
    let secret = state.decrypt(&post.api_secret)?; // Err if tampered or another key
    let _ = secret;
    Ok(())
}
```

Hooks run for `save`, `create`, `save_only`, `save_changes`, `delete` and `force_delete`, not
for bulk `Query::update`/`delete` or `insert_many`.

## Every field type (details in docs/types.md)

```rust
use renox::chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use renox::db::Json;
use renox::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(DbEnum, Debug, Clone, Copy, PartialEq, Default)]
enum Size { Small, #[default] Medium, Large } // <select>; TEXT "small" | "medium" | "large"

#[derive(Model, Serialize, Default)]
#[model(table = "products")]
struct Product {
    id: i64,
    name: String,                 // <input>            TEXT
    price: i64,                   // money in rupiah    INTEGER / BIGINT
    weight_kg: f64,               // step="0.01"        REAL / DOUBLE PRECISION
    available: bool,              // checkbox           INTEGER 0/1 / BOOLEAN
    size: Size,                   // <select>           TEXT
    colors: Json<Vec<String>>,    // <select multiple>  TEXT / JSONB
    opens_at: Option<NaiveTime>,          // type=time            TEXT / TIME
    launch_at: Option<NaiveDateTime>,     // type=datetime-local  TEXT / TIMESTAMP
    released_on: Option<NaiveDate>,       // type=date            TEXT / DATE
}

#[derive(Deserialize)]
struct ProductForm {
    available: bool,     // "on" → true; unchecked (not sent) → false
    #[serde(default)]
    colors: Vec<String>, // colors=black&colors=red; nothing chosen → []
    size: Size,          // "huge" → a validation error, reported with the others
}
```

## Pagination

```rust
use renox::prelude::*;
#[derive(Model, serde::Serialize, Default)]
#[model(table = "products")]
struct Product { id: i64, name: String }

async fn index(State(db): State<Db>, Page(page): Page) -> Result<View> {
    let products = Product::query().latest().paginate(&db, page, 20).await?;
    Ok(view("products/index.html", context! { products })) // products.items, .total, …
}
```

## Seeders and factories

```rust
use renox::fake::{Fake, faker::lorem::en::Word};
use renox::prelude::*;

#[derive(Model, serde::Serialize, Default)]
#[model(table = "products")]
struct Product { id: i64, name: String, price: i64 }

impl Factory for Product {
    fn definition() -> Self {
        Product { name: Word().fake(), price: (5_000..50_000).fake(), ..Default::default() }
    }
}

fn seeders(app: App) -> App {
    // `rnx db:seed`, or `rnx migrate:fresh --seed`
    app.seeder(|db| async move {
        Product::create_many(&db, 50).await?; // in one transaction
        // Seeders run in the app's context: config, encrypt, the cache, random_token().
        let state = renox::context::app().expect("in a seeder");
        let _invite = (state.encrypt("secret"), renox::random_token());
        Ok(())
    })
}

async fn in_a_test(db: &Db) -> Result {
    let draft = Product::make();                // not saved
    let saved = Product::create_one(db).await?; // saved
    let _ = (draft, saved);
    Ok(())
}
```

```html
<section id="products">
  {% for p in products.items %}<p>{{ p.name }}</p>{% endfor %}
  {# page links swap just this section and keep other query parameters (?q=…) #}
  <div hx-boost="true" hx-target="#products" hx-select="#products" hx-swap="outerHTML">
    {% from "renox/pagination.html" import pagination %}{{ pagination(products) }}
  </div>
</section>
```

## Auth, policies, gates

```rust
use renox::prelude::*;
#[derive(Model, serde::Serialize, Default)]
#[model(table = "products")]
struct Product { id: i64, user_id: i64 }

impl Policy for Product {
    fn allows(&self, user: &User, ability: &str) -> bool {
        match ability {
            // has_role: the current request's roles (the Permissions module)
            "update" | "delete" => self.user_id == user.id || user.has_role("admin"),
            _ => false,
        }
    }
}

// Routes::new().resource("/products", "products", Resource::new().index(index).show(show)…)
// registers index/create/store/show/edit/update/destroy as Laravel does (products.index, …).
// `AuthUser` sends guests to the login page; use `Option<AuthUser>` when optional.
// `Path` is Renox's: `/products/abc` for a `Path<i64>` is a 404, like a missing product.
async fn edit(State(db): State<Db>, user: AuthUser, Path(id): Path<i64>) -> Result<View> {
    let product = Product::find_or_404(&db, id).await?;
    user.authorize("update", &product)?; // 403 unless allowed
    Ok(view("products/edit.html", context! { product }))
}

fn gates(app: App) -> App {
    // A gate on a column the app added to `users`; for roles use the `Permissions` module
    // (below, "Tenants, roles and permissions") and docs/authorization.md.
    app.gate("admin", |user| user.get::<String>("role").as_deref() == Some("admin")) // user.gate("admin")?
        // May query the database; check with `user.gate_async("billing").await?` in handlers.
        .gate_async("billing", |user, state| async move {
            let n: i64 = renox::db::sql("SELECT COUNT(*) FROM team_admins WHERE user_id = ?")
                .bind(user.id)
                .scalar(&state.db)
                .await?;
            Ok(n > 0)
        })
}

// Extra registration fields (add the inputs to your own renox/auth/register.html).
fn auth() -> Auth {
    Auth::new()
        .registration_rules(|form, v| {
            v.field("phone", &form.get("phone")).required().max(20);
        })
        .on_registered(|state, mut user, form| async move {
            user.set(&state.db, "phone", form.get("phone")).await // a failure undoes the sign-up
        })
}

// API tokens (`Authorization: Bearer …`, no CSRF): issue one, and revoke the
// one a request used ("log out" on one device) or all of them.
async fn issue(State(db): State<Db>, user: AuthUser) -> Result<String> {
    Ok(user.create_token(&db, "phone", None).await?.plain) // shown once
}
async fn log_out_device(State(db): State<Db>, user: AuthUser) -> Result<StatusCode> {
    if let Some(id) = user.token_id() {
        user.revoke_token(&db, id).await?; // user.revoke_tokens(&db) for every device
    }
    Ok(StatusCode::NO_CONTENT)
}

// For `{% if can('update', product) %}` in the view, attach the abilities.
async fn index(State(db): State<Db>, user: Option<AuthUser>) -> Result<View> {
    let products: Vec<_> = Product::query()
        .get(&db)
        .await?
        .into_iter()
        .map(|p| Can::new(p, user.as_deref(), &["update", "delete"]))
        .collect(); // on a page: .paginate(…).await?.map(|p| Can::new(…))
    Ok(view("products/index.html", context! { products }))
}
```

```rust
use renox::prelude::*;

fn back_office() -> Auth {
    Auth::new()
        .account()                 // /account: profile, password, other devices, delete
        .password_rules(renox::validation::Password::min(12).mixed_case().numbers())
        .verify_email()            // mails a link; guard routes with .require_verified()
        .without_registration()    // no /register: an admin adds users
        .redirect_to("/dashboard") // after login, when no page asked for one
}

#[derive(serde::Deserialize)]
struct Login { email: String, password: String }

// Your own login (Auth's /login already does this, with a lockout).
async fn sign_in(State(state): State<AppState>, session: Session, Form(f): Form<Login>) -> Result<Redirect> {
    let Some(user) = User::attempt(&state.db, &f.email, &f.password).await? else {
        return Err(abort(StatusCode::UNAUTHORIZED, "Wrong email or password."));
    };
    renox::auth::login(&session, &user, None)?; // Some(state.config.remember_lifetime): "remember me"
    Ok(Redirect::to("/dashboard"))
}

async fn sign_out(State(db): State<Db>, session: Session) -> Result<Redirect> {
    renox::auth::logout(&db, &session).await?; // this device (a copied cookie dies too)
    Ok(Redirect::to("/"))
}

async fn security(State(db): State<Db>, session: Session, user: AuthUser) -> Result<View> {
    let mut me = user.user().clone();
    renox::auth::change_password(&db, &session, &mut me, "a new password").await?; // others end
    renox::auth::logout_other_devices(&db, &session, &me).await?; // keeps this one
    // me.revoke_sessions(&db) ends every session, this one too; me.delete_account(&db)
    let admin = user.allows("admin");               // a gate as a bool; allows_async for gate_async
    let billing = user.allows_async("billing").await?;
    Ok(view("account/security.html", context! { admin, billing }))
}

// The database channel's inbox.
async fn inbox(State(db): State<Db>, user: AuthUser) -> Result<View> {
    let notifications = user.notifications(&db, 20).await?; // newest first; .data, .read_at
    let unread = user.unread_notification_count(&db).await?;
    user.mark_all_notifications_read(&db).await?; // or mark_notification_read(&db, id)
    Ok(view("inbox.html", context! { notifications, unread }))
}
```

Routes that should ask for the password again (after three hours): `.require_password_confirmed()`
(the `Auth` module serves `/confirm-password`). Users imported from Laravel keep their bcrypt
hashes and are moved to Argon2id when they next log in.

## Auth events and the audit log

```rust
use renox::prelude::*;
use renox::audit::{self, Audit, Entry};
use renox::auth::events::{LoginFailed, Registered};

fn app() -> App {
    App::new()
        .module(Auth::new())
        .module(Audit) // an audit_logs table; records every auth event below
        // Registered, LoggedIn, LoginFailed, LockedOut, LoggedOut, PasswordReset,
        // PasswordChanged, EmailVerified, ProfileUpdated, OtherDevicesLoggedOut, AccountDeleted
        .listen(|e: Registered, state| async move {
            let _ = (e.user_id, state); // e.g. queue a welcome mail
            Ok(())
        })
        .listen(|e: LoginFailed, _state| async move {
            let _ = (e.email, e.ip); // e.g. alert on many failures from one address
            Ok(())
        })
}

async fn refund(State(db): State<Db>, user: AuthUser, ClientIp(ip): ClientIp) -> Result<&'static str> {
    audit::record(&db, Entry::new("order.refunded").user(user.id).subject("orders", 42)
        .data(json!({ "amount": 75_000 })).ip(ip)).await?;
    let history = audit::for_subject(&db, "orders", 42, 20).await?; // also for_user, latest
    let _ = history;
    Ok("refunded")
}
```

`rnx audit:prune --days 365` deletes older entries.

## Tenants, roles and permissions

```rust
use renox::prelude::*;
use renox::auth::{Permissions, permissions};
use renox::axum::{extract::Request, middleware::{Next, from_fn}};

#[derive(Clone)]
struct CurrentTeam(i64);

// Every query of the model starts with its default scope; `Project::unscoped()` skips it.
#[derive(Model, serde::Serialize, Default)]
#[model(table = "projects", default_scope = "team_only")]
struct Project { id: i64, team_id: i64, name: String }

fn team_only(query: renox::db::Query<Project>) -> renox::db::Query<Project> {
    match renox::context::get::<CurrentTeam>() { // this request's (or job's) value
        Some(team) => query.where_eq("team_id", team.0),
        None => query.none(), // no team, no rows
    }
}

async fn pick_team(user: Option<AuthUser>, req: Request, next: Next) -> Response {
    if let Some(team) = user.as_ref().and_then(|u| u.get::<i64>("team_id")) {
        renox::context::set(CurrentTeam(team)); // jobs/commands: set it from the payload
    }
    next.run(req).await
}

fn app() -> App {
    App::new()
        .module(Auth::new())
        .module(Permissions) // roles, permissions, and loading them per request
        .layer(from_fn(pick_team))
        .gate_before(|user, _ability| (user.get::<bool>("super_admin") == Some(true)).then_some(true))
}

fn routes() -> Routes {
    let admin = Routes::new().get("/admin", || async { "hi" }).require_gate("admin"); // 403 unless
    let editors = Routes::new().get("/drafts", || async { "" }).require_role("editor");
    let publish = Routes::new().post("/publish", || async { "" }).require_permission("posts.publish");
    let api = Routes::new().post("/api/orders", || async { "" }).require_ability("orders:write");
    admin.merge(editors).merge(publish).merge(api) // shown in route:list as gate:admin, …
}

async fn setup(db: &Db, user: &User) -> Result {
    permissions::define_role(db, "editor", &["posts.create", "posts.publish"]).await?; // exactly these
    user.assign_role(db, "editor").await?; // also remove_role, sync_roles, roles, permissions
    let token = user.create_token_with(db, "reports", &["orders:read"], None).await?; // limited
    let _ = token.plain;
    Ok(())
}

async fn check(user: AuthUser) -> String {
    // allows: gate_before, a gate, then permissions. has_role/has_permission: exactly that.
    format!("{} {} {}", user.allows("posts.publish"), user.has_role("editor"), user.token_can("orders:read"))
}

#[derive(serde::Deserialize)]
struct ProjectForm { name: String, #[serde(default)] id: i64 }

impl Validate for ProjectForm {
    fn rules(&self, v: &mut Validator) {
        let team = renox::context::get::<CurrentTeam>().map_or(0, |t| t.0);
        v.field("name", &self.name)
            .unique("projects", "name")
            .ignore(self.id)
            .where_eq("team_id", team)    // unique per team
            .where_null("deleted_at");    // soft-deleted rows don't count
    }
}
```

In templates: `{% if can('posts.publish') %}` (a gate or a permission) and `auth.roles`.
`rnx tokens:prune` deletes API tokens that expired more than a day ago.

## HTMX

```rust
use renox::prelude::*;
use renox::{HxPushUrl, HxReswap, HxRetarget, Toast};

// A form posted with hx-post gets just the `list` block of the page back;
// a normal request gets the whole page.
async fn add(htmx: Htmx) -> Result<Response> {
    let page = view("todos/index.html", context! {}).fragment("list");
    if htmx.request {
        return Ok((HxTrigger("todo-added".into()), page).into_response());
    }
    Ok(Redirect::to("/todos").into_response())
}

async fn after_delete() -> HxRedirect {
    HxRedirect("/todos".into()) // the browser navigates there
}

async fn after_save(htmx: Htmx) -> Response {
    htmx.redirect("/todos") // HX-Redirect for htmx posts, 303 for plain forms
}

async fn after_toggle() -> (Toast, HxRetarget, HxReswap, HxPushUrl, View) {
    // A toast (HX-Trigger for htmx; the next page otherwise), and several blocks at once:
    // blocks after the first need an id and hx-swap-oob="true".
    (
        Toast::success("Order updated"), // also info, warning, error (errors stay until dismissed)
        HxRetarget("#orders".into()),
        HxReswap("outerHTML".into()),
        HxPushUrl("/orders?status=open".into()),
        view("orders/index.html", context! {}).fragment("rows").also("count"),
    )
}

async fn after_import(htmx: Htmx) -> Response {
    if htmx.wants_fragment() { // htmx, but not hx-boost
        return HxRefresh.into_response(); // the browser reloads the page
    }
    Redirect::to("/todos").into_response()
}
```

`Back` redirects to the previous page (the Referer). `<form data-live-validate>` (with the UI
kit's script) checks each field against `Valid<T>`'s rules as it's left, without running the
handler. HTMX requests send the CSRF token by
themselves. Errors of list items (`photos.1`, `tags.0`) show at the list's input and its
`data-error-for="photos"` slot, and `{{ error('photos') }}` includes them. Boosted requests (`hx-boost`) get whole pages, so pair them with `hx-select`.

## Raw SQL and transactions (SQLite and PostgreSQL)

```rust
use renox::prelude::*;

async fn report(db: &Db) -> Result {
    let rows = renox::db::sql("SELECT name, price FROM products WHERE price < ?")
        .bind(20_000)
        .fetch_all(db)
        .await?;
    let name: String = rows[0].try_get("name")?;
    let total: i64 = renox::db::sql("SELECT CAST(SUM(price) AS BIGINT) FROM products")
        .scalar(db)
        .await?;

    let mut tx = db.begin().await?; // pass `&mut tx` wherever `db` goes
    renox::db::sql("UPDATE products SET price = price + ?").bind(1_000).execute(&mut tx).await?;
    tx.commit().await?; // dropped without commit = rolled back

    // Retried on SQLite busy / PostgreSQL conflicts; borrows freely, may roll back with a value.
    let raised = db
        .retrying(3, || async {
            let mut tx = db.begin().await?;
            let n = renox::db::sql("UPDATE products SET price = ? WHERE price < ?")
                .bind(total) // a local, borrowed
                .bind(total)
                .execute(&mut tx)
                .await?;
            if n > 100 {
                tx.rollback().await?;
                return Ok(0); // too many: undone, still an answer
            }
            tx.commit().await?;
            Ok(n)
        })
        .await?;
    let _ = (name, raised);
    Ok(())
}
```

SQLite has one writer: while a transaction is open, write through `&mut tx`, not `db` (that
waits for the transaction and fails with "database is locked"). Queries fail with a
`renox::db::DbError` (`is_unique_violation()`, `is_foreign_key_violation()`, `is_row_not_found()`,
`is_timeout()`); a duplicate that slips past a `unique` rule answers 409.

Migrations run in a transaction each. A migration with `CREATE INDEX CONCURRENTLY` or its own
`BEGIN … COMMIT` runs without one, as does one with a `-- renox:no-transaction` line. Two
`migrate` runs at once wait for each other; `migrate:status` flags edited and missing files.

## Jobs, events, schedule, mail

```rust
use renox::prelude::*;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Serialize, Deserialize)]
struct SendReceipt {
    order_id: i64, // keep jobs small: ids, not whole models
}

impl Job for SendReceipt {
    const NAME: &'static str = "send-receipt";
    const QUEUE: &'static str = "mail";                // default "default"; queue:work --queue mail
    const MAX_ATTEMPTS: u32 = 5;                       // default 3
    const TIMEOUT: Duration = Duration::from_secs(30); // per attempt; default 60 s

    // Also: const ENCRYPTED (payload sealed with APP_KEY), const UNIQUE_FOR + fn unique_id
    // (one queued at a time), fn middleware (Middleware::without_overlapping(key),
    // rate_limited(key, max, per)), async fn failed(self, state, error) once it fails for good.

    async fn handle(self, ctx: JobContext) -> Result {
        let mail = ctx.state.mail_view(
            "buyer@example.com",
            "Your receipt",
            "mail/receipt", // resources/views/mail/receipt.html (+ .txt)
            context! { order_id => self.order_id },
        )?;
        ctx.state.mailer.send(mail).await
    }
}

#[derive(Clone)]
struct OrderPlaced {
    order_id: i64,
}

impl Event for OrderPlaced {}

fn background(app: App) -> App {
    app.job::<SendReceipt>()
        .listen(|event: OrderPlaced, state| async move {
            state.dispatch(SendReceipt { order_id: event.order_id }).await?;
            Ok(())
        })
        .schedule(|s| {
            // Also every_minute(name, task), hourly(name, task), every(Duration, name, task).
            s.every_minutes(5, "sync-stock", |_state| async move { Ok(()) }) // :00, :05, … on the clock
                .weekdays() // also weekends(), days(&[Weekday::Sat])
                .between("08:00", "18:00");
            s.daily_at("02:00", "cleanup", |state| async move { // in APP_TIMEZONE
                renox::db::sql("DELETE FROM carts WHERE updated_at < ?")
                    .bind(renox::db::now() - renox::chrono::TimeDelta::days(30))
                    .execute(&state.db)
                    .await?;
                Ok(())
            });
            s.cron("30 9 * * 1-5", "standup", |_state| async move { Ok(()) }) // min hour day month weekday
                .timezone("Europe/Amsterdam") // instead of APP_TIMEZONE; DST handled
                .on_failure(|_state, err| async move { eprintln!("standup failed: {err:?}") })
                .on_success(|_state| async move {});
            s.weekly_on(renox::chrono::Weekday::Mon, "07:00", "weekly", |_state| async move { Ok(()) });
            s.monthly_on(1, "00:05", "invoices", |_state| async move { Ok(()) });
        })
}

async fn place_order(State(state): State<AppState>) -> Result<Redirect> {
    state.emit(OrderPlaced { order_id: 1 }).await?; // listeners run, the job is queued
    state.queue.dispatch_after(SendReceipt { order_id: 1 }, Duration::from_secs(3600)).await?; // in an hour
    Ok(Redirect::to("/orders"))
}

async fn several(State(state): State<AppState>) -> Result<String> {
    let queue = &state.queue;
    queue.chain().then(SendReceipt { order_id: 1 }).then(SendReceipt { order_id: 2 }).dispatch().await?; // in order
    let batch = queue
        .batch("receipts") // side by side; the first failure cancels the rest unless .allow_failures()
        .push(SendReceipt { order_id: 3 })
        .push(SendReceipt { order_id: 4 })
        .then(SendReceipt { order_id: 5 }) // all succeeded; also .catch(job), .finally(job)
        .dispatch()
        .await?;
    let progress = queue.batch_status(batch).await?.map_or(0, |b| b.progress()); // 0–100
    queue.dispatch_on("high", SendReceipt { order_id: 6 }).await?; // queue:work --queue high,default
    state.dispatch_sync(SendReceipt { order_id: 7 }).await?; // now, in this request
    Ok(format!("{progress}%"))
}

async fn checkout(State(state): State<AppState>) -> Result {
    let mut tx = state.db.begin().await?;
    // … insert the order with `&mut tx`
    state.queue.dispatch_in(&mut tx, SendReceipt { order_id: 1 }).await?; // only if committed
    tx.commit().await?;
    Ok(())
}

fn card_number(raw: &str) -> Result<u64> {
    // Retrying can't fix a bad number: the job goes straight to failed_jobs.
    raw.parse::<u64>().map_err(Error::permanent)
}
```

Jobs and scheduled tasks run inside `rnx serve` / `my-app serve` (`QUEUE_WORKERS`, `SCHEDULER`).
`schedule:list` shows each task's next run and zone, `schedule:run NAME` runs one now (also
`kernel.run_scheduled(name)` in tests). `.module(renox::queue::Dashboard)` adds `/_renox/queue`
(waiting jobs, throughput, failed jobs to retry or forget, batches) for users who pass the
`view-queue-dashboard` gate (`renox::queue::DASHBOARD_GATE`); `state.queue.stats()` has the same
numbers for monitoring.
A job that errors, panics or passes its `TIMEOUT` is retried up to `MAX_ATTEMPTS`, then moved to
`failed_jobs` (`queue:failed`, `queue:retry`, `queue:forget`, `queue:prune-failed`); a panicking
task or listener doesn't stop the others.

## Mail and notifications

```rust
use renox::prelude::*;
use renox::auth::{Channel, Notification, Recipient};
use renox::mail::Mail;

async fn send_invoice(state: &AppState, pdf: Vec<u8>) -> Result {
    let mail = state
        .mail_view("budi@example.com", "Invoice INV-001", "mail/invoice", context! {})? // .html + .txt
        .also_to("siti@example.com")
        .cc("sales@example.com")
        .bcc("archive@example.com")
        .reply_to("Toko Kopi <halo@toko.id>")
        .from("Billing <billing@toko.id>") // instead of MAIL_FROM_*
        .attach("INV-001.pdf", "application/pdf", pdf);
    state.queue_mail(mail).await?; // or state.mailer.send(mail).await? for now
    Ok(())
}

struct OrderShipped { order_id: i64 }

impl Notification for OrderShipped {
    fn kind(&self) -> &'static str { "order-shipped" }
    fn channels(&self) -> Vec<Channel> {
        vec![Channel::Mail, Channel::Database, Channel::Custom("whatsapp")]
    }
    fn channels_for(&self, to: &Recipient) -> Vec<Channel> {
        // Per recipient; defaults to channels().
        let mut channels = self.channels();
        if to.address("whatsapp").is_none() {
            channels.retain(|c| *c != Channel::Custom("whatsapp"));
        }
        channels
    }
    fn to_mail(&self, to: &Recipient, state: &AppState) -> Result<Mail> {
        // Built in the recipient's language (Recipient::locale: in_locale(..) or users.locale):
        // t() in the mail view and state.current_lang() here use it.
        let subject = state.current_lang().t("mail.shipped", &[("id", &self.order_id)]);
        Ok(Mail::new(to.email().unwrap_or_default(), subject, "On its way."))
    }
    fn to_database(&self, _: &Recipient) -> renox::serde_json::Value {
        json!({ "order_id": self.order_id }) // user.unread_notifications(&db)
    }
    fn to_channel(&self, _: &str, _: &Recipient) -> Result<renox::serde_json::Value> {
        Ok(json!({ "text": format!("Order #{} shipped", self.order_id) }))
    }
}

fn channels(app: App) -> App {
    // Your own channel: call WhatsApp, SMS or Slack with what `to_channel` built.
    app.channel("whatsapp", |_state, to: Recipient, message| async move {
        let phone = to.address("whatsapp").or_else(|| to.user.as_ref()?.get("phone"));
        let _ = (phone, message);
        Ok(())
    })
}

async fn ship(state: &AppState, user: &User) -> Result {
    state.notify(user, &OrderShipped { order_id: 7 }).await?;       // now
    state.notify_later(user, &OrderShipped { order_id: 7 }).await?; // one queued job per channel
    let guest = Recipient::to("mail", "guest@example.com").and("whatsapp", "+628123").in_locale("id");
    state.notify_to(&guest, &OrderShipped { order_id: 7 }).await   // no account: no database row
}
```

Mail views can `{% from "renox/mail/components.html" import button, panel, table, divider %}`:
`{{ button(url, t('mail.track')) }}`, `{% call panel() %}…{% endcall %}`,
`{{ table(rows, head=[…], total=[…]) }}`. `state.mail_view_in("id", …)` renders in a given
language; `state.lang("id").t(…)` and `renox::i18n::set_current_locale("id")` (for the rest of
a job) help code outside a request.

## Calling other services (HTTP)

```rust
use renox::prelude::*;
use std::time::Duration;

#[derive(serde::Deserialize)]
struct Rate { idr: f64 }

async fn rate(state: &AppState) -> Result<f64> {
    let rate: Rate = state
        .http // also post/put/patch/delete; .json(&body), .form(&body), .header(..), .basic_auth(..)
        .get("https://api.example.com/rates")
        .query(&[("base", "USD")])
        .bearer("token")
        .timeout(Duration::from_secs(5)) // 30 s by default
        .retry(3, Duration::from_millis(200)) // connection errors, 429, 5xx
        .send()
        .await?
        .error_for_status()? // 4xx/5xx → Err
        .json()?;
    Ok(rate.idr)
}
```

In tests: `let http = app.fake_http(); http.on("https://api.example.com/*",
FakeResponse::json(200, json!({ "idr": 16000.0 })));` then `http.assert_sent(|r| …)`. Requests
without a fake fail, so tests never reach the network. Scheduled tasks can ping health checks:
`.ping_before(url)`, `.then_ping(url)`, `.ping_on_success(url)`, `.ping_on_failure(url)`.

## Cookies and downloads

```rust
use renox::prelude::*;
use renox::{Cookies, Download, SetCookie};
use std::time::Duration;

async fn remember_theme(State(state): State<AppState>) -> (SetCookie, Redirect) {
    let cookie = SetCookie::new(&state, "theme", "dark").max_age(Duration::from_secs(365 * 86_400));
    (cookie, Redirect::to("/")) // HttpOnly, SameSite=Lax, Secure on https; SetCookie::encrypted / remove
}

async fn theme(cookies: Cookies) -> String {
    cookies.get("theme").unwrap_or_default() // get_encrypted for SetCookie::encrypted
}

async fn invoice(State(state): State<AppState>) -> Result<Download> {
    let pdf = state.storage.get("invoices/1.pdf").await?.unwrap_or_default();
    Ok(Download::bytes("INV-001.pdf", "application/pdf", pdf).inline()) // or attachment (default)
    // Download::file(path, name).await (streamed), Download::from_storage(&storage, key, name),
    // Download::stream(name, type, stream) for a CSV written row by row
}
```

## Cache, session, uploads, translations

```rust
use renox::prelude::*;
use std::time::Duration;

async fn misc(State(state): State<AppState>, session: Session, lang: Lang) -> Result<String> {
    let count: i64 = state
        .cache
        .remember("products.count", Duration::from_secs(60), || async {
            renox::db::sql("SELECT COUNT(*) FROM products").scalar(&state.db).await.map_err(Into::into)
        })
        .await?;
    let visits = state.cache.increment("visits", 1).await?; // atomic; also decrement
    let first = state.cache.add("promo-sent:7", &true, None).await?; // false if already there
    let otp: Option<String> = state.cache.pull("otp:7").await?; // read and remove
    let lock = state.cache.lock("stock:42", Duration::from_secs(30)); // across servers with CACHE_STORE=database
    if let Some(guard) = lock.try_acquire().await? {
        // … one worker at a time; lock.block(Duration::from_secs(5)) waits instead (423 after)
        guard.release().await?; // or drop it
    }
    let files = state.storage.list("invoices/2026").await?; // key, size, modified; any depth
    state.storage.copy("public/a.png", "public/b.png").await?; // also rename (move), size, delete_all(prefix)
    let _ = (visits, first, otp, files);
    session.put("cart", vec![1, 2, 3])?;
    let cart: Option<Vec<i64>> = session.get("cart");
    // Also: pull (read and remove), remove, regenerate_token(), set_lifetime(minutes).
    let _ = cart;
    Ok(lang.choice("cart.count", count, &[])) // "12 items"; lang.t("cart.hello", &[("name", &"Arif")])
}

async fn switch_language(session: Session, back: Back) -> Result<Back> {
    renox::i18n::set_locale(&session, "id")?; // this visitor's language from the next request on
    Ok(back)
}

#[derive(serde::Deserialize)]
struct PhotoForm {
    photo: Upload, // from a multipart form
}

impl Validate for PhotoForm {
    fn rules(&self, v: &mut Validator) {
        v.field("photo", &self.photo).required().image().max(2048); // KB
    }
}

async fn upload(State(state): State<AppState>, Valid(form): Valid<PhotoForm>) -> Result<String> {
    let key = form.photo.store_public(&state.storage, "photos").await?;
    Ok(state.storage.url(&key)) // {{ storage_url(key) }} in templates
}
```

Translations live in `resources/lang/<locale>.json`, nested or flat. `one|many` texts are plurals:

```json
{ "cart": { "count": "One item|:count items", "hello": "Hello, :name" } }
```

```html
<p>{{ t('cart.count', count=cart_count) }} · {{ t('cart.hello', name=auth.user.name) }}</p>
```

## Signed URLs and private files

```rust
use renox::prelude::*;
use renox::signed::ValidSignature;
use std::time::Duration;

// Routes::new().get("/invoices/{id}", show_invoice).name("invoices.show")
async fn share(State(state): State<AppState>, Path(id): Path<i64>) -> Result<String> {
    state.signed_url("invoices.show", &[&id], Duration::from_secs(3600)) // absolute; an hour
}

async fn show_invoice(_: ValidSignature, Path(id): Path<i64>) -> String {
    format!("Invoice {id}") // an altered or expired link gets 403
}

async fn private_file(State(state): State<AppState>) -> Result<Redirect> {
    // A key outside `public/`, as a link that works for 10 minutes (presigned on S3).
    let url = state.storage.temporary_url(&state, "invoices/1.pdf", Duration::from_secs(600)).await?;
    Ok(Redirect::to(&url))
}
```

## Security: CSP, CORS, webhooks

```rust
use renox::prelude::*;

fn secured(app: App) -> App {
    // Every response gets nosniff, Referrer-Policy, X-Frame-Options, HSTS
    // (production + https) and a Content-Security-Policy (`CSP=relaxed|strict|off`).
    app.csp(|csp| {
        csp.allow("script-src", "https://www.googletagmanager.com")
            .allow("frame-src", "https://www.youtube.com");
    })
}

fn routes() -> Routes {
    let api = Routes::new()
        .get("/api/stock", || async { "12" })
        .cors(&["https://app.example.com"]); // or &["*"]
    let webhooks = Routes::new()
        .post("/webhooks/payment", || async { StatusCode::OK })
        .without_csrf(); // no session: check the gateway's signature instead
    api.merge(webhooks)
}
```

A payment gateway's webhook: verified, stored once per event, processed in the queue
(`webhook:failed`, `webhook:retry <id>`):

```rust
use renox::prelude::*;
use renox::webhook;

struct Xendit;

impl Webhook for Xendit {
    const PROVIDER: &'static str = "xendit";

    fn verify(request: &WebhookRequest, state: &AppState) -> Result {
        let token = webhook::secret(state, "XENDIT_CALLBACK_TOKEN")?; // from .env
        let sent = request.header("x-callback-token").unwrap_or_default();
        webhook::ensure(webhook::same(sent, &token)) // or verify_hmac_sha256, verify_timestamped
    }

    fn event_id(request: &WebhookRequest) -> Result<String> {
        let invoice: serde_json::Value = request.json()?;
        Ok(format!("{}:{}", invoice["id"], invoice["status"]))
    }

    async fn handle(call: WebhookCall, ctx: JobContext) -> Result {
        let invoice: serde_json::Value = call.json()?;
        let _ = (invoice, ctx.state); // mark the order paid; errors are retried
        Ok(())
    }
}

// Module::routes:   Routes::new().webhook::<Xendit>("/webhooks/xendit")
// Module::register: app.webhook::<Xendit>();
```

With `CSP=strict`, an inline script needs `<script nonce="{{ csp_nonce() }}">`, and Alpine
expressions must stay simple (move statements into `Alpine.data(...)`).

## SEO and analytics

```html
{% block seo %}{{ seo(title=product.name ~ " · " ~ app.name, description=product.summary,
                      image=storage_url(product.photo), type="product") }}{% endblock %}
{# → <title>, description, canonical URL, OpenGraph and Twitter card tags #}
```

```rust
use renox::analytics::{self, GaClientId, ServerEvent};
use renox::prelude::*;
use renox::seo::Sitemap;

// Routes::new().get("/sitemap.xml", sitemap).name("sitemap")  → robots.txt links it
async fn sitemap(State(state): State<AppState>) -> Result<Sitemap> {
    Sitemap::new(&state).route("home", &[], None)?.route("products.show", &[&7], None)
}

async fn signed_up(session: Session) -> Result<Redirect> {
    // Reaches gtag / the dataLayer with this htmx swap, this page, or the next one.
    analytics::event(&session, "sign_up", json!({ "method": "email" }))?;
    Ok(Redirect::to("/welcome"))
}

async fn paid(State(state): State<AppState>, GaClientId(client): GaClientId) -> Result<StatusCode> {
    // From the server (GA4 Measurement Protocol), so ad blockers can't drop it.
    state.dispatch(ServerEvent::new(client, "purchase").param("value", 18_000).param("currency", "IDR")).await?;
    Ok(StatusCode::OK)
}
```

`.env`: `GOOGLE_SITE_VERIFICATION`, `GA4_MEASUREMENT_ID`, `GA4_API_SECRET`, `GTM_CONTAINER_ID`. Tags
are added (with the CSP nonce and sources) only in production; elsewhere pages say `noindex` and
`robots.txt` disallows everything.

## Tests

```rust
use renox::prelude::*;
use renox::testing::TestApp;

fn app() -> App {
    App::new().module(Auth::new())
}

#[renox::test]
async fn members_only() {
    let app = TestApp::new(app()).await; // in-memory SQLite (or TEST_DATABASE_URL), migrated
    let user = User::register(app.db(), "Arif", "arif@example.com", "password123").await.unwrap();

    app.get("/login").await.assert_ok().assert_see("Log in");
    app.acting_as(&user);
    app.post("/logout", &[]).await.assert_status(303);
    app.htmx().post("/register", &[("email", "")]).await.assert_invalid("email");
    app.assert_database_has("users", &[("email", &"arif@example.com")]).await;
}
```

```rust
use renox::prelude::*;
use renox::testing::TestApp;

fn app() -> App {
    App::new()
}

#[renox::test]
async fn with_settings() {
    let app = TestApp::with_config(app(), |c| {
        c.vars.insert("MIDTRANS_SERVER_KEY".into(), "test-key".into()); // what state.config.var reads
        c.locale = "id".into();
    })
    .await;
    let state = app.state(); // the AppState handlers get
    let _ = state.config.var("MIDTRANS_SERVER_KEY");
    let res = app.request().header("x-api-version", "2").json().get("/health").await;
    let body: renox::serde_json::Value = res.json(); // also res.text(), res.header("…")
    let _ = body;
}
```

```rust
use renox::prelude::*;
use renox::testing::TestApp;
use std::time::Duration;

#[derive(Clone)]
struct OrderPlaced { id: i64 }
impl Event for OrderPlaced {}

#[renox::test]
async fn test_tools() {
    let app = TestApp::new(App::new().module(Auth::new())).await;
    app.fake_events().fake_notifications(); // record instead of running / sending
    let res = app.get("/login").await;
    res.assert_view("renox/auth/login.html");
    app.assert_guest().assert_session_missing("cart");
    app.travel(Duration::from_secs(3600)); // requests and jobs see the clock an hour on
    app.run_all_jobs().await; // retries waiting for their backoff too
    app.assert_not_emitted::<OrderPlaced>().assert_nothing_notified();
    // JSON: res.assert_json_path("data.0.name", "Kopi"), res.assert_json(json!({ "total": 2 }))
    // Events/notifications: app.assert_emitted::<OrderPlaced>(|e| e.id == 7), app.assert_notified(&user, "kind")
    // A real server for a browser test: let url = app.serve().await;
}
```

Also available: `put` / `patch` (form) and `delete`, `post_json`,
`post_multipart(uri, &[("title", "x")], &[("photo", "a.png", &bytes)])`, `post_body` (exact
bytes, e.g. signed webhooks), `request().without_csrf()`, `logout()`, `csrf_token()`,
`app.kernel().call("products:import", ["file.csv"])` (an app command), `assert_redirect`,
`assert_hx_redirect`, `assert_header(name, value)`, `assert_unauthorized`, `assert_forbidden`,
`assert_not_found`, `assert_dont_see`, `assert_database_missing` / `assert_database_count`,
`queued_jobs()`, `run_jobs()`, `run_all_jobs()` (retries and delayed jobs too), `sent_mail()` /
`assert_mail_sent`, `session_cookie()` / `use_session_cookie(…)` (play a second device),
`app.kernel().run_scheduled("task")`, `confirm_password()`, `fake_http()`, and
`let (res, queries) = renox::db::capture_queries(app.get("/posts")).await;` to count the SQL a
request (or any future) runs.

## Errors, logs and debugging

```rust
use renox::prelude::*;
use renox::RequestId;
use renox::rate_limit::Limit;
use renox::report::ErrorReport;

async fn show(id: RequestId) -> String {
    format!("request {id}") // also the response's X-Request-Id and every log line's `id`
}

fn app() -> App {
    App::new()
        // 500s, jobs that failed for good and failed scheduled tasks, in the background.
        .report(|report: ErrorReport, state: AppState| async move {
            // report.kind, .message, .details, .source, .request (method, path, id, ip, user_id)
            let _ = state.http.post("https://errors.example.com/api/events").json(&report).send().await;
        })
        // A limit picked per request; routes use it with `.throttle_by("api")`.
        .rate_limiter("api", |req| match req.user {
            Some(user) if user.has_role("partner") => Limit::none(),
            Some(user) => Limit::per_minute(600).by(format!("user:{}", user.id)),
            None => Limit::per_minute(60), // per IP
        })
}
```

- `LOG_FORMAT=json` writes one JSON object per line (with the request's span: `id`, `method`,
  `uri`, `ip`); `LOG_FILE=storage/logs/app.log` appends there instead of stdout.
- A request's `X-Request-Id` is kept when a proxy sends one (8–64 of `A-Z a-z 0-9 . _ -`),
  otherwise it's generated.
- `/_renox/debug` (only with `APP_DEBUG` and `APP_ENV=local`): the last 50 requests with their
  status, time, view and SQL; a statement run 3+ times is flagged as a likely N+1. Mail is at
  `/_renox/mail`, jobs at the queue dashboard (`renox::queue::Dashboard`).
- A `throttle_by` without its `rate_limiter` stops the app at boot.

## Configuration (`.env`)

`APP_ENV` (`local` | `production` | `testing`; also `dev`/`development`, `prod`, `test`; any other
value, such as `staging`, stops the app at boot), `APP_KEY` (`rnx key:generate`; required in
production), `APP_DEBUG` (on by default in `local`), `APP_NAME`, `APP_URL`, `APP_HOST` (an IP
address, `127.0.0.1`; `0.0.0.0` in a container) and `APP_PORT` (3000), `APP_LOCALE`,
`APP_FALLBACK_LOCALE`, `APP_TIMEZONE` (`Asia/Jakarta`, `+07:00` or `UTC`),
`SESSION_LIFETIME` (minutes, 120), `REMEMBER_LIFETIME` (minutes, 43200 = 30 days),
`SESSION_COOKIE` (`renox_session`), `SESSION_DRIVER` (`cookie` | `database`: the `sessions`
table, no 4 KB limit, a new id at each login/logout), `MAIL_FROM_ADDRESS`, `MAIL_FROM_NAME` (defaults to `APP_NAME`),
`VIEWS_PATH` (`resources/views`), `PUBLIC_PATH` (`public`), `LANG_PATH` (`resources/lang`),
`DATABASE_URL` (`sqlite://storage/app.db` or `postgres://…` with the `postgres` feature),
`TEST_DATABASE_URL`, `MAIL_MAILER` (`log` | `smtp`), `QUEUE_WORKERS`, `SCHEDULER`,
`CACHE_STORE` (`memory` | `database`: with several servers, also shares rate limits and the login lock), `STORAGE_DISK` (`local` | `s3`), `UPLOAD_MAX_SIZE` (MB),
`LOG_FORMAT` (`text` | `json`), `LOG_FILE` (append to this file instead of stdout),
`CSP` (`relaxed` | `strict` | `off`), `TRUSTED_PROXIES` (`127.0.0.1,10.0.0.0/8` or `*`: behind a
proxy, rate limits, the login lock, logs and the `ClientIp` extractor use `X-Forwarded-For`).
Timeouts in seconds: `DATABASE_ACQUIRE_TIMEOUT` (5), `DATABASE_STATEMENT_TIMEOUT` (30,
PostgreSQL; 0 = none), `REQUEST_TIMEOUT` (60; 0 = none), `MAIL_TIMEOUT` (10).

Your own settings are plain `.env` lines, read with `config.var`. In tests, set them with
`TestApp::with_config(app, |c| { c.vars.insert(…); })` (see Tests).

```rust
use renox::prelude::*;

async fn pay(State(state): State<AppState>) -> Result<String> {
    let key = state
        .config
        .var("MIDTRANS_KEY") // None when missing or empty
        .ok_or_else(|| abort(StatusCode::SERVICE_UNAVAILABLE, "Payments are not set up."))?;
    let live = state.config.env == Environment::Production; // also state.config.name, .url, .debug
    Ok(format!("key of {} characters, live: {live}", key.len()))
}
```
