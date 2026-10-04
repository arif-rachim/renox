# Laravel → Renox

You know Laravel well, and you want the same way of working in Rust: generators, migrations,
models, form requests, queues, mail, a login out of the box, pages rendered on the server. Renox
gives you most of that, under names you'll recognise. This guide shows the mapping area by
area, and, more usefully, explains *why* some things look different, so you stop looking for
the Laravel version of them.

It assumes you have read the README's quick start. Each section links to the guide with the
details; [CHEATSHEET.md](../CHEATSHEET.md) has one example of every pattern, and the
[Laravel parity review](audit/2026-10-laravel-parity.md) rates every Laravel and Filament
feature against Renox.

## The big differences, on one page

**It's compiled.** Laravel finds things at run time: a method name in a route file, a
column through `__get`, a class through the container. Renox finds them at compile time. A typo
in a handler's name, a column type that doesn't match, a form field you forgot to declare: the
compiler tells you before anything runs. The price is compile time (`rnx serve` rebuilds on
save, usually in a few seconds) and some ceremony: types are written out.

**Types instead of magic.** A model is a struct with one field per column. A form is a struct
with one field per input, so there is no `$fillable` and no mass assignment: a field the struct
doesn't declare can't reach the database. `$casts` become field types (`bool`, `Json<T>`, an
enum, `NaiveDate`, `Encrypted<T>`).

**Extractors instead of the container.** A Laravel controller method asks the container for
`Request $request`, `Post $post`, `Auth::user()`. A Renox handler lists what it needs as
arguments, and each argument type knows how to pull itself out of the request: `Session`,
`AuthUser`, `Valid<StorePost>`, `Found<Post>`, `State<AppState>`. There are no facades either:
the database, cache, queue, mailer and storage are fields of `AppState`.

**No runtime reflection, so no lazy relations.** `$post->author` can't quietly run a query,
because Rust has no `__get`. Renox makes that a feature: a method for one related row, and
*loaders* that fetch the related rows of a whole page in one query. N+1 can't happen by
accident (and `/_renox/debug` flags a statement run three times or more).

**Migrations are SQL files.** No schema builder: `migrations/2026…_create_posts.up.sql` and
`.down.sql`, plus `.postgres.up.sql` when PostgreSQL needs different SQL. You see exactly what
runs, and the same migrations compile into the binary.

**One binary.** `rnx build` makes one file holding the web server, the queue workers, the
scheduler, the CLI (your `artisan`), the migrations, the templates, the translations and the
public files. No PHP-FPM, Nginx config for PHP, Supervisor, cron entry or Redis. SQLite is the
default database; PostgreSQL is a feature flag away.

**htmx and Alpine.js instead of Livewire or Inertia.** Pages are rendered on the server; htmx
swaps parts of them, and handlers return just a block of a template (`.fragment("list")`).
Validation errors land next to the right inputs with no per-form JavaScript.

**MiniJinja instead of Blade.** Jinja2 syntax (`{{ }}`, `{% if %}`, `{% extends %}`,
`{% block %}`), auto-escaped, with Laravel's helpers as functions: `route()`, `old()`,
`error()`, `csrf_field()`, `t()`, `can()`, `asset()`.

**Errors are values.** Handlers return `Result<T>`; `?` passes an error up and Renox turns it
into the right page (a 404 for `find_or_404`, a 403 for `authorize`, a redirect back with
errors for invalid input, a 500 with the error page otherwise). `abort(StatusCode, "…")` is
Laravel's `abort()`.

## Projects and commands

`rnx new shop` makes an app. Where Laravel's files live in Renox:

| Laravel | Renox |
|---|---|
| `composer.json`, `vendor/` | `Cargo.toml` (one dependency: `renox`) |
| `artisan` | `rnx` during development; the app binary itself in production |
| `public/index.php`, `bootstrap/app.php`, service providers | `src/main.rs` (one line) and `src/lib.rs` (`pub fn app() -> App`) |
| `routes/web.php`, `app/Http/Controllers/*` | a module per area: `src/app/products/mod.rs` with its routes and handlers |
| `app/Models/Product.php` | `src/app/products/model.rs` (struct, factory, policy) |
| `database/migrations/*.php` | `migrations/*.up.sql` / `*.down.sql` |
| `resources/views/*.blade.php` | `resources/views/*.html` |
| `lang/es.json` | `resources/lang/es.json` |
| `config/*.php` | `.env`, read once into `Config` |
| `storage/` | `storage/` (the SQLite file, uploads, the maintenance flag) |
| `tests/Feature/*` | `tests/*.rs` with `TestApp` |

The app is a library (`src/lib.rs`) plus a one-line binary, so tests can boot the same app the
binary runs. The binary is also the app's CLI, like `artisan`: Renox's commands and yours are
compiled into it, because migrations, jobs and commands are code. `rnx <command>` runs the same
thing through `cargo run` while you develop.

```bash
rnx serve                         # php artisan serve + Vite's reload: rebuilds and reloads on save
rnx make:module products --resource --fields "name:string price:money active:bool"
                                  # make:model -mfc + resource controller + views + tests
rnx make:model Product --module products --migration
rnx make:migration add_sku_to_products
rnx make:job SendReceipt --module products   # also make:mail, make:notification, make:policy,
                                             # make:command, make:event, make:rule, make:test, …
rnx migrate                       # migrate:rollback, migrate:fresh --seed, migrate:status, db:seed
rnx route:list
rnx db:shell                      # the closest thing to tinker (see "What Renox doesn't have")
rnx queue:failed                  # queue:retry <id|all>, queue:forget, queue:flush
rnx down --secret s3cret          # up
rnx build                         # one release binary in dist/
./dist/shop migrate               # in production: the binary is the CLI
```

## Routes and controllers

There are no controller classes. A module groups an area's routes, and handlers are plain
`async fn`s next to them.

```php
// routes/web.php
Route::get('/products', [ProductController::class, 'index'])->name('products.index');
Route::get('/products/{product}', [ProductController::class, 'show'])->name('products.show');
Route::post('/products', [ProductController::class, 'store'])->name('products.store')
    ->middleware('auth');
```

