# Stability and versions

From 1.0, Renox follows [semantic versioning]. Code that compiles against `renox = "1.x"` keeps
compiling on every later 1.x release, except where this page says otherwise. Until 1.0, breaking
changes are listed in each release's notes. New apps made by `rnx new` are pinned to the Renox
commit that `rnx` was built from.

[semantic versioning]: https://semver.org

## What 1.x may add without a major release

- **New fields on these structs.** Build them with their constructors, `Default` or
  `TestApp::with_config(|c| …)`, not with struct literals:
  - `Config`, `MailConfig`, `StorageConfig`, `AnalyticsConfig`
  - `Mail`, `User`, `Paginated`, `RouteInfo`, `MigrationStatus`, `FailedJob`
  - `DatabaseNotification`, `AccessToken`, `NewToken`
  - `WebhookRequest`, `WebhookCall`, `JobContext`, `Htmx`, `Down`, `analytics::Event`
- **New variants on these enums.** A `match` on them needs a `_` arm:
  - `Error`, `Environment`, `CspMode`, `Channel`, `Locale`, `DbValue`, `Inspected`
- New methods, functions, modules, template functions, validation rules, CLI commands and `.env`
  settings (always with defaults).

`Dialect` is deliberately not in that list. Supporting a third database would change the SQL every
app writes, so it would come with a major release.

## Public dependencies

Some crates show up in Renox's API. When one of them makes a breaking release, Renox moves to it
in a **major** release (or keeps the old one), so that `renox = "1"` never breaks your code
because of a dependency.

| Crate | Where it shows up |
|---|---|
| `axum` (0.8) | Handlers and extractors (`Path`, `Query`, `Form`, `Json`, `State`), `Routes::route(MethodRouter)`, `From<axum::Router>`, `Kernel::router()`, re-exported as `renox::axum` |
| `tower` / `tower-http` (0.5 / 0.7) | `Routes::route_layer(L)`, `Routes::cors_layer(CorsLayer)` (`renox::cors`) |
| `minijinja` (2) | `context!`, template values |
| `tokio` (1), `serde` (1), `serde_json` (1), `chrono` (0.4) | Re-exported and used throughout |
| `fake` (5) | `Factory` definitions, re-exported as `renox::fake` |

**`sqlx` is not part of the stable API.**
- Database errors are Renox's own `db::DbError`. Rows are `db::Row`, and values go through
  `ToDbValue`/`FromDb`.
- For anything Renox doesn't cover, these escape hatches hand out sqlx's own types:
  `Db::sqlite()`, `Db::postgres()`, `Row::sqlite()`, `Row::postgres()`, `DbError::sqlx()` and the
  re-export `renox::db::sqlx`.
- Renox may move to a new sqlx version in a minor release. Code that uses the escape hatches may
  then need a change.

## Also outside the promise

- Items marked `#[doc(hidden)]` (used by Renox's own macros).
- The HTML of the built-in pages under `renox/…` (override them in your views to fix their
  markup).
- The exact wording of built-in messages.
- The minimum supported Rust version (MSRV): it may rise in a minor release, and the release
  notes say so.
