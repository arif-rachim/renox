# Renox

**Laravel's productivity, Rust's performance, one binary to deploy.**

[![CI](https://github.com/arif-rachim/renox/actions/workflows/ci.yml/badge.svg)](https://github.com/arif-rachim/renox/actions/workflows/ci.yml) [![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license) [![Status: pre-1.0](https://img.shields.io/badge/status-pre--1.0-orange.svg)](ROADMAP.md) ![Rust: 1.94 or later](https://img.shields.io/badge/rust-1.94%2B-dea584.svg)

Renox is a batteries-included web framework for Rust: Axum underneath, HTMX and Alpine.js in the
browser, SQLite (or PostgreSQL) for data. Everything a typical app needs comes in one `renox`
dependency.

## Why Renox

- **One binary to deploy.** `rnx build` compiles your views, translations, CSS/JS and migrations
  into a single executable. Copy it and a `.env` to a server and you're done. There's no Node
  build, no Redis and no separate worker process. `rnx make:deploy` writes the Dockerfile, a
  systemd unit and Litestream backups for the SQLite file.
- **Laravel's workflow, in Rust.** Generators, migrations, models and factories, validation, login
  and registration, policies, queues, a scheduler, mail, notifications, cache, file storage
  (local or S3/R2), translations and test helpers are all built in and all fit together.
- **HTMX-first.** Pages are rendered on the server. Validation errors show up next to the fields
  and fragments swap in without writing any JavaScript. htmx and Alpine.js are bundled, and a
  Content-Security-Policy is on by default.

![A guestbook form: invalid input shows errors inline, a valid post appears in the list, all without a page reload](docs/assets/demo.gif)

<sub>The [`examples/hello`](examples/hello) guestbook: Rust validation, errors inline, the list swapped in by htmx, and no page reloads.</sub>

## Quick start

You need Rust 1.94 or later.

```bash
cargo install --locked --git https://github.com/arif-rachim/renox renox-cli   # installs `rnx`
rnx new blog && cd blog                                             # or: --database postgres
rnx serve                                                           # http://127.0.0.1:3000
```

The new app has a home page, login and registration, an account page (profile, password, other
devices), English and Indonesian texts, a test in
`tests/home.rs`, and an `AGENTS.md` for coding agents. With `--database postgres`, create the
`blog` and `blog_test` databases first (or edit `.env`).

`rnx serve` rebuilds and restarts on Rust changes, and the browser reloads itself when a view
changes. The first build compiles every dependency and takes a few minutes; see
[docs/development.md](docs/development.md) for faster builds.

## A taste

A form that validates in Rust, saves a row, and returns just the updated list to htmx:

```rust
use renox::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Model, Serialize, Default)]
#[model(table = "entries")]
struct Entry { id: i64, name: String, message: String }

#[derive(Deserialize)]
struct EntryForm { name: String, message: String }

impl Validate for EntryForm {
    fn rules(&self, v: &mut Validator) {
        v.field("name", &self.name).required().max(50);
        v.field("message", &self.message).required().between(3, 280);
    }
}

// Bad input never gets here: htmx gets a 422 and the errors appear next to the fields.
async fn store(State(db): State<Db>, Valid(form): Valid<EntryForm>) -> Result<View> {
    Entry::create(&db, Entry { name: form.name, message: form.message, ..Default::default() }).await?;
    let entries = Entry::query().latest().get(&db).await?;
    Ok(view("guestbook.html", context! { entries }).fragment("entries"))
}

// Routes live in modules, registered with `App::new().module(Guestbook)`.
struct Guestbook;

impl Module for Guestbook {
    fn name(&self) -> &'static str { "guestbook" }
    fn routes(&self) -> Routes {
        Routes::new().post("/entries", store).name("entries.store")
    }
}
```

```html
<form hx-post="/entries" hx-target="#entries" hx-swap="outerHTML">  <!-- CSRF sent for you -->
  <input name="name"> <textarea name="message"></textarea> <button>Send</button>
</form>
{% block entries %}                                                  {# all htmx gets back #}
<ul id="entries">{% for e in entries %}<li>{{ e.name }}: {{ e.message }}</li>{% endfor %}</ul>
{% endblock %}
```

## Feature tour

<details>
<summary><b>Web</b>: routes, sessions, CSRF, security headers</summary>

- Routes are grouped in modules and in prefixed groups (`Routes::group("/admin", "admin.", …)`),
  with names (`route('products.edit', id)` in templates),
  guards (`.require_auth()`, `.guest_only()`, `.require_verified()`, `.require_gate("admin")`,
  `.require_role(…)`, `.require_permission(…)`, `.require_ability(…)` for API tokens,
  `.require_password_confirmed()`) and rate limits (`.throttle(60, Duration::from_secs(60))`).
- Encrypted cookie sessions, flash messages and old input come built in. CSRF protection is
  automatic for forms and htmx.
- `_method` spoofing lets plain forms send PUT and DELETE.
- Security headers and a Content-Security-Policy are on by default, and CORS can be enabled per
  route.
- Webhooks from payment gateways and other services (`impl Webhook`, `.webhook::<W>(path)`):
  signature checks (HMAC, Stripe-style), each event stored and processed once in the queue,
  with `webhook:retry` when something failed.
- Maintenance mode (`my-app down --secret …`) and `/health` are included.
- Plain and encrypted cookies (`Cookies`, `SetCookie`), downloads and streamed responses
  (`Download`), and `abort(StatusCode::GONE, "…")` for any status with a message.
- Your own middleware (`App::layer`, `Routes::route_layer`), shared services (`App::provide`),
  template filters (`App::templates`), data for every view (`App::share`) and async gates.
</details>

<details>
<summary><b>Views & HTMX</b>: MiniJinja templates, fragments, Alpine.js</summary>

- Layouts, blocks and macros come from MiniJinja. Templates reload on refresh while you develop.
- `.fragment("block")` answers htmx with just one block. `HxTrigger`, `HxRedirect` and `Back`
  cover the rest.
- `{{ csrf_field() }}`, `{{ method_field('PUT') }}`, `old()`, `error()`, `t()` and `route()` work
  in every template, `can('update', product)` on models the handler wrapped with `Can::new`, and
  `pagination(products)` once imported from `renox/pagination.html`.
- `asset('app.css')` adds a content hash (`?v=…`), so assets can be cached for a year.
</details>

<details>
<summary><b>Database</b>: SQLite or PostgreSQL, migrations, models</summary>

- Every form field type maps to a Rust type and a column on both databases: checkboxes, selects
  (`#[derive(DbEnum)]`), multi-selects (`Json<Vec<_>>`), dates, times, JSON, UUIDs
  ([docs/types.md](docs/types.md)).
- Plain SQL migrations run in batches (`migrate`, `migrate:rollback`, `migrate:fresh --seed`),
  with per-database files when SQL differs.
- `#[derive(Model)]` gives you `create`, `save`, `delete` (with optional soft deletes), `find_or_404`,
  a query builder (OR groups, sub-queries, `where_has`, aggregates, `group_by`/`having`, raw
  fragments, row locks, bulk updates, upserts, `update_or_create`, chunks), pagination (numbered,
  simple or by cursor) and factories with fake data.
- Models can save only what changed (`save_changes`, `save_only`), run hooks (`saving`, `saved`,
  `deleting`, `deleted`) and carry a default scope, e.g. the current tenant, that every query
  applies until `unscoped()`.
- Relations are explicit and N+1-free: `belongs_to`, `has_many`, many-to-many pivots (with pivot
  columns) and polymorphic `Morph` load a page's related rows in one query each, and `count_many` /
  `sum_many` give counts and sums per row ([guide](docs/relations.md)); joins read into
  `#[derive(FromRow)]` structs with `fetch_as`.
- For anything else there's raw SQL with `?` placeholders, and transactions that can retry on a
  busy database (`db.transaction_retrying(3, …)`): `renox::db::sql("…").bind(x).fetch_all(&db)`.
- SQLite is the default. PostgreSQL is one feature flag away, with the same code
  ([guide](docs/postgresql.md)).
</details>

<details>
<summary><b>Validation, auth & authorization</b></summary>

- `Valid<T>` validates forms, JSON bodies and query strings with rules such as `required`,
  `required_if`, `email`, `between`, `matches` (regex), `digits`, dates (`before`, `after`),
  `unique`, `exists`, `same`, `image` and `mimes`, per item of a list (`each`, `nested`), and your
  own reusable `Rule`s. Messages come in English
  and Indonesian, or from your own translations.
- `Auth::new()` adds login, registration, logout, remember me, password reset and email
  verification, with Argon2id hashing and login throttling; `.account()` adds a profile page
  (email change with re-verification, password, "log out other devices", delete account).
  Logout ends only this device. Password rules come from `Password::min(12).mixed_case()…`, and
  users imported from Laravel log in with their bcrypt hashes.
- Auth events (`LoggedIn`, `LoginFailed`, `Registered`, …) and an opt-in `Audit` module that
  records them, plus your own entries (`audit::record`).
- API tokens (`Authorization: Bearer`) with abilities and expiry serve mobile apps and
  integrations.
- Policies and gates: `user.authorize("update", &product)?` in handlers, `can(...)` in views,
  `App::gate_before` for super-admins, and an opt-in `Permissions` module with roles and
  permissions (`user.assign_role(db, "editor")`, `.require_role("editor")`).
</details>

<details>
<summary><b>Background work</b>: queue, scheduler, events, mail, notifications</summary>

- The job queue lives in your own database, with retries, backoff and `queue:failed` / `queue:retry`.
  On PostgreSQL, workers on several servers never take the same job. Queues drain in priority
  order (`--queue high,default`); jobs can be unique, encrypted, rate limited or kept from
  overlapping, have a `failed` hook, and run in chains or in batches with progress.
- The scheduler (`every_minutes(5, …)`, `daily_at("02:00", …)`, `cron("30 9 * * 1-5", …)`,
  `weekly_on`, `monthly_on`, with `weekdays()`, `between(…)`, `on_failure(…)`) runs inside
  `serve` in `APP_TIMEZONE` or a task's own IANA zone, daylight saving included, and each run is
  claimed once when several servers share the database.
- Events and listeners are included.
- Mail comes from templates, with a text version, several recipients, cc/bcc, reply-to and
  attachments, SMTP in production and a preview page at `/_renox/mail` while developing.
- Notifications go by mail, to the database and through your own channels (WhatsApp, SMS…), now
  or through the queue, to users or to plain addresses.
</details>

<details>
<summary><b>Files, cache, translations</b></summary>

- Uploads are ordinary form fields, checked by their content. They're stored locally or on S3/R2,
  with signed temporary URLs.
- The cache (`remember`, `put`, `forget`, `add`, `pull`, `increment`) is kept in memory or in the
  database, with atomic locks (`state.cache.lock("stock:42", ttl)`) that hold across servers on
  the database store.
- Each visitor gets their own locale, from `resources/lang/*.json`, with `t()` and plurals.
</details>

<details>
<summary><b>SEO & analytics</b>: meta tags, sitemaps, Search Console, GA4, Tag Manager</summary>

- `{{ seo(title=…, description=…, image=…) }}` writes the title, description, canonical URL,
  OpenGraph and Twitter card tags.
- `robots.txt` is generated for you, and `Sitemap` builds `sitemap.xml` from routes and models.
  Staging servers say `noindex`.
- Search Console verification, GA4 and Tag Manager come from `.env`, CSP-ready with nonces, and
  only in production.
- `analytics::event(&session, "sign_up", …)` reaches `gtag` with the htmx swap, the page or the
  next page. `ServerEvent` sends from the server through the Measurement Protocol, where ad
  blockers can't drop it.
</details>

<details>
<summary><b>Testing & tooling</b></summary>

```rust
use renox::prelude::*;
use renox::testing::TestApp;

#[renox::test]
async fn login_page_and_registration_errors() {
    let app = TestApp::new(App::new().module(Auth::new())).await; // fresh DB, fake mail and queue
    app.get("/login").await.assert_ok().assert_see("Log in");
    app.htmx().post("/register", &[("email", "nope")]).await.assert_invalid("email");
}
```

```bash
rnx make:module products                          # also make:model -m, make:policy, make:job, make:command, make:mail
rnx route:list                                    # every route with its name, module and guards
rnx db:shell                                      # SQL prompt, no sqlite3/psql needed
rnx build && rnx make:deploy                      # dist/blog + Dockerfile, systemd, Litestream
```

In production, timeouts keep a slow database or mail server from holding requests, `/health`
feeds your load balancer, and panics in handlers, jobs and tasks are contained. CI checks this by
stopping, pausing and locking the database under a running app
([running in production](docs/operations.md)).
</details>

<details>
<summary><b>Made for coding agents</b></summary>

- [CHEATSHEET.md](CHEATSHEET.md) has every common pattern in a few lines, and it's compiled in CI,
  so it can't go stale.
- [llms.txt](llms.txt) maps each topic to the one example file that shows it.
- Apps made by `rnx new` include an `AGENTS.md` that tells an assistant how the project works.
</details>

## How it compares

|  | **Renox** | **Loco** | **Axum on its own** |
|---|---|---|---|
| Inspired by | Laravel | Rails | (a library, not a framework) |
| Front end | Server-rendered + htmx + Alpine, bundled, no Node | Server-side templates, or a client-side app built with npm | Up to you |
| Data | Its own light model layer on sqlx; SQLite first, PostgreSQL optional | SeaORM entities | Up to you |
| Jobs | Queue in your database (SQLite or PostgreSQL), run inside the same binary | Queue in Redis, PostgreSQL or SQLite, or in-process tasks | Up to you |
| Auth, mail, uploads, i18n | Built in, with pages and translations | Mailers and storage built in; auth comes with the SaaS starter | Assemble from crates |
| Deploy | One binary with its assets + `.env`; Dockerfile, systemd, Litestream generated | Binary + config; Dockerfile, nginx or AWS Lambda generated | Up to you |
| Maturity | Pre-1.0, installed from Git, one maintainer | Released on crates.io, larger community | Mature, widely used |

Choose **Loco** if you prefer Rails conventions, SeaORM or a JavaScript front end. Choose
**Axum on its own** if you want to assemble every piece yourself. Choose **Renox** if you want
Laravel's everything-included workflow and HTML over the wire, deployed as a single file.

## Examples

- [`examples/shop`](examples/shop): a whole online shop: htmx search, a cart, checkout in one
  transaction that never oversells, queued mail and notifications, an admin for the `admin` role
  with photo uploads and an audit trail, English and Indonesian, and its deploy files. Start here.
- [`examples/htmx-recipes`](examples/htmx-recipes): a modal form, inline edit, infinite scroll,
  delete in place, tabs and a dropdown, with htmx, Alpine and fragment-returning handlers.
- [`examples/relations`](examples/relations): a blog with belongs-to, has-many and many-to-many
  (a pivot with its own columns and `sync`), loaded without N+1 with counts per post, and
  reports with `group_by` and SQL joins.
- [`examples/crud`](examples/crud): one resource end to end, with pagination, validation,
  owner-only edit and delete through a policy, soft deletes with a trash, and tests.
- [`examples/api`](examples/api): a JSON API for a mobile app, with tokens that carry abilities
  and expire, Bearer auth, cursor pagination, JSON validation errors, CORS and a rate limit.
- [`examples/jobs`](examples/jobs): an event, a queued receipt mail, admin notifications, and
  daily and weekly reports scheduled in a time zone, with a failure alert and a lock.
- [`examples/uploads`](examples/uploads): public photos checked by content, and private invoices
  behind expiring links.
- [`examples/fields`](examples/fields): every form input type saved and shown back, on SQLite
  and PostgreSQL ([docs/types.md](docs/types.md)).
- [`examples/postgres`](examples/postgres): one app, tested on PostgreSQL and SQLite.
- [`examples/webhooks`](examples/webhooks): Midtrans, Xendit and Stripe webhooks marking orders
  paid, each tested with good, forged and repeated calls.
- [`examples/hello`](examples/hello): the guestbook from the GIF, with an HTMX form, a photo upload,
  an event that queues mail, a scheduled task, English and Indonesian, login and an account page.

## Coming from Laravel

| Laravel | Renox |
|---|---|
| `php artisan` | `rnx` (`rnx make:model`, `rnx migrate`, `rnx route:list`, …) |
| Blade | MiniJinja templates, with `{% extends %}` and `{% block %}` |
| `routes/web.php`, `Route::prefix()->name()->group()` | `Module::routes`, `Routes::group("/admin", "admin.", …)` |
| Middleware | `.require_auth()`, `.throttle(…)`, `Routes::route_layer`, `App::layer` |
| Eloquent | `#[derive(Model)]` and the query builder; relations are explicit loaders ([docs/relations.md](docs/relations.md)) |
| Form Requests | `Valid<T>` with `impl Validate` |
| Gates and policies | `App::gate`, `impl Policy`, `user.authorize(…)`, `.require_gate(…)` |
| spatie/laravel-permission | the `Permissions` module: `assign_role`, `has_permission`, `.require_role(…)` |
| Global scopes (tenancy) | `#[model(default_scope = "…")]` with `renox::context` |
| Breeze / Sanctum | `Auth::new().account()` (pages included) / API tokens with abilities (`create_token_with`, `.require_ability(…)`) |
| `Cache::lock` | `state.cache.lock(name, ttl)` |
| Queues, mail, notifications, scheduler | `impl Job`, `mail_view`, `impl Notification`, `app.schedule()` |
| `View::share` | `App::share` |
| Tinker | `rnx db:shell` and your own commands (`App::command`) |
| Livewire | htmx and Alpine.js, with handlers that return fragments |

Not planned: runtime-reflected Eloquent-style models, Redis, and a REPL.

## Documentation

- [CHEATSHEET.md](CHEATSHEET.md): one short, compiled example per task.
- Guides: [relations](docs/relations.md), [authorization and tenants](docs/authorization.md),
  [the queue](docs/queue.md), [field types](docs/types.md),
  [PostgreSQL](docs/postgresql.md), [production](docs/operations.md),
  [faster builds](docs/development.md), [stability and versions](docs/stability.md).
- [llms.txt](llms.txt): a map of the docs and examples for coding agents.
- [ROADMAP.md](ROADMAP.md) and [CHANGELOG.md](CHANGELOG.md).

## Status

Renox is **pre-1.0**. After the Laravel parity review
([docs/audit/2026-09-laravel-parity.md](docs/audit/2026-09-laravel-parity.md)), milestones M18
(tenancy, roles, accounts), M19 (query builder and models), M20a (scheduler, locks) and M20b
(queue) are done; M20c (HTTP client, queue dashboard, localized mail) and M21 (views and
developer experience) come next,
then 1.0: a documentation site with a tutorial and a Laravel guide, semver checks, and the first
real release on crates.io (today's crates there are placeholders, so install from Git as above).
Until then the API may still change; breaking changes are listed in [CHANGELOG.md](CHANGELOG.md).

`rnx new` pins your app to the Renox commit your `rnx` was built from
(`renox = { git = …, rev = "…" }`). To upgrade, reinstall `rnx` or move the `rev`, then read the
changelog.

Every change is tested in CI on Linux, macOS and Windows, on SQLite and PostgreSQL, against a
real S3 server, with a chaos test, the minimum Rust version, every Cargo feature on its own, and
a new app made with every generator and built into a Docker image. Optional Cargo features:
`postgres`, `s3`, `uuid` (and `fake`, `server-events`, on by default).

Issues and feedback are welcome: see [CONTRIBUTING.md](CONTRIBUTING.md), and
[SECURITY.md](SECURITY.md) to report a vulnerability.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
