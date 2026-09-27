# Renox Roadmap

Renox aims to be for Rust what Laravel is for PHP: a batteries-included web
framework where a new app works out of the box.

## Principles

1. **Convention over configuration.** Folder layout, naming and defaults are decided for you.
2. **One dependency, every battery.** `renox = "x.y"` and `use renox::prelude::*`. Internally split
   into `renox-*` crates; unused batteries can be switched off with cargo features.
3. **Stand on mature crates.** Renox is the glue, conventions and developer experience on top of
   axum, sqlx, minijinja, apalis, lettre and friends.
4. **HTMX-first.** Response helpers know whether a request wants a full page or a fragment.
5. **Single-binary deploys.** SQLite, templates and htmx/Alpine assets ship inside the binary.
6. **Dogfooded.** Every feature is exercised by an app in `examples/`.

## Application layout

```
my-app/
├── Cargo.toml              # renox = "0.x"
├── .env
├── src/
│   ├── main.rs             # App::new().module(...).run()
│   └── app/
│       └── products/       # one module per folder
│           ├── mod.rs      # impl Module
│           ├── routes.rs
│           ├── handlers.rs
│           ├── model.rs
│           ├── policy.rs
│           └── jobs.rs
├── migrations/
├── resources/
│   ├── views/              # layouts/, components/, products/
│   └── lang/               # id/, en/
├── public/
├── storage/                # app.db, uploads/, logs/
└── tests/
```

## Milestones

### M0 · v0.1: Foundation
- [x] Workspace: `renox` (facade + prelude), `renox-core`, `renox-cli`
- [x] `App` builder and `Module` trait (routes)
- [x] Typed `Config` from `.env` (`APP_NAME`, `APP_ENV`, `APP_DEBUG`, `APP_URL`, `APP_KEY`, `APP_HOST`, `APP_PORT`)
- [x] `Error` type with error pages; details only when `APP_DEBUG` is on
- [x] Request logging with `tracing`, graceful shutdown (Ctrl-C / SIGTERM)
- [x] `examples/hello`

### M1 · v0.2: Web layer
- [x] `Routes` builder with named routes; URL generation with `state.url()` and `route()` in templates
- [x] Sessions in an encrypted cookie (`APP_KEY`), flash messages, old input, flashed errors
- [x] CSRF protection: `X-CSRF-Token` header (sent automatically for HTMX) or `_token` field; 419 Page Expired
- [x] MiniJinja views: layouts, request globals (`app`, `request`, `flash`, `errors`, `old()`, `csrf_field()`, `renox_head()`), reload in dev
- [x] Built-in error page, overridable per status with `errors/{status}.html`
- [x] HTMX helpers: `Htmx` extractor, `view().fragment()`, `HxRedirect`, `HxRefresh`, `HxTrigger`, `Back`
- [x] htmx 2.0.11 and Alpine.js 3.17.4 embedded with cache-forever URLs; `public/` served at the root
- [x] CLI: `rnx new`, `rnx serve` (rebuild and restart on change), `rnx key:generate`
- [ ] Route groups with a shared prefix and name prefix
- [ ] CSRF token in multipart forms (moves to M6 with uploads; the header works today)

### M2 · v0.3: Database
- [x] SQLite pool from `DATABASE_URL` with WAL, foreign keys, a busy timeout; `State(db): State<Db>` in handlers
- [x] Migrations in `migrations/*.up.sql` / `.down.sql`, embedded with `renox::migrations!()`, per app or per module
- [x] Batches like Laravel: `migrate`, `migrate:rollback [--step N]`, `migrate:fresh [--seed]`, `migrate:status`
- [x] The app binary is its own command line (`my-app migrate`); `rnx migrate` forwards to it
- [x] `rnx make:migration`; `rnx serve` migrates before each restart and rebuilds when migrations change
- [x] `#[derive(Model)]`: `find`, `find_or_404`, `all`, `create`, `save`, `delete`, `force_delete`, `restore`, timestamps, soft deletes, skipped fields
- [x] Query builder: `where_eq/op/like/in/null/not_null`, `order_by`, `latest`, `limit`, `offset`, `get`, `first`, `count`, `exists`, bulk `delete`; unknown columns and operators are errors
- [x] Pagination: `Page` extractor, `paginate()`, built-in `renox/pagination.html` macro
- [x] Transactions via `db.begin()`, seeders (`App::seeder`, `db:seed`), factories (`Factory`, `fake`)
- [ ] SQLite session driver (deferred: cookie sessions cover M3 and M4; revisit if sessions outgrow 4 KB)
- [ ] Pagination links that keep other query parameters