```rust
use renox::prelude::*;

#[derive(Model, serde::Serialize, Default)]
#[model(table = "products")]
struct Product {
    id: i64,
    name: String,
    price: i64,
}

pub struct Products;

impl Module for Products {
    fn name(&self) -> &'static str {
        "products"
    }

    fn routes(&self) -> Routes {
        let public = Routes::new()
            .get("/products", index).name("products.index")
            .get("/products/{id}", show).name("products.show"); // {id}, not :id
        let members = Routes::new()
            .post("/products", store).name("products.store")
            .require_auth(); // the `auth` middleware, for the routes added before it
        public.merge(members)
    }
}

async fn index(State(db): State<Db>) -> Result<View> {
    let products = Product::query().order_by("name").get(&db).await?;
    Ok(view("products/index.html", context! { products }))
}

// Route model binding: a missing row (or `/products/abc`) is a 404.
async fn show(Found(product): Found<Product>) -> View {
    view("products/show.html", context! { product })
}

#[derive(serde::Deserialize, Validate)]
struct ProductForm {
    #[validate(required, max = 100)]
    name: String,
    #[validate(min = 0)]
    price: i64,
}

async fn store(State(db): State<Db>, Valid(form): Valid<ProductForm>) -> Result<Redirect> {
    let product = Product { name: form.name, price: form.price, ..Default::default() };
    let product = Product::create(&db, product).await?;
    Redirect::route("products.show", &[&product.id]) // redirect()->route(...)
}

pub fn app() -> App {
    App::new().module(Auth::new()).module(Products)
}
```

- `Route::resource` is `.resource("/products", "products", Resource::new().index(…).store(…)…)`
  with the same seven names; `rnx make:module products --resource` writes all of it.
- `Route::prefix('admin')->name('admin.')->group(…)` is `.group("/admin", "admin.", routes)`;
  `Route::domain`, `Route::fallback`, `Route::view` and `Route::redirect` keep their names.
- `$request->input('q')` becomes a typed extractor: `Query<Search>` for the query string,
  `Form<T>` or `Json<T>` for a body (or `Valid<T>` to validate it), `Path<i64>` for a
  parameter, `ClientIp` for `$request->ip()`, `Htmx` for htmx headers.
- A handler returns anything that becomes a response: `View`, `Redirect`, `Json<T>`, a
  `String`, a `StatusCode`, `Download`, or a tuple adding headers, cookies or a `Toast`.

Details: [routing.md](routing.md).

## Middleware and guards

Laravel's named middleware (`auth`, `verified`, `can:`, `throttle`) are methods on `Routes`.
Your own middleware is an `async fn` wrapped with axum's `from_fn`: `App::layer` for every
route (Laravel's global middleware), `Routes::route_layer` for some.

```rust
use renox::prelude::*;
use renox::axum::extract::Request;
use renox::axum::middleware::{Next, from_fn};
use std::time::Duration;

// Laravel's handle($request, Closure $next)
async fn stamp(user: Option<AuthUser>, req: Request, next: Next) -> Response {
    let mut res = next.run(req).await;
    let who = if user.is_some() { "member" } else { "guest" };
    res.headers_mut().insert("x-visitor", who.parse().unwrap());
    res
}

fn routes() -> Routes {
    let orders = Routes::new()
        .get("/orders", || async { "your orders" })
        .require_verified(); // ['auth', 'verified']
    let checkout = Routes::new()
        .post("/checkout", || async { "paid" })
        .require_auth()
        .throttle(10, Duration::from_secs(60)); // throttle:10,1
    let admin = Routes::new()
        .get("/admin", || async { "admin" })
        .require_role("admin")        // role:admin (the Permissions module)
        .route_layer(from_fn(stamp)); // ->middleware(Stamp::class)
    orders.merge(checkout).merge(admin)
}

fn app() -> App {
    App::new().layer(from_fn(stamp)) // every route of the app's modules
}
```

Also `.guest_only()`, `.require_gate("…")` (`can:`), `.require_permission("…")`,
`.require_ability("…")` (Sanctum abilities), `.require_password_confirmed()`
(`password.confirm`), `.without_csrf()` (`VerifyCsrfToken::$except`), `.cors(&[…])`,
`.throttle_by("api")` with `App::rate_limiter` (`RateLimiter::for`). `route:list` shows each
route's guards.

## Requests and validation

A form request is a struct: its fields say what the form sends (and their types), its rules
say what is allowed. `Valid<T>` in a handler's arguments reads the body (form, JSON or
multipart) or the query string, runs the rules, and only calls the handler when everything
passes. A plain form post goes back with errors and old input (never passwords); an htmx or
JSON request gets a 422 with the errors, which the bundled script places next to the inputs.

```php
class StorePost extends FormRequest
{
    public function authorize(): bool { return $this->user()->hasRole('editor'); }

    protected function prepareForValidation(): void
    {
        $this->merge(['title' => trim($this->title)]);
    }

    public function rules(): array
    {
        return [
            'title' => 'required|max:200|unique:posts,title',
            'tags.*' => 'max:30',
            'published_on' => 'nullable|date',
        ];
    }
}
```

```rust
use renox::chrono::NaiveDate;
use renox::prelude::*;
use renox::validation::FormContext;
use serde::Deserialize;

#[derive(Deserialize)]
struct StorePost {
    title: String,
    #[serde(default)]
    tags: Vec<String>,            // tags=a&tags=b; nothing chosen → []
    published_on: Option<NaiveDate>, // nullable|date: the type does it
}

impl Validate for StorePost {
    fn prepare(&mut self) {
        self.title = self.title.trim().to_string();
    }

    async fn authorize(&self, form: &FormContext<'_>) -> Result<bool> {
        Ok(form.user.is_some_and(|u| u.has_role("editor"))) // false → 403
    }

    fn rules(&self, v: &mut Validator) {
        v.field("title", &self.title).required().max(200).unique("posts", "title");
        v.each("tags", &self.tags, |tag| tag.max(30));
    }
}

async fn store(Valid(post): Valid<StorePost>) -> Result<Redirect> {
    let _ = (post.title, post.tags, post.published_on);
    Ok(Redirect::to("/posts"))
}
```

