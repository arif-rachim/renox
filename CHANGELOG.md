# Changelog

Notable changes to Renox. From 1.0 the project follows [semantic versioning]; see
[docs/stability.md](docs/stability.md) for what counts as a breaking change. Until the first
release on crates.io, apps made by `rnx new` are pinned to a commit, and this file lists
changes by milestone (each one pull request; details in its description and in
[ROADMAP.md](ROADMAP.md)).

[semantic versioning]: https://semver.org

## Unreleased

### Docs refresh

- The `AGENTS.md` that `rnx new` writes links the cheat-sheet and llms.txt of the Renox commit the
  app is pinned to (not `main`), points to examples/shop, htmx-recipes, relations and the guides,
  and lists the traps agents hit (`{id}` routes, `require_auth` order, handlers that aren't `Send`).
- CHEATSHEET: path parameters, route guards, `unique(…).ignore(id)`, signed URLs, queues and
  delays, plurals and language switching, auth options, seeders and factories, template globals
  and overridable pages, more test helpers, every `.env` setting.
- README: a "Coming from Laravel" table, a Documentation section, an updated status and upgrade
  path, prerequisites, and a corrected comparison with Loco.
- CLAUDE.md rewritten for accuracy (pipeline, CI jobs, env vars, conventions), without personal
  or session details. ROADMAP: finished items ticked, a "Not planned" section.
- Crate descriptions and keywords mention PostgreSQL.

### M17b · More examples, a README for each

- `examples/htmx-recipes` (modal form, inline edit, toggle, dropdown with delete, infinite
  scroll, tabs, `HxRefresh`, `HxRedirect`) and `examples/relations` (belongs to, has many,
  many to many with `sync` and `inverse`, no N+1, SQL reports).
- A README for every example.
- `AuthUser::token_id()`: the API token the request logged in with, to revoke just that one.
- `rnx key:generate` creates `.env` (from `.env.example` when there is one) instead of failing.
- examples/api: `DELETE /api/tokens/current` revokes only the token used; `DELETE /api/tokens`
  revokes all.

### M17a · examples/shop

- `examples/shop`: a whole online shop with its README (catalog, cart, checkout in one
  transaction, queued mail and notifications, a daily task, an admin with photo uploads, en/id,
  deploy files).
- **Fix:** `relations::belongs_to`, `has_many`, `Pivot::load`/`load_for` and
  `Query::first_or_create` couldn't be awaited in a routed handler ("implementation of `Send` is
  not general enough"). They now return `impl Future + Send` and hold no closure across an
  `.await`; a test routes every data API so this can't come back unnoticed.
- `first_or_create`'s closure must be `Send` (closures that capture only data are).

### Fixes

- **Stale schema in pooled connections.** After a migration added a column, a connection opened
  before it could run `SELECT *` on that table with the old column list. sqlx-sqlite then
  panicked, and the query returned no rows; PostgreSQL could refuse its cached plan. Pools now
  drop connections opened before the last migration batch run in the process. Seen as a flaky
  macOS failure of the multi-server test.

### M16b · CI and trust

- CI builds and tests an app made by `rnx new` with every `make:*` generator, on SQLite and
  PostgreSQL, and builds and starts the Docker image from `make:deploy`.
- CI also checks the MSRV (Rust 1.94, now `rust-version` in `Cargo.toml`), every feature on its
  own (`cargo hack`), S3 storage against a real S3 server (SeaweedFS), licenses and advisories
  (`cargo deny`), and reports line coverage.
- `rnx make:job` and `rnx make:command` register what they create in the module's `register`.
- `rnx make:migration` never gives two migrations the same timestamp, so migrations made in
  the same second run in the order they were made.
- `Config::from_vars` builds a config from any source of variables (tests, secrets managers).
- **Fix:** `STORAGE_DISK=s3` with an `http://` `S3_ENDPOINT` (a local MinIO or SeaweedFS) failed
  on every request.
- The macros' docs compile, and their misuse is tested to fail with a clear error.
- SECURITY.md, CONTRIBUTING.md and this changelog.

### M16a · Lighter builds (#40)

- No C crypto library in a default build: reqwest and lettre use rustls with ring.
- `fake` and `server-events` are features (on by default).
- `asset()` adds `?v=<content hash>`, and versioned assets are cached for a year.
- The Dockerfile from `make:deploy` uses cargo-chef: a code change rebuilds in seconds.
- With `CACHE_STORE=database`, rate limits and the login lock hold across servers.
- **Breaking:** `renox::fake` and `analytics::ServerEvent` need their features.

### M15b · Validation and requests (#39)

- Rules: regex, dates (`before`/`after`), digits, `in`/`not_in`, `required_if`/`required_with`,
  `same`/`different`, custom `Rule`s, per-item rules for lists.
- Several files in one field (`Vec<Upload>`), cookies (plain and encrypted), downloads and
  streamed responses.

### M15a · Query builder and relations (#38)

- `select`, `where_any`/`where_all`, `where_between`, `where_not_in`, `where_in_query`,
  `when`, aggregates, `pluck`, bulk `update`/`increment`, `insert_many`/`upsert`,
  `first_or_create`, `chunk`.
- `sql(…).fetch_as::<T>()` into a `#[derive(FromRow)]` struct, a model or a tuple.
- Explicit relations without N+1: `belongs_to`, `has_many`, `Pivot` (see `docs/relations.md`).
- **Breaking:** `Model::from_row` moved to the `FromRow` trait (only manual `impl Model`
  blocks change).

### M14c · Mail and notifications (#37)

- Mail with cc, bcc, reply-to and attachments; custom notification channels, notifications
  to people who aren't users, and queued notifications.
- **Breaking:** `Mail.to` is a `Vec<String>`; `Notification` methods take `&Recipient`.

### M14b · Extension points (#36)

- Template filters and functions, data shared with every view, `App::layer`, typed state,
  async gates, extra `User` columns and a registration hook.
- Templates are strict about undefined variables while debugging, and the debug error page
  shows the error chain and the request.

### M14a · API foundations (#35)

- `#[non_exhaustive]` on public structs and enums (see `docs/stability.md`), `Error::Status`
  and `abort`, route groups with prefixes, app commands (`App::command`, `rnx make:command`),
  and `rnx new` pins the app to a Renox commit.
- **Breaking:** struct literals and exhaustive `match`es on those types no longer compile;
  database methods return `DbError`; `renox::sqlx` moved to `renox::db::sqlx`.

### Earlier milestones

- **M13 (#31–#33):** fixes from the pre-1.0 security and resilience audit
  (`docs/audit/2026-09-pre-1.0.md`), a chaos test in CI, `docs/operations.md`.
- **M12 (#29):** every field type from the form to the database and back.
- **M11 (#23, #25, #26):** CSP with nonces, security headers, CORS per route; webhooks
  (verify, store once, process in the queue); SEO, sitemaps and analytics.
- **M10 (#21, #22, #27, #28):** examples, CHEATSHEET.md, llms.txt, compiled doc examples.
- **M9 (#19, #20):** Renox's own database layer, and PostgreSQL next to SQLite.
- **M0–M8:** the framework itself: routing, views, database and migrations, validation,
  authentication and policies, queue, scheduler and events, mail, cache, storage, i18n,
  the `rnx` CLI, test helpers, and single-binary deploys.