### M3 · v0.4: Forms and validation
- [x] `Valid<T>` extractor for forms, JSON bodies and GET query strings
- [x] Regular posts are redirected back with errors and old input flashed (passwords never are)
- [x] HTMX and JSON requests get `422 {"message", "errors"}`; the bundled script shows the errors next
      to the inputs (`data-error-for` slots or inserted `<p class="error">`), sets `aria-invalid`,
      focuses the first invalid input and leaves what the user typed in place
- [x] Rules: `required`, `min`, `max`, `between`, `email`, `url`, `one_of`, `confirmed`, `accepted`,
      custom `rule(bool, msg)`, and database-backed `unique` (with `ignore(id)`) and `exists`
- [x] `label()` and `message()` per field; empty inputs count as missing; wrong types become field errors
- [x] Messages in English and Indonesian (`APP_LOCALE=en|id`); `error('field')` in templates
- [x] Handlers can return their own `ValidationError` / `Errors`
- [ ] `#[derive(Validate)]` with attribute rules, for forms that only need the basics
- [ ] More rules: `regex`, dates, `digits`, file uploads (with M6)

### M4 · v0.5: Authentication and authorization
M4a (done):
- [x] `Auth` module: `users` table, `/login`, `/register`, `/logout`, Argon2id password hashing
- [x] Remember me (session lasts `REMEMBER_LIFETIME`), new CSRF token on login
- [x] Changing a password logs out the user's other sessions; deleted users are logged out
- [x] Login throttling: 5 failures per email and IP per minute; unknown emails take as long as known ones
- [x] `AuthUser` / `Option<AuthUser>` extractors; `Routes::require_auth()` and `guest_only()`;
      guests are sent to `login` and back to the page they wanted (HTMX via `HX-Redirect`, JSON gets 401)
- [x] Policies (`impl Policy for Model`, `auth.authorize("update", &model)?`) and gates
      (`App::gate("admin", |user| ...)`, `auth.gate("admin")?`, `can('admin')` in templates)
- [x] `auth.check` / `auth.user` in templates (the password hash is never serialized)
- [x] Built-in pages in English and Indonesian, overridable per file (`renox/auth/*.html`)
- [x] `rnx new` apps come with `Auth` and a login/logout nav

M4b (done):
- [x] Password reset by email: one-time links valid for 60 minutes, one email per address per
      minute, the same reply for unknown addresses; resetting ends other sessions
- [x] Email verification with signed links (`Auth::new().verify_email()`,
      `Routes::require_verified()`), resend, and `verification.notice`
- [x] Signed URLs for any route (`state.signed_url(..)`, `ValidSignature` extractor)
- [x] `log` and `memory` mail drivers (`MAIL_MAILER`), so both flows work before SMTP arrives in M5
- [x] API tokens: `user.create_token()`, `tokens()`, `revoke_token()`; `Authorization: Bearer id|secret`
      skips CSRF, answers 401 JSON when invalid, can expire, and records `last_used_at`

### M5 · v0.6: Background work
M5a (done):
- [x] Queue in SQLite: `Job` trait (name, queue, attempts, timeout, backoff), `state.dispatch()`,
      `dispatch_after()`, retries, a `failed_jobs` table, abandoned reservations released after 15 minutes
- [x] Workers inside `serve` (`QUEUE_WORKERS`, woken immediately on dispatch) or on their own with
      `queue:work [--queue a,b] [--workers N] [--once]`; running jobs finish on shutdown
- [x] `queue:failed`, `queue:retry <id|all>`, `queue:flush`
- [x] Scheduler defined in code: `every`, `every_minute(s)`, `hourly`, `daily_at("HH:MM")` in
      `APP_TIMEZONE`; overlapping runs are skipped; `schedule:list`, `schedule:work`
