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

M6b (next):
- [ ] Storage (local / S3 / R2) and upload helpers, multipart forms (with CSRF) and file rules
- [ ] i18n for app texts (`resources/lang/{en,id}`), `t()` in templates

### M7 · v0.8: CLI and developer experience
- [ ] `make:module`, `make:model`, `make:migration`, `make:job`, `make:mail`, `make:policy`
- [ ] `route:list`, `db:seed`, `db:shell`
- [ ] Browser live reload after `rnx serve` restarts

### M8 · v0.9: Testing and deployment
- [ ] `renox-testing`: `TestApp`, HTTP client, `acting_as(user)`, `assert_see()`, in-memory DB per test, mail/queue fakes
- [ ] `renox build` with views embedded in the binary, Dockerfile and systemd templates, SQLite backups with Litestream

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
- **HTMX validation errors:** returned as 422 JSON and placed by the bundled script, rather than
  re-rendering a form fragment. It works for any form without a per-form partial, and the form
  keeps the user's input, focus and Alpine state.
- **Migrations:** Renox runs its own migrator (table `renox_migrations`) instead of sqlx's, to get
  Laravel-style batches and module-owned migrations. Migrations are compiled into the app, so the
  app binary runs them; `rnx` forwards to it.
- **Models:** values are bound through `DbValue`/`ToDbValue` and rows decoded with sqlx, so the
  derive only needs `renox` as a dependency.
- **Named routes:** implemented in Renox; axum does not provide them.
- **Queue:** Renox's own SQLite queue (tables `jobs` and `failed_jobs`), so no Redis is required.
  apalis was the plan, but its stable SQL backend needs sqlx 0.8 (which can't link next to our 0.9)
  and the 0.9 backend is still a release candidate; a small queue on our own pool also keeps
  dispatch, retries and the `queue:*` commands Laravel-like.
- **Scheduler and workers run inside `serve`** by default, keeping deploys to one process. Running
  several instances of the app would run scheduled tasks on each; set `SCHEDULER=false` on all but
  one (queue workers are safe to run anywhere).
- **Relations:** Rust has no runtime reflection, so there is no full Eloquent. `derive(Model)` covers
  CRUD; relations are explicit methods; complex queries use `sqlx::query!`.
- **Service container:** replaced by typed `AppState` and extractors.
- **No REPL:** `rnx db:shell` and custom CLI commands instead of Tinker.
