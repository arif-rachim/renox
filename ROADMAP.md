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
- [ ] Server-side sessions (deferred: cookie sessions cover M3 and M4). Revocation on logout
      comes first, as a session version (M13a, W2); a server-side store only if sessions outgrow 4 KB
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
- [ ] Several files in one field (`Vec<Upload>`): forms now use serde_html_form (M12), which reads
      sequences; needs a test and per-file rules

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
`examples/crud`), M10b (the framework gaps the examples exposed, below), M10c (the other
examples, and the gaps they exposed) and M10d (doctests on public APIs).

- [ ] Small, focused examples, one pattern each, every one compiled and tested in CI (an untested
      example goes stale, and a stale example is worse than none):
  - [x] `examples/hello`: routes, views, forms, validation, uploads (exists)
  - [x] `examples/crud`: model, migration, pagination, soft deletes and a trash, policy, flash (M10a)
  - [x] `examples/api`: API tokens (Bearer), JSON validation errors, CORS, rate limit (M10c)
  - [x] `examples/jobs`: events, queued mail, notifications, a scheduled task (M10c)
  - [x] `examples/uploads`: file rules, public files, private files behind expiring links (M10c)
  - [x] `examples/postgres`: one app on PostgreSQL and SQLite, tested on both in CI (M10c)
  - [x] `examples/webhooks`: payment gateway webhooks (M11b)
- [x] Short files, no decorative code, comments only where something isn't obvious; the official way
      only (when there are two ways, show the main one)
- [x] Each example names the generator commands that made its files (`rnx make:model Produk`, …),
      so agents know not to type the boilerplate
- [x] `CHEATSHEET.md`: one page of the most common patterns (commands, app/module/routes, views,
      forms + validation, model + migration + queries, pagination, auth/policies/gates, HTMX, raw
      SQL + transactions, jobs/events/schedule/mail, cache/session/uploads/i18n, tests, `.env`);
      every Rust block is compiled by `cargo test --doc -p renox` (M10a)
- [x] `llms.txt` at the repo root: what each example and guide covers, file by file (M10a)
- [x] Doc comments on public APIs get doctests instead of `ignore`: all 35 examples in renox-core
      compile (renox is a dev-dependency so they're written the way apps write them; setup lines
      hidden with `# `); writing them found a stale one (`Session::prune`) (M10d)
- [x] Apps from `rnx new` ship an `AGENTS.md` (and a `CLAUDE.md` importing it) with the layout,
      the generators, where the cheat-sheet and examples are, and the checks to run (M10a)

Gaps found while writing examples/api, jobs, uploads and postgres (M10c, closed):
- [x] Errors for API clients (`Accept: application/json` or a JSON body) are `{"message": …}` with
      the status, also for errors raised outside the view layer (CSRF's 419)
- [x] JSON bodies report every field's errors at once, like forms (missing fields, wrong types
      with placeholders, rules)
- [x] `User::attempt(db, email, password)` for token endpoints, as slow for unknown emails as for
      wrong passwords
- [x] `TestApp::post_multipart(uri, fields, files)` for testing uploads

Gaps found while writing the first example, to close before 1.0:
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

M11c · SEO and analytics (done):
- [x] `seo(title=…, description=…, image=…, type=…, canonical=…)` in templates (a Rust function,
      since imported macros can't see the page's `app`/`request`): title, description, canonical
      (APP_URL + path, no query), OpenGraph, Twitter cards; `<html lang="{{ app.locale }}">` and a
      `{% block seo %}` in the generated layout; `noindex, nofollow` outside production
- [x] `/robots.txt` generated unless `public/robots.txt` exists (production: allow + the sitemap
      when a route is named `sitemap`; elsewhere disallow); `renox::seo::Sitemap` builder
- [x] Search Console verification meta (`GOOGLE_SITE_VERIFICATION`)
- [x] GA4 (`GA4_MEASUREMENT_ID`) and Tag Manager (`GTM_CONTAINER_ID`) tags in `renox_head()` with
      the CSP nonce, and their hosts added to the CSP; production only
- [x] `renox::analytics::event(&session, name, params)`: delivered in the htmx swap's `HX-Trigger`,
      the page's head, or the next page after a redirect; renox.js calls `gtag('event', …)` and
      pushes to the GTM `dataLayer`. Page views on `hx-boost` need nothing (GA4 enhanced
      measurement counts history changes; GTM has a History Change trigger)
- [x] `ServerEvent` job for GA4's Measurement Protocol (`GA4_API_SECRET`), `GaClientId` from the
      `_ga` cookie; only sent in production