- [x] Events and listeners (`Event`, `App::listen`, `state.emit`); queue slow work from a listener
- [x] `Module::register` for a module's jobs, listeners and scheduled tasks
- [x] `rnx` forwards any other command to the app (`rnx queue:work`, `rnx schedule:list`, ...)

M5b (done):
- [x] SMTP driver (lettre with rustls: `tls`, `starttls` or `none`), plus `log` and `memory`
- [x] Mail templates: `state.mail_view(to, subject, "mail/x", ctx)` renders `x.html` and `x.txt`
      (or text made from the HTML), a built-in `renox/mail/layout.html` and `button` macro
- [x] `state.queue_mail(mail)` sends through the queue with retries
- [x] `/_renox/mail` lists recent mail with HTML and text previews while `APP_DEBUG` is on
- [x] Notifications: `Notification` with mail and database channels, `state.notify(&user, &n)`,
      and `user.notifications()`, `unread_notifications()`, `mark_notification_read()`
- [x] Password reset and verification emails use the templates (HTML + text, English and Indonesian)
- [x] HTML escaping leaves `/` alone (like Jinja2), so URLs in pages and mail stay readable

### M6 · v0.7: Infrastructure
M6a (done):
- [x] Cache: `state.cache` with `get`, `put` (TTL), `has`, `remember`, `forget`, `flush`; `memory`
      or `database` store (`CACHE_STORE`, table `cache`); values stored as JSON
- [x] Rate limiting: `Routes::throttle(max, per)` per user or IP; 429 with `Retry-After`,
      `X-RateLimit-Limit` / `X-RateLimit-Remaining` on allowed responses
- [x] Maintenance mode: `down [--secret S] [--retry N]` / `up`; 503 through the error page
      (`errors/503.html` overrides it); `/S` sets a bypass cookie; state in `STORAGE_PATH`
- [x] `GET /health`: database ping, queue counts, maintenance flag; 503 when the database is down;
      unaffected by maintenance mode and sessions

M6b (done):
- [x] `Upload` form fields (`Option<Upload>`) through `Valid<T>` for `multipart/form-data`
- [x] File rules: `image()` (checked from the content, not the name), `mimes(&[..])`, `min`/`max`/
      `between` in kilobytes; "must be a file" for text sent to a file field; empty file inputs are
      missing
- [x] `state.storage`: `put`, `get`, `exists`, `delete`, `url` (public keys under `public/`),
      `temporary_url` (signed for the local disk, presigned for S3); keys can't escape the root
- [x] `upload.store(..)` / `store_public(..)` with random names and extensions from the content
- [x] Local disk (`STORAGE_PATH/app`, public files at `/storage/...` without sessions) and
      S3/R2/MinIO behind the `s3` cargo feature (object_store)
- [x] CSRF `_token` read from multipart forms; `UPLOAD_MAX_SIZE` body limit (413 above it)
- [x] `storage_url(key)` in templates; the guestbook takes an optional photo
- [ ] Several files in one field (`Vec<Upload>`): serde_urlencoded has no sequences

M6c (done):
- [x] `resources/lang/{locale}.json` (`LANG_PATH`), nested or flat; reloaded on change in debug;
      invalid JSON fails at boot
- [x] `t('key', name='…', count=n)` in templates and `Lang` (`lang.t`, `lang.choice`) in handlers;
      `:name`/`:Name` placeholders, `one|many` plurals, fallback to `APP_FALLBACK_LOCALE`, then the key
- [x] Per-visitor language: `i18n::set_locale(&session, "en")`, else `APP_LOCALE`; `app.locale` follows it
- [x] Built-in texts are translatable from the same files: `renox.validation.*` messages,
      `renox.validation.attributes.*` field names, `renox.auth.*` page texts — so languages
      beyond en/id work without changing Renox
- [x] Validation messages and auth pages follow the visitor's language
- [x] The guestbook and `rnx new` apps ship `en.json` and `id.json`; the guestbook has an ID | EN switch
- [ ] Choosing the language from `Accept-Language` (opt-in)

### M7 · v0.8: CLI and developer experience
- [x] Generators: `rnx make:module` (routes, view, `pub mod` and `.module(..)` in main.rs),
      `make:model [-m]`, `make:migration`, `make:job`, `make:policy`, `make:mail`; they never
      overwrite files
