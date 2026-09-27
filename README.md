# Renox

**Laravel's productivity, Rust's performance, one binary to deploy.**

[![CI](https://github.com/arif-rachim/renox/actions/workflows/ci.yml/badge.svg)](https://github.com/arif-rachim/renox/actions/workflows/ci.yml) [![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license) [![Status: pre-1.0](https://img.shields.io/badge/status-pre--1.0-orange.svg)](ROADMAP.md) ![Rust: stable, edition 2024](https://img.shields.io/badge/rust-stable%20%C2%B7%202024-dea584.svg)

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

```bash
cargo install --git https://github.com/arif-rachim/renox renox-cli   # installs `rnx`
rnx new shop && cd shop                                             # or: --database postgres
rnx serve                                                           # http://127.0.0.1:3000
```

`rnx serve` rebuilds and restarts on Rust changes, and the browser reloads itself when a view
changes.

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
```

```html
<form hx-post="/entries" hx-target="#entries" hx-swap="outerHTML">  <!-- CSRF sent for you -->
  <input name="name"> <textarea name="message"></textarea> <button>Send</button>
</form>
```

## Feature tour

<details>
<summary><b>Web</b>: routes, sessions, CSRF, security headers</summary>

- Routes are grouped in modules, with names (`route('products.edit', id)` in templates),
  guards (`.require_auth()`, `.guest_only()`, `.require_verified()`) and rate limits
  (`.throttle(60, Duration::from_secs(60))`).
- Encrypted cookie sessions, flash messages and old input come built in. CSRF protection is
  automatic for forms and htmx.
- `_method` spoofing lets plain forms send PUT and DELETE.
- Security headers and a Content-Security-Policy are on by default, and CORS can be enabled per
  route.
- Webhooks from payment gateways and other services (`impl Webhook`, `.webhook::<W>(path)`):
  signature checks (HMAC, Stripe-style), each event stored and processed once in the queue,
  with `webhook:retry` when something failed.
- Maintenance mode (`my-app down --secret …`) and `/health` are included.
</details>

<details>
<summary><b>Views & HTMX</b>: MiniJinja templates, fragments, Alpine.js</summary>

- Layouts, blocks and macros come from MiniJinja. Templates reload on refresh while you develop.
- `.fragment("block")` answers htmx with just one block. `HxTrigger`, `HxRedirect` and `Back`
  cover the rest.
- `{{ pagination(products) }}`, `{{ csrf_field() }}`, `{{ method_field('PUT') }}`, `old()`,
  `error()`, `t()`, and `can('update', product)` for policies are available in every template.
</details>

<details>
<summary><b>Database</b>: SQLite or PostgreSQL, migrations, models</summary>

- Every form field type maps to a Rust type and a column on both databases: checkboxes, selects
  (`#[derive(DbEnum)]`), multi-selects (`Json<Vec<_>>`), dates, times, JSON, UUIDs
  ([docs/types.md](docs/types.md)).
- Plain SQL migrations run in batches (`migrate`, `migrate:rollback`, `migrate:fresh --seed`),
  with per-database files when SQL differs.
- `#[derive(Model)]` gives you `create`, `save`, `delete` (with optional soft deletes), `find_or_404`,
  a query builder, pagination and factories with fake data.
- For anything else there's raw SQL with `?` placeholders and transactions:
  `renox::db::sql("…").bind(x).fetch_all(&db)`.
- SQLite is the default. PostgreSQL is one feature flag away, with the same code
  ([guide](docs/postgresql.md)).
</details>

<details>
<summary><b>Validation, auth & authorization</b></summary>

- `Valid<T>` validates forms, JSON bodies and query strings with rules such as `required`,
  `email`, `between`, `unique`, `exists`, `confirmed`, `image` and `mimes`. Messages come in English
  and Indonesian, or from your own translations.
- `Auth::new()` adds login, registration, logout, remember me, password reset and email
  verification, with Argon2id hashing and login throttling.
- API tokens (`Authorization: Bearer`) serve mobile apps and integrations.
- Policies and gates: `user.authorize("update", &product)?` in handlers, `can(...)` in views.
</details>

<details>
<summary><b>Background work</b>: queue, scheduler, events, mail, notifications</summary>

- The job queue lives in your own database, with retries, backoff and `queue:failed` / `queue:retry`.
  On PostgreSQL, workers on several servers never take the same job.
- The scheduler (`every_minutes(5, …)`, `daily_at("02:00", …)`) runs inside `serve`, and each run
  is claimed once when several servers share the database.
- Events and listeners are included.
- Mail comes from templates, with a text version, SMTP in production and a preview page at
  `/_renox/mail` while developing.
- Notifications go by mail and/or to the database.
</details>

<details>
<summary><b>Files, cache, translations</b></summary>

- Uploads are ordinary form fields, checked by their content. They're stored locally or on S3/R2,
  with signed temporary URLs.
- The cache (`remember`, `put`, `forget`) is kept in memory or in the database.
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
async fn guests_are_sent_to_login() {
    let app = TestApp::new(App::new().module(Auth::new())).await; // fresh DB, fake mail and queue
    app.get("/login").await.assert_ok().assert_see("Log in");
    app.htmx().post("/register", &[("email", "nope")]).await.assert_invalid("email");
}
```

```bash
rnx make:module products                          # also make:model -m, make:policy, make:job, make:command, make:mail
rnx route:list                                    # every route with its name, module and guards
rnx db:shell                                      # SQL prompt, no sqlite3/psql needed
rnx build && rnx make:deploy                      # dist/shop + Dockerfile, systemd, Litestream
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
| Jobs | Queue in your database, run inside the same binary | Redis-backed queue or in-process tasks | Up to you |
| Auth, mail, uploads, i18n | Built in, with pages and translations | Available through starters and modules | Assemble from crates |
| Deploy | One binary with its assets + `.env`; Dockerfile, systemd, Litestream generated | Binary + config | Up to you |

Choose **Loco** if you prefer Rails conventions, SeaORM or a JavaScript front end. Choose
**Axum on its own** if you want to assemble every piece yourself. Choose **Renox** if you want
Laravel's everything-included workflow and HTML over the wire, deployed as a single file.

## Examples

- [`examples/crud`](examples/crud): one resource end to end, with pagination, validation,
  owner-only edit and delete through a policy, soft deletes with a trash, and tests.
- [`examples/api`](examples/api): a JSON API for a mobile app, with tokens, Bearer auth, JSON
  validation errors, CORS and a rate limit.
- [`examples/jobs`](examples/jobs): an event, a queued receipt mail, admin notifications and a
  scheduled daily report.
- [`examples/uploads`](examples/uploads): public photos checked by content, and private invoices
  behind expiring links.
- [`examples/postgres`](examples/postgres): one app, tested on PostgreSQL and SQLite.
- [`examples/webhooks`](examples/webhooks): Midtrans, Xendit and Stripe webhooks marking orders
  paid, each tested with good, forged and repeated calls.
- [`examples/hello`](examples/hello): the guestbook from the GIF, with an HTMX form, a photo upload,
  an event that queues mail, a scheduled task, English and Indonesian, and login.

## Status

Renox is **pre-1.0**: the API may still change between versions, and the crates on crates.io are
placeholders until the first real release, so install from Git as shown above. Everything listed
here is implemented and tested on Linux, macOS and Windows, against SQLite and PostgreSQL. What's
next (hardening from a pre-1.0 audit, then an API freeze and 1.0) is in
[ROADMAP.md](ROADMAP.md). Issues and feedback are welcome.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