- [x] `renox::serde_json` re-export and `json!` in the prelude (apps from `rnx new` had no
      `serde_json` for event parameters)

### M12 · v0.13: Types from the form to the database (before 1.0)
Asked by the owner after M10: do the examples cover every type in the database, in Renox and in
the UI? They didn't, and a probe of what browsers send showed three inputs that didn't work.

- [x] Checkboxes: `on` (and `1`, `yes`) read as `true`, an unchecked box (nothing sent) as `false`
- [x] `<input type="datetime-local">` without seconds reads as `NaiveDateTime`
- [x] Multi-selects and checkbox groups (repeated names) read into `Vec<T>`: forms are
      deserialized with `serde_html_form` instead of `serde_urlencoded`
- [x] `#[derive(DbEnum)]`: text-backed enums for models, forms, JSON and templates (`ALL`,
      `as_str`, `Display`, `FromStr`, serde, `ToDbValue`, sqlx decoding on every enabled database
      through `__db_text_type!`, chosen when renox-core compiles)
- [x] `renox::db::Json<T>` fields (TEXT on SQLite, JSONB/JSON/TEXT on PostgreSQL) and
      `serde_json::Value` read back (sqlx `json` feature); `DbValue::Json`
- [x] UUID fields with renox's `uuid` feature (BLOB on SQLite, UUID on PostgreSQL); `DbValue::Uuid`
- [x] A field that doesn't parse as an enum gets the enum's first variant as its placeholder, so
      every other field's errors still show
- [x] `examples/fields`: one form with every input type, saved and shown back in the edit form,
      tested on SQLite and PostgreSQL (in the PostgreSQL CI job); [docs/types.md](docs/types.md)
      maps HTML input ↔ Rust type ↔ SQLite ↔ PostgreSQL and is compiled as a doctest
- Decimals: not added. sqlx deliberately has no decimal type on SQLite, so money stays `i64` in
  the smallest unit, as the guide explains

### M13 · v0.14: Hardening from the pre-1.0 audit
The audit in [docs/audit/2026-09-pre-1.0.md](docs/audit/2026-09-pre-1.0.md) probed negative
flows and injected faults into running apps. IDs below refer to it. Each fix lands with its probe
(on the local `probe-web` / `probe-data` branches) moved to main as a passing regression test.

M13a · web security:
- [x] W1 Uploads can't become active content: html/xml/js/php/… extensions are stored as
      `.txt`; `/storage` and `public/` answer with `CSP: sandbox`, `nosniff`, and `attachment`
      for HTML/XML/JavaScript (an SVG stays an image, sandboxed)
- [x] W18 / A5 `TRUSTED_PROXIES` (addresses, CIDR ranges or `*`) and one `ClientIp` extractor
      (X-Forwarded-For / Forwarded, walked from the right) used by throttles, the login lock
      and request logs
- [x] W2 Logging out ends every session of the user (`users.sessions_revoked_at`, checked on
      each request; `auth::logout` is now async and takes the `Db`)
- [x] W3 Only a valid Bearer token skips CSRF; a wrong one answers 401
- [x] W4 Multipart CSRF / `_method` fields read up to `UPLOAD_MAX_SIZE` (with `multer`)
- [x] W5 Old input kept under 2 KB; password and `_` fields never flashed
- [x] W6, W7 Redirects (`Back`, after validation, intended URL) stay on the app's origin
- [x] W8, W9 Login lock per email+IP, per account and per IP; password reset revokes API tokens
- [x] W10–W17 Maintenance cookie as an HMAC with `Secure`; security headers on 413; empty files
      fail type rules; long paths 404; unique races 422; finite floats only; 415 for unknown
      bodies; normalized emails on login and reset
- [x] The web probes are in main as `crates/renox/tests/it/web_security.rs`