- [x] `route:list`: method, path, name, module and guards (`auth`, `guest`, `throttle:…`) of every
      route, the framework's included
- [x] `db:shell`: a built-in SQL prompt (`.tables`, `.quit`, piped input), no `sqlite3` needed
- [x] `db:seed`, `key:generate` (earlier milestones)
- [x] Browser live reload while developing locally: views, public and lang files trigger a reload
      over `/_renox/live` (SSE); a restart by `rnx serve` reloads after reconnecting; open streams
      end on shutdown so it stays graceful

### M8 · v0.9: Testing and deployment
M8a (done):
- [x] `renox::testing::TestApp` (a module, not a separate crate: it needs the kernel's internals):
      boots the app with an in-memory, migrated database, the memory mailer, no workers, a temporary
      storage directory; keeps the session cookie and sends CSRF tokens itself
- [x] Requests: `get`, `post`, `put`, `patch`, `delete`, `post_json`, `htmx()`, `json()`,
      `header()`, `without_csrf()`; `acting_as(&user)`, `logout()`
- [x] Assertions: `assert_ok/status/redirect/hx_redirect/not_found/forbidden/unauthorized`,
      `assert_see/dont_see`, `assert_header`, `assert_invalid(field)`, `json()`;
      `assert_database_has/missing/count`, `queued_jobs()`, `run_jobs()`, `assert_mail_sent()`
- [x] `#[renox::test]` (Tokio test through Renox's re-export, so apps don't depend on tokio)
- [x] Apps are a library plus a tiny `main.rs` (`rnx new` writes `src/lib.rs` with `pub fn app()`
      and `tests/home.rs`), because tests can't import from `main.rs`; the guestbook has tests too

M8b (done):
- [x] `renox::embedded!()` + `App::embed(..)`: `resources/views`, `resources/lang` and `public/` are
      compiled into the binary and used when `APP_DEBUG` is off; debug builds keep reading the disk
      (live reload). A release build runs from one file plus `.env`
- [x] `rnx build`: release build copied to `dist/<name>`
- [x] `rnx make:deploy`: a multi-stage `Dockerfile` (+ `.dockerignore`), a systemd unit that migrates
      before starting and stops gracefully, a Litestream config for continuous SQLite backups to
      S3/R2, and `deploy/README.md` with the steps
- [x] `rnx new` apps embed their files and ignore `dist/`

### M9 · v0.10: PostgreSQL (before 1.0)
SQLite stays the default and is right for an app on one server. PostgreSQL is for apps that
outgrow that: several app servers, heavy concurrent writes, or a managed database. It has to land
before 1.0 because `Db` was a plain `SqlitePool` alias; turning it into a type that can hold
either pool after 1.0 would break every app. Split in two PRs: M9a (Renox's own database layer,
still SQLite underneath) and M9b (the PostgreSQL backend proper).

- [x] `Db` becomes Renox's own type (holding a SQLite or a PostgreSQL pool), chosen at boot from
      the `DATABASE_URL` scheme (`sqlite://…` or `postgres://…`); `State(db): State<Db>` stays the
      same, with `db.sqlite()` / `db.postgres()` for raw sqlx queries (M9a)
- [x] `Transaction` (`db.begin()`, pass `&mut tx` where `&db` goes), `Row` (`try_get` by name or
      position) and raw SQL with `renox::db::sql("… ?").bind(v).fetch_all(&db)` / `scalar` /
      `execute`; models, queries and all framework code go through them (M9a)
- [x] PostgreSQL behind a `postgres` cargo feature, like `s3`, so SQLite-only apps don't compile it
      (M9a: the feature, the pool and the connection; M9b makes the framework's own SQL work on it)
- [x] `?` placeholders rewritten to `$1`, `$2`, … on PostgreSQL, skipping quotes and comments (M9a)
- [x] `DbValue` binding for PostgreSQL's stricter types: untyped `NULL`, `BOOLEAN`, `TIMESTAMPTZ`,
      `TIMESTAMP`, `DATE`, `TIME` (new `DbValue` variants, `#[non_exhaustive]`); `OFFSET` without
      `LIMIT`; `like` sent as `ILIKE` so it ignores case on both (M9b)
- [x] Framework migrations in both dialects: `BIGINT GENERATED BY DEFAULT AS IDENTITY`,
      `TIMESTAMPTZ`, `BIGINT` counters; emails stored lowercased with a unique index on
      `lower(email)` (chosen over `citext`: a `citext` column compared with a text parameter is
      case-sensitive again) (M9b)
- [x] App migrations per dialect when SQL differs: `migrations/*.up.sql` for both, with optional
      `*.sqlite.up.sql` / `*.postgres.up.sql` overrides picked by `migrations!()` at run time (M9b)
- [x] `migrate:fresh` and `db:shell` (`.tables`, cell display, banner) handle both (M9a)
- [x] No SQLite-only statements left in the framework's own SQL (`ON CONFLICT … DO UPDATE/NOTHING`
      and `RETURNING` work on both) (M9b)
- [x] Queue workers on PostgreSQL reserve jobs with `FOR UPDATE SKIP LOCKED`, so workers on
      several servers never take the same job (M9b)
- [x] Scheduled runs are claimed in the `cache` table (insert-if-absent per task and time slot),
      so several app servers can run `serve` without running a task twice; works on SQLite too and
      lifts the `SCHEDULER=false` workaround. Chosen over advisory locks, which would need a
      held connection per run and still let a slower server run the same slot after the first
      finished (M9b)
- [x] `/health`, `db:shell`, `route:list` and the `rnx` generators work with both (M9b;
      `make:migration` writes SQL for the app's `DATABASE_URL`)
- [x] `rnx new --database postgres`; `.env` templates document `DATABASE_URL` and
      `TEST_DATABASE_URL` (M9b)
- [x] CI runs the whole test suite against SQLite and against PostgreSQL (a service container);
      `TEST_DATABASE_URL` gives every in-memory test app a fresh PostgreSQL schema (M9b)
- [x] A guide: [docs/postgresql.md](docs/postgresql.md): new apps, switching, portable SQL,
      migrations, tests, moving the data with pgloader, deploying (M9b)

### M10 · v0.11: Examples and docs for people and coding agents (before 1.0)
An agent (or a person) building on Renox should be able to find the idiomatic way to do something by
opening one short file, not by reading the framework. That saves tokens and stops agents copying
outdated APIs. Split in PRs: M10a (cheat-sheet, llms.txt, agent files in new apps,
`examples/crud`), M10b (the framework gaps the examples exposed, below) and M10c (the other
examples, doctests on public APIs).

- [ ] Small, focused examples, one pattern each, every one compiled and tested in CI (an untested
      example goes stale, and a stale example is worse than none):
  - [x] `examples/hello`: routes, views, forms, validation, uploads (exists)
  - [x] `examples/crud`: model, migration, pagination, soft deletes and a trash, policy, flash (M10a)
  - [ ] `examples/api`: auth plus API tokens (Bearer)
  - [ ] `examples/jobs`: queue, jobs, scheduler, mail, notifications
  - [ ] `examples/uploads`: file rules, storage (local and S3)
  - [ ] `examples/postgres`: the same app on PostgreSQL (after M9b)
- [x] Short files, no decorative code, comments only where something isn't obvious; the official way
      only (when there are two ways, show the main one)
- [x] Each example names the generator commands that made its files (`rnx make:model Produk`, …),
      so agents know not to type the boilerplate
- [x] `CHEATSHEET.md`: one page of the most common patterns (commands, app/module/routes, views,
      forms + validation, model + migration + queries, pagination, auth/policies/gates, HTMX, raw
      SQL + transactions, jobs/events/schedule/mail, cache/session/uploads/i18n, tests, `.env`);
      every Rust block is compiled by `cargo test --doc -p renox` (M10a)
- [x] `llms.txt` at the repo root: what each example and guide covers, file by file (M10a)
- [ ] Doc comments on public APIs get small runnable doctests instead of `ignore` where possible
- [x] Apps from `rnx new` ship an `AGENTS.md` (and a `CLAUDE.md` importing it) with the layout,
      the generators, where the cheat-sheet and examples are, and the checks to run (M10a)

Gaps found while writing the examples, to close before 1.0:
- [x] Method spoofing (`_method=PUT|PATCH|DELETE` in plain HTML forms, urlencoded or multipart, or
      the `X-HTTP-Method-Override` header), with `{{ method_field('PUT') }}`; a layer in front of
      the whole router, since route layers run after the method is matched (M10b)
- [x] When a field fails to parse (e.g. `price=abc` for an `i64`), it gets its error and a
      placeholder (`0`, then `false`) so the rest parses and every other field's rules still run;
      a placeholder's own rule errors are dropped (M10b; urlencoded and multipart forms — JSON
      bodies still report the first parse error)
- [x] `can('update', product)` in templates for policies: the handler wraps models in
      `auth::Can::new(model, user, &["update", …])` (`Paginated::map` for pages), which adds `_can`
      (M10b)
- [x] Pagination links keep the other query parameters: `page_url(n)` in templates reads
      `request.query` (M10b)

### M11 · v0.12: Web essentials (before 1.0)
What most real apps need on day one besides pages and a database: safe defaults in the browser,
payment gateway callbacks, and being found and measured. Owner's request, done before M10c so the
remaining examples use the final APIs. Order: M11a → M11b → M11c → M10c → v1.0.

M11a · security (done):
- [x] Security headers on every response: `X-Content-Type-Options: nosniff`,
      `Referrer-Policy: strict-origin-when-cross-origin`, `X-Frame-Options: SAMEORIGIN`, and HSTS in
      production over https; a header the handler set is kept
- [x] Content-Security-Policy, `CSP=relaxed|strict|off`. **relaxed is the default** (owner's
      choice): this site's scripts, inline scripts and `eval` (Alpine's standard build) are allowed;
      other sites' scripts, framing by other sites and plugins are not. `strict` allows scripts only
      from this site or with the request's nonce (`csp_nonce()` in templates), switches to Alpine's
      CSP build (vendored) and turns off htmx's `eval`; Alpine expressions must then be simple
      (move statements into `Alpine.data(...)`)