When a form needs only rules, derive them, as attributes (`#[validate(hooks)]` plus
`impl ValidateHooks` adds `prepare`/`authorize`/`after` to a derived form):

```rust
use renox::prelude::*;

#[derive(serde::Deserialize, Validate)]
struct ContactForm {
    #[validate(required, max = 100, label = "Your name")]
    name: String,
    #[validate(required, email)]
    email: String,
}
```

- `Rule::unique('users')->ignore($id)->where(…)` is
  `.unique("users", "email").ignore(id).where_eq("team_id", team)`; `exists:` is `.exists(…)`.
- `in:`/`not_in:`/`regex:` are `one_of`, `none_of`, `matches`; `messages()` and
  `attributes()` are `.message(…)` and `.label(…)` (or `renox.validation.*` in a lang file).
- `after()`/`withValidator` is `async fn after(&self, form, errors)`, which may query the
  database; `ValidationException::withMessages` is `Err(ValidationError::new(errors).into())`.
- Most of Laravel's rules exist under the same name in snake_case; integer, numeric, boolean,
  array and date are the field's type.

Details: [validation.md](validation.md).

## Eloquent → models

### A model

```php
class Post extends Model
{
    use SoftDeletes;
    protected $fillable = ['title', 'status', 'tags'];
    protected $hidden = ['api_secret'];
    protected $casts = ['status' => Status::class, 'tags' => 'array',
                        'featured' => 'boolean', 'api_secret' => 'encrypted'];
}
```

```rust
use renox::db::{Encrypted, Json};
use renox::prelude::*;

#[derive(DbEnum, Debug, Clone, Copy, PartialEq, Default)]
enum Status {
    #[default]
    Draft,     // stored as the text "draft"
    Published,
}

#[derive(Model, serde::Serialize, Default, Clone)]
#[model(table = "posts", soft_deletes)]
struct Post {
    id: i64,                  // the key is always `id`; 0 = not saved yet
    user_id: i64,
    title: String,
    status: Status,           // an enum cast
    tags: Json<Vec<String>>,  // 'array' (TEXT on SQLite, JSONB on PostgreSQL)
    featured: bool,           // 'boolean'
    #[serde(skip)]            // $hidden: never reaches a template or JSON
    api_secret: Encrypted<String>, // 'encrypted': sealed with APP_KEY on save
    created_at: Option<DateTime>,  // filled on save, like Laravel's timestamps
    updated_at: Option<DateTime>,
    deleted_at: Option<DateTime>,  // soft deletes
}
```

The struct is the whole definition: there is nothing to keep in sync except the migration.
Table names are not pluralised: say `table = "posts"` (without it, the struct `Post` maps to
a table `post`). The key's type is the `id` field's type: `i64`, `Ulid` (`HasUlids`), `Uuid`
(`HasUuids`, the `uuid` feature) or `String`.

Accessors are methods (`fn excerpt(&self) -> String`); a mutator is the `saving` hook (below)
or plain code before `save`. Values a template needs that aren't columns go in a struct made
for the view (with `#[serde(flatten)]` for the model), or a `#[model(skip)]` field.

### Queries

The query builder reads like Eloquent's. Every terminal method takes the database (or a
transaction) explicitly, since there is no global connection.

```rust
use renox::db::Query;
use renox::prelude::*;
# #[derive(DbEnum, Debug, Clone, Copy, PartialEq, Default)]
# enum Status { #[default] Draft, Published }
# #[derive(Model, serde::Serialize, Default, Clone)]
# #[model(table = "posts", soft_deletes)]
# struct Post {
#     id: i64, user_id: i64, title: String, status: Status, views: i64,
#     created_at: Option<DateTime>, updated_at: Option<DateTime>, deleted_at: Option<DateTime>,
# }

impl Post {
    /// A local scope (scopePublished) is a function returning a query.
    fn published() -> Query<Post> {
        Post::where_eq("status", Status::Published)
    }
}

async fn queries(db: &Db) -> Result {
    let latest = Post::published().latest().limit(10).get(db).await?; // ->latest()->take(10)->get()
    let post = Post::find_or_404(db, 1).await?;                        // findOrFail → 404 page
    let popular = Post::where_eq("user_id", 7).where_op("views", ">", 100).count(db).await?;
    let titles: Vec<String> = Post::published().pluck(db, "title").await?;
    let page = Post::published().paginate(db, 1, 20).await?;          // ->paginate(20)
    Post::where_eq("user_id", 7).update(db, &[("status", &Status::Draft)]).await?; // mass update
    let mut post = post;
    post.title = "New title".into();
    post.save(db).await?;   // UPDATE, sets updated_at
    post.delete(db).await?; // soft delete; Post::query().with_trashed() / only_trashed()
    let _ = (latest, popular, titles, page);
    Ok(())
}
```

Global scopes (tenancy) are `#[model(default_scope = "team_only")]`, a function that gets every
query of the model first (`unscoped()` skips it). Raw SQL is `renox::db::sql("… ? …").bind(v)`,
read with `fetch_all`, `scalar` or `fetch_as::<T>()` into a `#[derive(FromRow)]` struct.
Transactions are `db.begin()` / `tx.commit()` or `db.transaction(…)`, with savepoints and
retries; `DB::listen` is `renox::db::capture_queries`.

### Relations

```php
$posts = Post::with(['author', 'comments'])->latest()->paginate(20);
```

