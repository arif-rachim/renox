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
- [ ] Named routes and URL generation (`url_for("products.show", id)`)
- [ ] Middleware stack: sessions (SQLite store), encrypted cookies, CSRF, flash messages, old input
- [ ] MiniJinja views: layouts, components, app templates override built-in ones, reload in dev
- [ ] HTMX helpers: `HxRequest`, `view()` that renders fragments for HTMX requests, `HxRedirect`, `HxTrigger`
- [ ] htmx and Alpine.js embedded with versioned URLs
- [ ] CLI: `renox new`, `renox serve`

### M2 · v0.3: Database
- [ ] sqlx SQLite pool with tuned pragmas (WAL, foreign keys, busy timeout)
- [ ] Per-module migrations ordered by timestamp; `migrate`, `migrate:rollback`, `migrate:fresh`
- [ ] `#[derive(Model)]`: `find`, `find_or_404`, `all`, `create`, `update`, `delete`, timestamps, soft deletes
- [ ] Pagination (HTMX-ready links), transactions, seeders, factories

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
- [ ] `route:list`, `db:seed`, `key:generate`, `db:shell`
- [ ] `serve` with hot reload (templates instantly, Rust code via rebuild)

### M8 · v0.9: Testing and deployment
- [ ] `renox-testing`: `TestApp`, HTTP client, `acting_as(user)`, `assert_see()`, in-memory DB per test, mail/queue fakes
- [ ] `renox build`, Dockerfile and systemd templates, SQLite backups with Litestream

### v1.0
- [ ] Documentation site built with Renox, starter kit, semver stability guarantee

## Decisions

- **Templates:** MiniJinja (runtime, overridable, reloadable). Askama may be offered later.
- **Named routes:** implemented in Renox; axum does not provide them.
- **Queue:** apalis with the SQLite backend, so no Redis is required.
- **Models:** Rust has no runtime reflection, so there is no full Eloquent. `derive(Model)` covers
  CRUD; relations are explicit methods; complex queries use `sqlx::query!`.
- **Service container:** replaced by typed `AppState` and extractors.
- **No REPL:** `renox db:shell` and custom CLI commands instead of Tinker.