- [x] `App::csp(|csp| { csp.allow("script-src", "https://…"); })` adds sources; a new directive keeps
      `'self'`
- [x] `Routes::cors(&["https://app.example.com"])` (or `"*"`) answers preflights and adds the
      CORS headers for the routes added so far; `cors_layer(renox::cors::CorsLayer)` for anything
      else; `route:list` shows `cors`
- [x] `Routes::without_csrf()` for callers without a session (webhooks); `route:list` shows `no-csrf`

M11b · webhooks (done):
- [x] `impl Webhook` (`PROVIDER`, `verify`, `event_id`, `handle`) + `Routes::webhook::<W>(path)` +
      `app.webhook::<W>()` (boot fails if a webhook route's provider isn't registered)
- [x] `renox::webhook` helpers: `sha256_hex`, `sha512_hex`, `hmac_sha256_hex`, `hmac_sha512_hex`,
      `verify_hmac_sha256` (`sha256=` prefix, any hex case), `verify_timestamped` (Stripe-style
      `t=…,v1=…` with a tolerance against replays), `same` (constant time), `ensure`, `secret`
- [x] A `webhook_calls` table (both databases), unique per (provider, event id): a call is stored
      and its processing queued in one transaction, answered 200 at once; duplicates are answered
      200 and not processed again; forged calls 401, calls without an event id 400