```rust
use renox::db::relations::{belongs_to, has_many};
use renox::prelude::*;

#[derive(Model, serde::Serialize, Default, Clone)]
#[model(table = "authors")]
struct Author { id: i64, name: String }

#[derive(Model, serde::Serialize, Default, Clone)]
#[model(table = "posts")]
struct Post { id: i64, author_id: i64, title: String }

#[derive(Model, serde::Serialize, Default, Clone)]
#[model(table = "comments")]
struct Comment { id: i64, post_id: i64, body: String }

impl Post {
    /// `$post->author`, for one post: one visible query.
    async fn author(&self, db: &Db) -> Result<Option<Author>> {
        Author::find(db, self.author_id).await
    }
}

/// What the template gets for each post.
#[derive(serde::Serialize)]
struct PostCard {
    #[serde(flatten)]
    post: Post,
    author: Option<Author>,
    comments: Vec<Comment>,
}

/// `with(['author', 'comments'])`: one query per relation for the whole page.
async fn index(State(db): State<Db>, Page(page): Page) -> Result<View> {
    let posts = Post::query().order_by_desc("id").paginate(&db, page, 20).await?;
    let mut authors = belongs_to::<Author, _, _>(&db, &posts.items, |p| p.author_id).await?;
    let mut comments = has_many(&db, &posts.items, Comment::query(), "post_id", |c| c.post_id).await?;
    let cards = posts.map(|post| PostCard {
        author: authors.remove(&post.author_id),
        comments: comments.remove(&post.id).unwrap_or_default(),
        post,
    });
    Ok(view("posts/index.html", context! { posts => cards }))
}
```

It is more typing than `with()`, and that is the point: the queries are where you can see
them. Many-to-many is a `Pivot` (`attach`, `detach`, `sync`, `toggle`, pivot columns),
`withCount`/`withSum` are `count_many`/`sum_many`, `whereHas` is `where_has`, `morphTo`/
`morphMany` are `Morph`, `hasManyThrough` is `has_many_through`. Details:
[relations.md](relations.md).

### Model events, factories and seeders

`creating`/`saved`/`deleting` observers are `#[model(hooks)]` plus `impl ModelHooks`
(`saving`, `saved`, `deleting`, `deleted`). Like Laravel's, they run for single-model calls
(`save`, `create`, `delete`) and not for bulk query updates.

A factory is `impl Factory` on the model; states are plain functions; seeders are closures.

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

fn premium(p: &mut Product) {
    p.price = 250_000; // Laravel's ->state([...])
}

async fn in_a_test(db: &Db) -> Result {
    let draft = Product::factory().make_one();             // Product::factory()->make()
    let saved = Product::factory().create_one(db).await?;  // ->create()
    let three = Product::factory()
        .count(3)
        .state(premium)
        .sequence(|i, p| p.name = format!("Coffee {}", i + 1))
        .create(db)
        .await?;
    let _ = (draft, saved, three);
    Ok(())
}

fn seeders(app: App) -> App {
    // DatabaseSeeder: `rnx db:seed` or `rnx migrate:fresh --seed`
    app.seeder(|state| async move {
        Product::factory().count(50).create(&state.db).await?;
        Ok(())
    })
}
```

## Blade → MiniJinja

```blade
@extends('layouts.app')
@section('content')
  <h1>{{ $title }}</h1>
  @foreach ($products as $product)
    <a href="{{ route('products.show', $product) }}">{{ $product->name }}</a>
  @empty
    <p>No products.</p>
  @endforeach
  @can('create', App\Models\Product::class) … @endcan
  <form method="POST" action="{{ route('products.store') }}">
    @csrf
    <input name="name" value="{{ old('name') }}">
    @error('name') <p>{{ $message }}</p> @enderror
  </form>
@endsection
```

```html
{% extends "layouts/app.html" %}
{% block content %}
  <h1>{{ title }}</h1>
  {% for product in products %}
    <a href="{{ route('products.show', product.id) }}">{{ product.name }}</a>
  {% else %}
    <p>No products.</p>
  {% endfor %}
  {% if can('create-products') %} … {% endif %}
  <form method="post" action="{{ route('products.store') }}">
    {{ csrf_field() }}
    <input name="name" value="{{ old('name') }}">
    <p class="error" data-error-for="name">{{ error('name') }}</p>
  </form>
{% endblock %}
```

| Blade | MiniJinja in Renox |
|---|---|
| `{{ $x }}`, `{!! $x !!}` | `{{ x }}` (escaped), `{{ x \| safe }}` |
| `@include('partials.x')` | `{% include "partials/x.html" %}` |
| `@auth`, `auth()->user()->name` | `{% if auth.check %}`, `auth.user.name` |
| `@method('PUT')` | `{{ method_field('PUT') }}` |
| `__('shop.welcome', ['name' => $n])` | `t('shop.welcome', name=n)` |
| `@class([...])`, `@push`/`@stack`, `@once` | `class_names(…)`, `{% call push('scripts') %}` / `stack('scripts')`, `once('key')` |
| `@vite(...)` | `{{ renox_head() }}` (htmx, Alpine, renox.js) and `asset('app.css')` (hashed URL) |
| `View::share` / view composers | `App::share(key, \|ctx\| async { … })` |
| `<x-alert type="error">…</x-alert>` | a macro: `{% from "components/alert.html" import alert %}`, `{% call alert("error") %}…{% endcall %}` |

Components are MiniJinja macros (`rnx make:component price_tag`) that still see `old()`,
`error()`, `t()`, `auth` and `csrf_field()`. Renox ships a whole kit of them in
`renox/ui.html` (inputs, selects, date pickers, repeaters, sheets, tables, navigation): what
Breeze's Blade components and Filament's forms give you. Views get the data you pass with
`context! { … }`; anything passed must implement `serde::Serialize`. Details: [ui.md](ui.md).

### Livewire and Inertia → htmx

The handler that renders a page can also answer an htmx request with just one block of it:

```rust
use renox::prelude::*;

