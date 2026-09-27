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
- [x] CLI: `renox new`, `renox serve` (rebuild and restart on change), `renox key:generate`
- [ ] Route groups with a shared prefix and name prefix
- [ ] CSRF token in multipart forms (moves to M6 with uploads; the header works today)

### M2 · v0.3: Database
- [x] SQLite pool from `DATABASE_URL` with WAL, foreign keys, a busy timeout; `State(db): State<Db>` in handlers
- [x] Migrations in `migrations/*.up.sql` / `.down.sql`, embedded with `renox::migrations!()`, per app or per module
- [x] Batches like Laravel: `migrate`, `migrate:rollback [--step N]`, `migrate:fresh [--seed]`, `migrate:status`
- [x] The app binary is its own command line (`my-app migrate`); `renox migrate` forwards to it
- [x] `renox make:migration`; `renox serve` migrates before each restart and rebuilds when migrations change
- [x] `#[derive(Model)]`: `find`, `find_or_404`, `all`, `create`, `save`, `delete`, `force_delete`, `restore`, timestamps, soft deletes, skipped fields
- [x] Query builder: `where_eq/op/like/in/null/not_null`, `order_by`, `latest`, `limit`, `offset`, `get`, `first`, `count`, `exists`, bulk `delete`; unknown columns and operators are errors
- [x] Pagination: `Page` extractor, `paginate()`, built-in `renox/pagination.html` macro
- [x] Transactions via `db.begin()`, seeders (`App::seeder`, `db:seed`), factories (`Factory`, `fake`)
- [ ] SQLite session driver (deferred: cookie sessions cover M3 and M4; revisit if sessions outgrow 4 KB)
- [ ] Pagination links that keep other query parameters

### M3 · v0.4: Forms and validation
- [ ] `Valid<Form<T>>` extractor: redirect back with errors + old input, or re-render the form fragment (422) for HTMX
- [ ] Rules including database-backed `unique` and `exists`
- [ ] Messages in Indonesian and English

### M4 · v0.5: Authentication and authorization
- [ ] Register, login, logout, remember me, password reset, email verification, login throttling
- [ ] `AuthUser` extractor, `auth` / `guest` middleware, API tokens
- [ ] Gates and policies (`authorize!(user, "update", &product)`)
- [ ] Overridable auth views

### M5 · v0.6: Background work
- [ ] Queue: `Job` trait, `dispatch()`, delays, retries, failed jobs, `renox queue:work` or in-process worker
- [ ] Scheduler defined in code, `renox schedule:run`
- [ ] Events and listeners (sync or queued)
- [ ] Mail: templates, `log` driver in dev, preview route
- [ ] Notifications: mail and database channels

### M6 · v0.7: Infrastructure
- [ ] Cache (memory / SQLite, `remember()`)
- [ ] Storage (local / S3 / R2) and upload helpers
- [ ] i18n, rate limiting, maintenance mode, `/health`

### M7 · v0.8: CLI and developer experience
- [ ] `make:module`, `make:model`, `make:migration`, `make:job`, `make:mail`, `make:policy`
- [ ] `route:list`, `db:seed`, `db:shell`
- [ ] Browser live reload after `renox serve` restarts

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
- **Migrations:** Renox runs its own migrator (table `renox_migrations`) instead of sqlx's, to get
  Laravel-style batches and module-owned migrations. Migrations are compiled into the app, so the
  app binary runs them; `renox` forwards to it.
- **Models:** values are bound through `DbValue`/`ToDbValue` and rows decoded with sqlx, so the
  derive only needs `renox` as a dependency.
- **Named routes:** implemented in Renox; axum does not provide them.
- **Queue:** apalis with the SQLite backend, so no Redis is required.
- **Relations:** Rust has no runtime reflection, so there is no full Eloquent. `derive(Model)` covers
  CRUD; relations are explicit methods; complex queries use `sqlx::query!`.
- **Service container:** replaced by typed `AppState` and extractors.
- **No REPL:** `renox db:shell` and custom CLI commands instead of Tinker.