- [x] Processing in the queue (`renox:webhook`, 5 attempts, backoff): status `processed` or
      `failed` with the error; `webhook:failed`, `webhook:retry <id>`, `webhook::retry(state, id)`
- [x] Webhook routes skip CSRF, keep working in maintenance mode, and show in `route:list`
- [x] `Config::var(name)` / `config.vars` for secrets (tests set them without touching the
      process environment); `TestApp::post_body` / `TestRequest::post_body` for exact bytes
- [x] `examples/webhooks`: Midtrans (`signature_key` SHA-512), Xendit (`x-callback-token`), Stripe
      (`Stripe-Signature`), each tested with good, forged and repeated calls

M11c · SEO and analytics:
- [ ] A `seo()` macro: title, description, canonical, OpenGraph and Twitter cards; `<html lang>` from
      the locale; `noindex` outside production
- [ ] `robots.txt` and `sitemap.xml` helpers (routes and models)
- [ ] Google Search Console verification meta (`GOOGLE_SITE_VERIFICATION`)
- [ ] GA4 / Google Tag Manager from `.env` in `renox_head()`, with the CSP nonce and CSP sources,
      off in local and testing
- [ ] Analytics events: `HxTrigger` → `gtag('event', …)` in renox.js, page views on `hx-boost`
      navigation, and a job that sends server-side events (GA4 Measurement Protocol)