async fn add_todo(htmx: Htmx) -> Result<Response> {
    if htmx.request {
        // {% block list %} of the page, and a toast.
        let list = view("todos/index.html", context! {}).fragment("list");
        return Ok((Toast::success("Added."), list).into_response());
    }
    Ok(Redirect::to("/todos").into_response())
}
```

`.also("count")` adds out-of-band blocks; `HxRedirect`, `HxRefresh`, `HxTrigger`, `HxRetarget`
are the response headers; `<form data-live-validate>` is Precognition. Alpine.js covers
client-side state. `examples/htmx-recipes` has modals, inline editing, infinite scroll and tabs.

## Auth: Breeze, Sanctum, gates and policies

`Auth::new()` is a module with Breeze's pages built in (login, registration, password reset,
email verification, an account page), on the kit and overridable by file. Sanctum's tokens
are part of it. For what Breeze and Jetstream scaffold around those pages, `rnx new desk
--starter` writes the starter kit: a sidebar layout with the notification bell, a dashboard,
roles (`admin`, `member`), a users page where admins change roles, the activity log, a
`users:admin` command for the first admin, a seeder and the tests for all of it.

```rust
use renox::auth::Permissions;
use renox::prelude::*;

#[derive(Model, serde::Serialize, Default)]
#[model(table = "posts")]
struct Post { id: i64, user_id: i64, title: String }

impl Policy for Post {
    fn allows(&self, user: &User, ability: &str) -> bool {
        match ability {
            "update" | "delete" => self.user_id == user.id || user.has_role("admin"),
            _ => false,
        }
    }
}

pub fn app() -> App {
    App::new()
        .module(Auth::new().account().verify_email()) // Breeze, with profile and verification
        .module(Permissions)                          // spatie/laravel-permission
        .gate("see-reports", |user| user.email.ends_with("@shop.example")) // Gate::define
}

// `AuthUser` is Auth::user(); a guest is sent to the login page (Option<AuthUser> if optional).
async fn edit(user: AuthUser, Found(post): Found<Post>) -> Result<View> {
    user.authorize("update", &post)?; // $this->authorize('update', $post): 403 unless allowed
    Ok(view("posts/edit.html", context! { post }))
}

// Sanctum's createToken with abilities; used as `Authorization: Bearer …`.
async fn issue_token(State(db): State<Db>, user: AuthUser) -> Result<String> {
    Ok(user.create_token_with(&db, "mobile", &["posts:read"], None).await?.plain)
}
```

- `Gate::before` is `App::gate_before`; a gate that needs the database is `gate_async`.
- `@can('update', $post)` needs the answer in the data: wrap rows in `Can::new(post, user,
  &["update"])`, then `{% if can('update', post) %}`.
- Roles and permissions: `user.assign_role(&db, "editor")`, `has_role`, `has_permission`,
  `.require_role(…)`, `.require_permission(…)`.
- `logoutOtherDevices` is `auth::logout_other_devices`; logout ends this device only.
- Users imported from Laravel keep their bcrypt hashes and move to Argon2id at their next login.

Details: [authorization.md](authorization.md).

## Queues, events, mail, notifications and scheduling

All of it runs inside `serve`: `QUEUE_WORKERS` workers (2 by default) and the scheduler start
with the web server, so there is no `queue:work` under Supervisor and no cron entry.
`queue:work` and `schedule:work` exist for running them in their own processes. The queue lives
in the database (`jobs`, `failed_jobs`), on SQLite as on PostgreSQL.

```rust
use renox::prelude::*;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Serialize, Deserialize)]
struct SendReceipt {
    order_id: i64, // ids, not models (SerializesModels isn't a thing here)
}

impl Job for SendReceipt {
    const NAME: &'static str = "send-receipt"; // how a worker finds the type again
    const MAX_ATTEMPTS: u32 = 5;               // $tries