M13b · data and background resilience:
- [x] D1, D3, D9 Panics are contained: a job runs in its own task, so a panic is a failed attempt
      and the worker keeps going (a webhook handler's panic marks its call failed); a task panic
      doesn't stop its schedule (a guard clears `running`); a handler panic answers 500 with the
      error page; one listener's panic doesn't skip the others
- [x] D2 Only jobs with attempts left are reserved; a job whose last attempt never finished
      (crash, kill, OOM) goes to `failed_jobs` (checked at most once a minute)
- [x] D4, D22 Timeouts in `.env`: `DATABASE_ACQUIRE_TIMEOUT` (5 s), `DATABASE_STATEMENT_TIMEOUT`
      (30 s, PostgreSQL), `REQUEST_TIMEOUT` (60 s, a 500), `MAIL_TIMEOUT` (10 s, the whole send)
- [x] D5 `DATABASE_URL` accepts only `sqlite:`, `postgres://` and `postgresql://` (the error
      hides the password)
- [x] D6, D7 A job's outcome is written with retries (about 10 s) so a database blip doesn't
      strand it; a job whose `TIMEOUT` is longer than the 15-minute reservation keeps its
      reservation for `TIMEOUT` + 1 min (instead of refusing such jobs)
- [x] D8 `state.queue.dispatch_in(&mut tx, job)`: the job exists only if the transaction
      commits; workers see it within a second. Plain `dispatch` while SQLite's one write
      transaction is open fails with "database is locked" (documented)
- [x] D10, D15, D16, D17 `unique`/`exists` refuse an unknown column on SQLite (checked with
      `pragma_table_info`, instead of turning SQLite's double-quoted-string fallback off, which
      could break apps' own SQL); text input is compared as text on PostgreSQL; `where_in`
      with over 1,000 ints or strings sends one JSON array; `limit`/`offset` clamp to `i64::MAX`
- [x] D11, D12, D13, D26, D27 Migrations: `fresh` on PostgreSQL drops types, sequences,
      functions and materialized views too; runs take turns (an in-process lock, an advisory
      lock on PostgreSQL, `BEGIN IMMEDIATE` plus a re-check per migration on SQLite); rollback
      checks every `down` before undoing anything and forgets unregistered migrations with a
      warning; `-- renox:no-transaction`, `CONCURRENTLY` or an own `BEGIN` run without the
      wrapper; checksums flag edited migrations, and `migrate:status` lists missing ones
- [x] D14 Webhook payloads are stored as bytes (BLOB/BYTEA; `WebhookCall::payload: Vec<u8>`,
      `text()`); event ids over 200 bytes are stored as `sha256:<hash>`
- [x] D18, D21, D23 A unique violation answers 409; `Error::permanent` (payloads that no longer
      decode, invalid mail addresses) skips the retries; notifications write the database row
      before sending mail
- [x] D19, D20, D24, D25, D28, D29, D30 Saturating signed-URL expiry (S3 presigns for at most
      7 days); `DATABASE_POOL_SIZE=0`, a huge `UPLOAD_MAX_SIZE` and a path defined by two
      modules are boot errors; cache: TTL 0 stores nothing, `remember` computes once per key in
      a process, a wrong type is an error (`remember` recomputes), `flush` keeps `renox:` rows;
      scheduler claims last the interval + 1 min and are pruned once a minute; the CLI refuses
      Rust keywords and `renox`, quotes table names, rejects `create__table`, keeps `export
      APP_KEY=`; `assert_database_has` is `Send`; storage errors name the path, and an invalid
      key is a 400
- [x] The data probes are in main as `tests/it/data_resilience.rs` and
      `tests/it/background_resilience.rs`

M13c · keep it that way:
- [x] Every probe passes on main (web, data, background), on SQLite and PostgreSQL: they run in
      the `test` and `test (PostgreSQL)` CI jobs as `tests/it/web_security.rs`,
      `data_resilience.rs` and `background_resilience.rs`
- [x] A chaos job in CI (`chaos (sqlite)`, `chaos (postgres)`): `tests/chaos/run.sh` runs the
      `tests/chaos` app and checks, with time limits, that PostgreSQL stopped, paused (also during
      a request) and restarted gives fast 500s and a 503 `/health`, and that the app and workers
      recover without a restart; that SQLite held locked keeps reads up and strands no job; and
      that panics in handlers, listeners, jobs and scheduled tasks are contained
- [x] [docs/operations.md](docs/operations.md): timeouts, proxies, `/health`, failure behaviour,
      failed jobs and webhook calls, backups, deploys and migrations, maintenance mode, logs

### M14 · v0.15: API freeze
What would be a breaking change after 1.0, settled now (IDs from the audit), in three PRs.

M14a · API foundations:
- [x] A1 `#[non_exhaustive]` on the public structs and enums that will grow (list in
      [docs/stability.md](docs/stability.md)); `Dialect` stays exhaustive on purpose
- [x] A2 [docs/stability.md](docs/stability.md): semver scope and public dependencies. axum,
      tower(-http), minijinja, tokio, serde, chrono and fake are public (Renox's major follows
      theirs); sqlx is not: queries fail with `db::DbError`, and sqlx is reachable only through
      escape hatches (`Db::sqlite()`, `renox::db::sqlx`, …)
- [x] A3 `Error::Status(code, message)` and `abort`, `abort_if`, `abort_unless` (in the prelude)
- [x] A10 `Routes::group(path_prefix, name_prefix, routes)`
- [x] A9 `App::command` / `Registry::command` with `command::Args`, listed in `help`,
      `Kernel::call` for tests; `rnx make:command`; `examples/hello` has `entries:prune`
- [x] `rnx new` pins Renox to the commit `rnx` was built from (`rev = …`, also through
      `cargo install --git`), falling back to `branch = "main"` without git

M14b · extension points:
- [x] A8 `App::templates(|env| …)` for filters, functions and globals, plus built-in `number`
      (locale separators) and `date` (chrono format, `APP_TIMEZONE`) filters; `App::share(key,
      async fn(ViewContext))` for data every view gets (the handler's context wins); typed values
      with `App::provide(value)`, the `Provided<T>` extractor and `state.provided::<T>()`;
      `App::layer(..)` around the app's own routes (after the session and user are loaded)
- [x] A4 The app's own `users` columns are kept on `User` (`user.get::<T>("role")`,
      `user.set(&db, "role", "admin")`, `User::where_eq("role", …)`, `{{ auth.user.role }}`);
      `Auth::registration_rules` and `Auth::on_registered` for extra registration fields (a
      failing hook undoes the sign-up); `App::gate_async` with `auth.gate_async(..).await?`
- [x] Templates are semi-strict while `APP_DEBUG` is on (printing a missing variable fails;
      `if` and `flash.x` don't); the debug error page shows the request, the error chain and the
      template with its line, also when a view fails to render (it used to fall back to a bare page)

M14c · mail and notifications:
- [x] A6 `Mail` with several recipients (`to: Vec<String>`, `.also_to`), `.cc`, `.bcc`,
      `.reply_to`, `.from` (instead of `MAIL_FROM_*`) and `.attach(name, type, bytes)`
      (base64 in the queue); invalid addresses fail permanently; the preview page lists them all;
      `Mail::is_for(address)`
- [x] A7 `Channel::Custom(name)` with `App::channel(name, handler)` and
      `Notification::to_channel`; `Recipient` for users or plain addresses
      (`Recipient::to("mail", …).and("whatsapp", …)`) with `state.notify_to`; `state.notify_later`
      (database row now, each other channel as its own queued job with retries)
- [x] The cheat-sheet, README, docs and examples follow the frozen API (`Notification` methods
      take a `&Recipient`)

### M15 · v0.16: The data layer
The biggest day-to-day gap for developers coming from Laravel (see "Readiness vs Laravel" in the
audit). Explicit, typed, no magic; settled before the 1.0 freeze. Two PRs.

M15a · queries and relations:
- [x] Query builder: `where_any` / `where_all` (nested OR / AND groups), `where_between`,
      `where_not_in`, `when()`, `where_in_query` (a sub-query instead of a join),
      `sum::<i64|f64>` / `avg` / `min` / `max`, `pluck`, bulk `update` (sets `updated_at`),
      `increment`, `first_or_404`, `first_or_create` (race-safe with a unique index), `chunk` (by
      id); `Model::find_many`, `insert_many` and `upsert` (chunked under the bind limit). A
      column-picking `select` isn't offered on model queries (a model needs its columns); joins
      and projections use `fetch_as`
- [x] `sql(..).fetch_as::<T>()` / `fetch_one_as` / `fetch_optional_as` with the new `FromRow`
      trait: `#[derive(FromRow)]` (`rename`, `skip`), models (`Model: FromRow`) and tuples
- [x] Relations as explicit helpers (`db::relations`): `belongs_to`, `has_many` (with the
      children's query for order and filters), `Pivot` with `attach` / `detach` / `sync` / `ids` /
      `load` / `load_for` / `inverse`; each loads a page's related rows in one query;
      [docs/relations.md](docs/relations.md) (compiled as a doctest)

M15b · validation and requests:
- [x] Validation: `matches` (regex, cached), `digits`, `digits_between`, `date`, `before` /
      `after` / `before_or_equal` / `after_or_equal` (text, `NaiveDate`, `NaiveDateTime`,
      `DateTime`), `none_of` (Laravel's `not_in`; `in` is `one_of`), `required_if` /
      `required_unless` / `required_with`, `same` / `different`, `Validator::each` for every
      item of a list (errors on `name.0`, …), `Validator::nested` for a list of structs (errors on
      `name.0.field`), the `Rule` trait with `.apply(&rule)`; English and Indonesian messages
- [x] Requests and responses: `Vec<Upload>` for `<input type="file" multiple>` (checked per file
      with `each`), `Cookies` / `SetCookie` (plain or encrypted with `APP_KEY`, safe defaults),
      `Download` (bytes, a streamed file, a `Storage` key, any stream; `inline()`, but never for
      HTML/XML/JS; UTF-8 file names)
- [x] Around them: `Htmx::redirect(to)` (HX-Redirect or 303); list-item errors (`photos.1`) show at
      the list's input and slot (renox.js) and in `error('photos')`; `examples/uploads` takes several
      photos at once through htmx and shows invoices with `Download`

### M16 · v0.17: Developer experience and trust
Two PRs.

M16a · lighter builds and several servers:
- [x] Lighter builds: reqwest uses rustls with the `ring` provider (as lettre does), so
      `aws-lc-sys` (C, CMake) is no longer built; new default features `fake` (the `renox::fake`
      re-export) and `server-events` (`analytics::ServerEvent` and its HTTP client) can be turned
      off; `rnx new` sets `debug = "line-tables-only"` for dev builds;
      [docs/development.md](docs/development.md) (linker, features, sccache, Docker)
- [x] Cache-busting `asset()`: `/app.css?v=<content hash>` (hashed at boot for embedded files, by
      modified time from disk), and versioned URLs are cached for a year (`immutable`)
- [x] `make:deploy` Dockerfile with dependency caching (cargo-chef)
- [x] With `CACHE_STORE=database`, `Routes::throttle` limits and the login lock are counted in the
      `cache` table (`renox:count:…`), so several servers share them; a "Several servers" section
      in docs/operations.md

M16b · CI and trust:
- [x] CI: `rnx new` → every `make:*` → `cargo build` and `cargo test` on the result (SQLite and
      PostgreSQL, `tests/cli/run.sh`); compile-fail tests for the macros; a feature matrix
      (cargo-hack) including a SQLite-only build; S3 against a real S3 server; MSRV; cargo-deny;
      coverage; a Docker build of `make:deploy`, started and checked on `/health`
- [x] Direct tests for APIs covered only indirectly (session `pull`/`reflash`/`set_lifetime`,
      `HxRedirect`/`HxRefresh`, `Validator::rule`, `fetch_optional`/`bind_all`, schedule
      constructors and offsets, signed URL tampering and expiry, plural 0, `Config::load`)
- [x] SECURITY.md, CONTRIBUTING.md, CHANGELOG.md

Deviations in M16b:
- The macros' compile errors are `compile_fail` doctests, not trybuild: the same check without
  another dev-dependency or `.stderr` snapshots that change with every Rust release.
- S3 runs against SeaweedFS: MinIO no longer publishes Docker images.
- Semver checks (cargo-semver-checks) move to v1.0: before a first release there's no published
  baseline to compare with, and 0.x allows breaking changes anyway.
- The job found two generator gaps and one bug, fixed here: `make:job`/`make:command` now register
  what they create, migrations made in the same second keep their order, and S3 over `http://`
  (a local MinIO/SeaweedFS) failed on every request.

### M17 · v0.18: Examples of real apps
- [ ] `examples/shop`: auth with gates and policies, an admin with search, sort and pagination,
      uploads, checkout in a transaction, cache, mail and notifications, queue, i18n with
      plurals, SEO, and its `make:deploy` output
- [ ] `examples/htmx-recipes`: inline edit (`hx-patch`), infinite scroll, modal forms, delete
      with `HxRefresh`/`HxRedirect`, Alpine dropdown/tabs/modal
- [ ] `examples/relations`: one-to-many and many-to-many with joins and eager loading
- [ ] A README for every example

### v1.0
- [ ] Documentation site built with Renox: a tutorial, a "Laravel → Renox" guide, the API
      reference; a starter kit; the semver stability guarantee
- [ ] cargo-semver-checks in CI against the last release (moved from M16b)
- [ ] Real crates published to crates.io (`renox`, `renox-core`, `renox-macros`, `renox-cli`;
      only 0.0.1 placeholders exist), then crates.io/docs.rs badges and `cargo install renox-cli`
      in the README

## Decisions

- **Crates:** runtime code lives in one crate, `renox-core`, organised in modules and gated by cargo
  features where dependencies are heavy. Separate `renox-http`/`-db`/`-view` crates would all need
  `AppState` and `App` would need all of them, so splitting now only adds indirection. Revisit if
  compile times demand it.
- **Sessions:** stored in an encrypted, signed cookie (AES-256-GCM via `APP_KEY`), so M1 needs no
  database. Keep sessions small (old input is capped in M13a, W5); a server-side store is deferred.
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
- **No REPL:** `rnx db:shell`, and app commands (planned in M14, A9) instead of Tinker.