### v1.0
- [ ] Documentation site built with Renox, starter kit, semver stability guarantee

## Decisions

- **Crates:** runtime code lives in one crate, `renox-core`, organised in modules and gated by cargo
  features where dependencies are heavy. Separate `renox-http`/`-db`/`-view` crates would all need
  `AppState` and `App` would need all of them, so splitting now only adds indirection. Revisit if
  compile times demand it.
- **Sessions:** stored in an encrypted, signed cookie (AES-256-GCM via `APP_KEY`), so M1 needs no
  database. Keep sessions small; a SQLite driver comes with M2.
- **Templates:** MiniJinja (runtime, overridable, reloadable). Askama may be offered later.
- **Auth sessions:** the session stores the user id and a fingerprint of the password hash, so a
  password change ends other sessions without a separate token column. "Remember me" makes the
  (encrypted cookie) session itself longer-lived.
- **Uploads as form fields:** multipart files are swapped for tokens that `Upload`'s `Deserialize`
  resolves from a thread-local during the (synchronous) deserialization, so a plain
  `#[derive(Deserialize)]` struct can hold files and share the validation path with text forms.
- **Storage:** the local disk is Renox's own code; S3 is an opt-in `s3` feature because object_store's
  AWS support pulls in reqwest and aws-lc-rs, which most apps on one server don't need.
- **i18n:** plain JSON files, loaded into memory at boot and reloaded in debug. Renox's own texts
  stay in code for en/id (so apps work without lang files) but every one of them can be overridden
  by key, which is also how other languages are added.
- **Live reload polls file times** (500 ms) instead of using a watcher, avoiding the watcher-event
  pitfalls `rnx serve` hit, and is only compiled into responses when `APP_ENV=local` with debug on.
- **HTMX validation errors:** returned as 422 JSON and placed by the bundled script, rather than
  re-rendering a form fragment. It works for any form without a per-form partial, and the form
  keeps the user's input, focus and Alpine state.
- **Migrations:** Renox runs its own migrator (table `renox_migrations`) instead of sqlx's, to get
  Laravel-style batches and module-owned migrations. Migrations are compiled into the app, so the
  app binary runs them; `rnx` forwards to it.
- **Models:** values are bound through `DbValue`/`ToDbValue` and rows decoded with sqlx, so the
  derive only needs `renox` as a dependency.
- **Databases:** SQLite first (one file, no server, WAL; enough for one server). PostgreSQL (M9) is
  an opt-in `postgres` feature with the same `Db` API; one app uses one database, picked from
  `DATABASE_URL`. sqlx's `Any` driver was not chosen as the plan because
  it supports fewer types (e.g. chrono timestamps) than the typed pools.
- **Named routes:** implemented in Renox; axum does not provide them.
- **Queue:** Renox's own SQLite queue (tables `jobs` and `failed_jobs`), so no Redis is required.
  apalis was the plan, but its stable SQL backend needs sqlx 0.8 (which can't link next to our 0.9)
  and the 0.9 backend is still a release candidate; a small queue on our own pool also keeps
  dispatch, retries and the `queue:*` commands Laravel-like.
- **Scheduler and workers run inside `serve`** by default, keeping deploys to one process. Running
  several instances share the database: each scheduled run is claimed in the `cache` table first,
  so only one instance runs it (since M9b), and queue workers are safe to run anywhere.
- **Relations:** Rust has no runtime reflection, so there is no full Eloquent. `derive(Model)` covers
  CRUD; relations are explicit methods; complex queries use `renox::db::sql()` (portable) or sqlx
  directly through `db.sqlite()` / `db.postgres()` (e.g. for `query!`).
- **Service container:** replaced by typed `AppState` and extractors.
- **No REPL:** `rnx db:shell` and custom CLI commands instead of Tinker.