    async fn handle(self, ctx: JobContext) -> Result {
        let mail = ctx.state.mail_view(
            "buyer@example.com",
            "Your receipt",
            "mail/receipt", // resources/views/mail/receipt.html (+ .txt): a Mailable's view
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

pub fn app() -> App {
    App::new()
        .job::<SendReceipt>()
        .listen(|event: OrderPlaced, state| async move {
            // Listeners run in the request; queue the slow part (ShouldQueue).
            state.dispatch(SendReceipt { order_id: event.order_id }).await?;
            Ok(())
        })
        .schedule(|s| {
            // $schedule->call(...)->dailyAt('02:00'), in APP_TIMEZONE
            s.daily_at("02:00", "prune-carts", |state| async move {
                renox::db::sql("DELETE FROM carts WHERE updated_at < ?")
                    .bind(renox::db::now() - renox::chrono::TimeDelta::days(30))
                    .execute(&state.db)
                    .await?;
                Ok(())
            });
        })
}

async fn place(State(state): State<AppState>) -> Result<Redirect> {
    state.emit(OrderPlaced { order_id: 1 }).await?; // event(new OrderPlaced(...))
    let later = SendReceipt { order_id: 1 };
    state.queue.dispatch_after(later, Duration::from_secs(3600)).await?; // ->delay(...)
    Ok(Redirect::to("/orders"))
}
```

Notifications keep Laravel's shape, with `channels` for `via`:

```rust
use renox::auth::{Channel, Notification, Recipient};
use renox::mail::Mail;
use renox::prelude::*;

struct OrderShipped {
    order_id: i64,
}

impl Notification for OrderShipped {
    fn kind(&self) -> &'static str {
        "order-shipped"
    }
    fn channels(&self, _to: &Recipient) -> Vec<Channel> {
        vec![Channel::Mail, Channel::Database]
    }
    fn to_mail(&self, to: &Recipient, _state: &AppState) -> Result<Mail> {
        let subject = format!("Order #{} shipped", self.order_id);
        Ok(Mail::new(to.email().unwrap_or_default(), subject, "On its way."))
    }
    fn to_database(&self, _to: &Recipient, _state: &AppState) -> Result<renox::serde_json::Value> {
        Ok(json!({ "order_id": self.order_id }))
    }
}

async fn ship(State(state): State<AppState>, user: AuthUser) -> Result<StatusCode> {
    // $user->notify(new OrderShipped(7)) on a ShouldQueue notification
    state.notify_later(user.user(), &OrderShipped { order_id: 7 }).await?;
    Ok(StatusCode::NO_CONTENT)
}
```

- `Mail::to()->cc()->bcc()->attach()` is the `Mail` builder (`also_to`, `cc`, `bcc`,
  `reply_to`, `from`, `attach`); `Mail::queue` is `state.queue_mail(mail)`; Markdown mail
  components are `renox/mail/components.html`; mail previews are at `/_renox/mail`.
- `Bus::chain`/`Bus::batch` are `queue.chain()`/`queue.batch(name)`; `ShouldBeUnique`,
  `ShouldBeEncrypted`, `WithoutOverlapping`, `afterCommit` (`dispatch_in(&mut tx, job)`) and
  `dispatchSync` all exist; Horizon is `.module(renox::queue::Dashboard)`.
- The scheduler has `cron`, `hourly`, `weekly_on`, `monthly_on`, `weekdays`, `between`,
  `timezone`, `on_failure`, `then_ping`. Several servers can share one database: each run is
  claimed first, so `onOneServer` is built in.

Details: [queue.md](queue.md), [mail.md](mail.md), [scheduling.md](scheduling.md).

## Cache, storage, sessions, cookies and translations

The facades are fields of `AppState` (`state.cache`, `state.storage`, `state.db`,
`state.queue`, `state.mailer`, `state.http`), and the session and the request's language are
extractors.

```rust
use renox::prelude::*;
use renox::{Cookies, SetCookie};
use std::time::Duration;

async fn misc(State(state): State<AppState>, session: Session, lang: Lang) -> Result<String> {
    // Cache::remember
    let count: i64 = state
        .cache
        .remember("products.count", Duration::from_secs(60), || async {
            renox::db::sql("SELECT COUNT(*) FROM products").scalar(&state.db).await.map_err(Into::into)
        })
        .await?;
    session.put("cart", vec![1, 2, 3])?; // session()->put(...)
    session.flash("status", "Saved.")?;  // session()->flash(...)
    let cart: Option<Vec<i64>> = session.get("cart");
    let _ = cart;
    // __('cart.count', ...) with plural forms
    Ok(lang.choice("cart.count", count, &[]))
}

#[derive(serde::Deserialize)]
struct AvatarForm {
    photo: Upload, // $request->file('photo')
}

impl Validate for AvatarForm {
    fn rules(&self, v: &mut Validator) {
        v.field("photo", &self.photo).required().image().max(2048); // KB, checked by content
    }
}

async fn upload(State(state): State<AppState>, Valid(form): Valid<AvatarForm>) -> Result<String> {
    let key = form.photo.store_public(&state.storage, "avatars").await?; // ->store('avatars', 'public')
    Ok(state.storage.url(&key)) // Storage::url; storage_url(key) in templates
}

async fn theme(State(state): State<AppState>, cookies: Cookies) -> (SetCookie, String) {
    let current = cookies.get("theme").unwrap_or_default(); // $request->cookie('theme')
    (SetCookie::new(&state, "theme", "dark"), current)    // Cookie::queue(...)
}
```

- `CACHE_STORE=memory|database` (no Redis: the `database` store shares the cache, locks, rate
  limits and the login lock between servers). `Cache::lock` is `state.cache.lock(name, ttl)`.
- `Storage::disk('s3')`: `STORAGE_DISK=s3` for the default disk, `App::disk(name, …)` plus
  `state.disk_named(name)` for more; `temporaryUrl` is `temporary_url`.
- Sessions are an encrypted cookie by default (no table needed; keep them small);
  `SESSION_DRIVER=database` keeps them in a table.
- Translations are `resources/lang/<locale>.json` with Laravel's `:name` placeholders, `|`
  plurals and `{0}`/`[1,5]` ranges. Renox's own texts are English; your lang files translate
  them too (`renox.validation.*`, `renox.auth.*`, `ui.*`).

## Testing

Feature tests boot the whole app in memory, with a fresh migrated database per test (no
`RefreshDatabase`), and run in parallel. `TestApp` keeps cookies and sends the CSRF token, like
Laravel's test client.

```rust
use renox::prelude::*;
use renox::testing::TestApp;
use std::time::Duration;

#[derive(Clone)]
struct OrderPlaced {
    id: i64,
}
impl Event for OrderPlaced {}

fn app() -> App {
    App::new().module(Auth::new())
}

#[renox::test]
async fn members_log_in() {
    let app = TestApp::new(app()).await;
    let user = User::register(app.db(), "Anna", "anna@example.com", "password123").await.unwrap();

    app.get("/login").await.assert_ok().assert_see("Log in");
    app.post("/login", &[("email", "anna@example.com"), ("password", "password123")])
        .await
        .assert_status(303);
    app.assert_authenticated(Some(&user)); // assertAuthenticatedAs
    app.assert_database_has("users", &[("email", &"anna@example.com")]).await;

    app.fake_events();                       // Event::fake()
    app.travel(Duration::from_secs(3600));   // $this->travel(1)->hours()
    app.assert_not_emitted::<OrderPlaced>(); // Event::assertNotDispatched
}
```

- `actingAs` is `app.acting_as(&user)`; `assertInvalid` is `assert_invalid("field")` for htmx
  and JSON requests (`app.htmx().post(…)`); `assertJsonPath` is `assert_json_path`.
- `Mail::fake()` is the memory mailer (`sent_mail()`, `assert_mail_sent`);
  `Notification::fake()` is `fake_notifications()`; `Http::fake()` is `fake_http()`;
  `Queue::fake()` is `queued_jobs()`, and `run_jobs()` runs them.
- Time travel reaches the queue, sessions and rate limits, as long as your code reads time
  through `renox::db::now()`.

Details: [testing.md](testing.md).

## Configuration and `.env`

`.env` looks like Laravel's (`APP_ENV`, `APP_KEY`, `APP_DEBUG`, `DATABASE_URL`, `MAIL_*`,
`QUEUE_WORKERS`, …) but there is no `config/*.php`. Renox reads `.env` once at boot into a
typed `Config`, and settings with a fixed set of values are enums, not strings. A value it
can't read stops the boot with a message naming the variable, instead of misbehaving later:

```rust
use renox::prelude::*;
use renox::{CacheStore, SessionDriver};

fn describe(config: &Config) -> String {
    let shared_cache = config.cache_store == CacheStore::Database;
    let in_table = config.session_driver == SessionDriver::Database;
    let live = config.env == Environment::Production;
    // Lifetimes are Durations (SESSION_LIFETIME stays in minutes in .env).
    format!("{shared_cache} {in_table} {live} {:?}", config.session_lifetime)
}

fn a_typo_stops_the_boot() {
    let config = Config::from_vars(|name| (name == "SESSION_DRIVER").then(|| "redis".to_string()));
    assert!(config.is_err()); // SESSION_DRIVER must be cookie or database
}

async fn pay(State(state): State<AppState>) -> Result<String> {
    // config('services.stripe.key'): your own settings are plain .env lines.
    let key = state.config.var("STRIPE_KEY").unwrap_or_default();
    Ok(format!("{} characters", key.len()))
}
```

`config:cache` has no equivalent and needs none. In tests, `TestApp::with_config(app, |c| …)`
changes the config without touching the environment. The full list is in the cheat-sheet's
"Configuration" section.

## Deployment

Forge, Envoyer, Supervisor and the cron entry are replaced by one binary and systemd:

```bash
rnx build          # dist/shop: server, workers, scheduler, CLI, views, migrations, public files
rnx make:deploy    # Dockerfile, a systemd service and socket, Litestream for SQLite backups
./shop migrate && ./shop     # on the server: migrate, then serve
```

- **Zero-downtime deploys** (Envoyer): the systemd *socket* holds the port while the app
  restarts, so visitors wait for a moment instead of getting "connection refused".
- **Backups:** Litestream streams the SQLite file to S3-compatible storage; on PostgreSQL, your
  provider's backups.
- **Health checks:** `GET /health` (database and queue).
- **Several servers:** PostgreSQL (or one SQLite file on one machine) plus
  `CACHE_STORE=database`; the scheduler claims each run, workers share the queue.
- **Octane** isn't needed: the binary is a long-running server already.

Details: [operations.md](operations.md).

## Filament → the UI kit and the grid

Filament's panels are generated once and then edited, instead of declared at run time:
`rnx make:module orders --resource --fields "…"` writes the model, form, pages and tests on the
kit, and `examples/backoffice` is built the way Filament's demo is.

```php
public static function table(Table $table): Table
{
    return $table->columns([
        TextColumn::make('number')->searchable(),
        TextColumn::make('status')->badge(),
        TextColumn::make('total')->money('IDR')->summarize(Sum::make()),
    ])->defaultSort('total', 'desc');
}
```

```rust
use renox::grid::{Column, Grid, GridRequest, Summary};
use renox::prelude::*;

#[derive(Model, serde::Serialize, Default)]
#[model(table = "orders")]
struct Order { id: i64, number: String, status: String, total: i64 }

fn orders_grid() -> Grid {
    Grid::new("orders")
        .column(Column::text("number", "Order").searchable())
        .column(Column::select("status", "Status", [("new", "New"), ("paid", "Paid")])
            .badges(&[("paid", "success")]))
        .column(Column::money("total", "Total").summary(Summary::Sum))
        .sort_by("-total")
}

async fn index(request: GridRequest) -> Result<View> {
    // Filters, search, sort and page come from the query string.
    let page = orders_grid().page(Order::query(), &request).await?;
    Ok(view("orders/index.html", context! { orders => page }))
}
```

| Filament | Renox |
|---|---|
| Tables | `renox::grid` + `renox/grid.html`: filters, search, bulk and row actions, summaries, groups, exports (CSV, Excel, print), inline editing, reordering ([grid.md](grid.md)) |
| Forms | the kit's fields: `select` (searchable, options from the server), `date_picker`, `file`, `tags_input`, `repeater`, `key_value`, `wizard`, `show_when` |
| Infolists | `infolist`, `entry`, `repeatable` |
| Actions and modals | `action_sheet`, `slide_over`, `confirm`, `icon_button`, keyboard shortcuts |
| Notifications | `Toast` and the `notification_bell` over Server-Sent Events |
| Widgets | `stat`/`stats`, `chart(…)`, `renox::chart::Trend` |
| Panel navigation | `navbar`, `sidebar` + `rx-shell`, `page_header` |

Details: [ui.md](ui.md) and [grid.md](grid.md).

## What Renox doesn't have

Some of these are planned, some are left out on purpose. The parity review keeps the full list.

| Laravel | Status in Renox | Instead |
|---|---|---|
| Redis (cache, queue, sessions) | not planned | the database: `CACHE_STORE=database`, the database queue, `SESSION_DRIVER=database` |
| Broadcasting, Reverb, WebSockets | WebSockets not planned | Server-Sent Events power the notification bell; poll with htmx (`hx-trigger="every 10s"`) or the grid's `.poll(30)` for the rest |
| Schema builder (`Blueprint`) | not planned | SQL migrations; a `.postgres.up.sql` file when the two databases differ |
| Tinker | not planned | `db:shell` for SQL, and your own commands (`App::command`, `typed_command`) for code |
| MySQL, several connections, read/write split | SQLite and PostgreSQL, one `Db` per app | `renox::db::sql` for reports; a second sqlx pool by hand if you must |
| Livewire, Inertia | not planned | htmx + Alpine; for a SPA, a JSON API with tokens (`examples/api`) |
| Fortify 2FA, Socialite | planned as plugins (`renox-2fa`, `renox-oauth`) | none built in yet |
| Scout | open | `where_like` (case-insensitive), the grid's search; FTS5 or `tsvector` through raw SQL |
| Cashier | open | payment pages and verified webhooks (Midtrans, Xendit, Stripe) in `examples/backoffice` and `examples/webhooks` |
| Pennant feature flags | open | a setting or a gate |
| Several guards / user tables | one `users` table | roles from the `Permissions` module |
| Queued listeners | not built in | a listener that dispatches a job |
| Cache tags | missing | key prefixes, or `cache.forget` per key |
| Polymorphic many-to-many | missing | `Morph` covers morphTo/morphMany; a pivot per type |
| Slack and SMS channels, SES/Postmark API mailers | missing | `Channel::Custom` + `App::channel` with `state.http`; every provider offers SMTP |
| `model:prune` (`Prunable`) | partial | a scheduled `delete()` query; built-in `*:prune` commands for Renox's own tables |
| Optional route parameters, `where` constraints | partial | two routes; a value that doesn't parse is already a 404 |
| Dusk | partial | `TestApp::serve` plus a headless Chrome recipe (testing.md) |
| Telescope, Pulse | partial | `/_renox/debug` (last requests, SQL, views), the queue dashboard, `App::report`, JSON logs |
| Vapor (serverless) | not planned | one binary on a VM or in a container |
| An ecosystem of packages | none yet | many common packages are built in: permissions, activity log (`Audit`), webhooks, tenancy (`default_scope`), tables |

And honestly: Renox is pre-1.0, with one maintainer and no community yet. Laravel's maturity,
docs, courses and packages are years ahead. Check that the trade fits your project.

## Gotchas Laravel developers hit

**A route layer covers only the routes added before it.** `.require_auth()`, `.throttle(…)`,
`.route_layer(…)` wrap the routes already in that `Routes` value, not the ones after. A route
added below `.require_auth()` is public. Build a separate `Routes` per guard and `merge` them,
as the examples above do.

**Paths are axum's.** `/products/{id}`, a wildcard is `{*rest}`; `:id` doesn't work. A group's
prefix starts with `/` and doesn't end with one, and the group's index route is `"/"`, never
`""`.

**Handler futures must be `Send`.** axum runs handlers on a multi-threaded runtime. If a
handler won't compile as a route ("the trait `Handler<_, _>` is not implemented", or "`Send`
is not general enough"), look for an iterator or a closure that borrows local data and is
still alive across an `.await`: collect into a `Vec` first, then await. Holding a
`std::sync::MutexGuard` across an `.await` causes the same error.

**No lazy relations.** `post.author` is a field holding an id, not a query. Load related rows
with a method for one row, or a loader for a page of rows (see "Relations").

**Table names aren't pluralised.** `#[model(table = "posts")]`; without it the table is the
struct's name in snake_case. The key column is always `id`.

**Forms send text; your struct decides the type.** An `i64` field receiving `"abc"` is a
validation error ("must be a number"), and an empty number field is "required": make optional
inputs `Option<T>`. A checkbox is a `bool` (unticked sends nothing, read as `false`); a
multi-select or checkbox group is a `Vec<T>` with `#[serde(default)]`. Keep money in integers
(`i64` in the smallest unit), not floats.

**`.env` is read once, strictly.** `APP_ENV=staging`, `SESSION_DRIVER=redis`, a port that
isn't a number, a `throttle_by` without its `rate_limiter`, or an unknown `MAIL_FAILOVER` name
stop the app at boot with the variable's name. That is on purpose: a typo should not reach
production as a silent default. Restart after changing `.env`.

**Closures take the state last.** `listen(|event, state| …)`, `App::command(name, about,
|args, state| …)`, `App::channel(name, |to, message, state| …)`, `on_failure(|err, state| …)`,
`Auth::on_registered(|user, form, state| …)`, `report(|report, state| …)`: what the callback
is about comes first, then the `AppState`.

**Templates are strict while debugging.** With `APP_DEBUG` on, printing a variable that doesn't
exist is an error page showing the template line (`{% if x %}` on a missing one is fine).
Booleans print as `True`/`False`, so test them with `{% if %}`. Anything you pass to a view
must be `serde::Serialize`.

**Without `APP_DEBUG`, views are compiled in.** A release build (or an app without `.env`)
serves the templates, translations and public files embedded at build time: restart (or
rebuild) after editing them.

**Model hooks skip bulk queries.** `saving`/`saved`/`deleting` run for `save`, `create`,
`insert`, `save_only`, `save_changes`, `delete`; not for `Query::update`, `Query::delete`,
`insert_many` or `upsert`. Laravel behaves the same, but it's easy to forget.

**SQLite has one writer.** While a transaction is open, write through `&mut tx`, not `db`, or
the second write waits for the first and fails with "database is locked".

**PostgreSQL is strict about types.** An `i64` field needs a `BIGINT` column (`INTEGER` is
32-bit there), and `SUM(bigint)` is `NUMERIC` (write `CAST(SUM(x) AS BIGINT)`). The query
builder's `sum` does that for you.

**Read the time through Renox.** `renox::db::now()` (not `SystemTime::now()` or
`Utc::now()`), so `TestApp::travel` reaches your code as it reaches Renox's.

**Jobs are serialized.** A job's fields go into the `jobs` table as JSON: keep them small (ids,
not models) and register each job type with `App::job::<T>()` (or `Registry::job` in a
module), or the worker can't rebuild it.

**Listeners run inside `emit`.** They are awaited before `emit` returns, in the request. Put
slow work in a job.

**Sessions live in a cookie by default.** Browsers drop cookies over about 4 KB, and Renox logs
a warning before that. Store ids in the session, not whole objects, or use
`SESSION_DRIVER=database`.

**`hx-boost` gets whole pages.** A boosted link or form is a normal page request for Renox, so
pair `hx-boost` with `hx-select` when only part of the page should change.

**`dd()` is `dbg!`, `Log::info` is `tracing`.** Renox logs through the `tracing` crate; add
`tracing = "0.1"` to your `Cargo.toml` to use `tracing::info!` in your own code.
`LOG_FORMAT=json` and `LOG_FILE` replace Laravel's log channels.

## Where to go next

- [CHEATSHEET.md](../CHEATSHEET.md): one example of every pattern.
- The guides: [routing](routing.md), [validation](validation.md), [relations](relations.md),
  [types](types.md), [authorization](authorization.md), [queue](queue.md), [mail](mail.md),
  [scheduling](scheduling.md), [ui](ui.md), [grid](grid.md), [testing](testing.md),
  [operations](operations.md), [PostgreSQL](postgresql.md).
- The examples ([llms.txt](../llms.txt) says which shows what): `crud` is the reference CRUD
  module, `shop` a whole shop, `backoffice` the Filament-style admin.
