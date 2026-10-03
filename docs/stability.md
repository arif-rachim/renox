# Stability and versions

From 1.0, Renox follows [semantic versioning]. Code that compiles against `renox = "1.x"` keeps
compiling on every later 1.x release, except where this page says otherwise. Until 1.0, breaking
changes are listed in each release's notes. New apps made by `rnx new` are pinned to the Renox
commit that `rnx` was built from.

[semantic versioning]: https://semver.org

The crates `renox`, `renox-core` and `renox-macros` are released together with the same
version and pin each other exactly: the macros write code against renox-core's items of the
same release. Depend on `renox` only; the other two follow it. `renox-cli` (`rnx`) has the
same version and makes apps depend on its own release.

## What 1.x may add without a major release

- **New fields on these structs.** Build them with their constructors, `Default` or
  `TestApp::with_config(app, |c| …)`, not with struct literals:
  - `Config`, `MailConfig`, `StorageConfig`, `AnalyticsConfig`
  - `Mail`, `User`, `Paginated`, `SimplePage`, `CursorPage`, `RouteInfo`, `MigrationStatus`,
    `FailedJob`, `audit::AuditLog`, the `auth::events` structs
  - `DatabaseNotification`, `AccessToken`, `NewToken`
  - `WebhookRequest`, `WebhookCall`, `JobContext`, `Htmx`, `Down`, `analytics::Event`
  - `view::ViewContext`, `auth::Registration`, `auth::Recipient`, `mail::Attachment`
  - `Toast`, `ToastAction`, `auth::DatabaseMessage`, `chart::Series`, `report::ErrorReport`, `report::RequestReport`, `validation::FormContext`,
    `rate_limit::LimitRequest`, `SentNotification`, `db::InvalidUlid`, `grid::Grid`,
    `grid::Column`, `grid::GridPrefs`, `grid::RowOrder`,
    `grid::Action`, `grid::Selection`, `storage::FileInfo`, `queue::BatchStatus`,
    `queue::QueueCounts`, `queue::QueueStats` (the dashboard's), `http::SentRequest`,
    `select::SelectOption` (use `SelectOption::new`), `select::OptionQuery`, `Upload` (use
    `Upload::new`), `db::Migration` (use `Migration::new(..).sqlite(..).postgres(..)`)
- **New variants on these enums.** A `match` on them needs a `_` arm:
  - `Error`, `Environment`, `CspMode`, `Channel`, `DbValue`, `Inspected` (a `Rule`
    matching on `Inspected` needs a `_` arm), `ToastKind`, `chart::Bucket`, `report::ReportKind`, `grid::Kind`, `grid::Summary`
  - the settings: `SessionDriver`, `LogFormat`, `CacheStore`, `mail::MailDriver`,
    `mail::MailEncryption`, `storage::DiskDriver`, and `webhook::WebhookStatus`
- **New fields on this enum variant.** A pattern on it needs `..`:
  - `Inspected::File { image, .. }` (it gained `dimensions` in M33)
- New methods, functions, modules, template functions, validation rules, CLI commands and `.env`
  settings (always with defaults).
- New provided methods and associated constants with defaults on traits you implement
  (`Model`, `Notification`, `ModelHooks`, `validation::ValidateHooks`, `Validate::ERROR_BAG`, …); `FromRow` stays one method. `db::Number` is sealed: only `i64`
  and `f64`. `db::ModelKey` is sealed too (`i64`, `Ulid`, `Uuid`, `String`), so new key types
  and new methods on it aren't breaking, and so is `RedirectExt` (only for axum's `Redirect`).

`Dialect` is deliberately not in that list. Supporting a third database would change the SQL every
app writes, so it would come with a major release.

## Public dependencies

Some crates show up in Renox's API. When one of them makes a breaking release, Renox moves to it
in a **major** release (or keeps the old one), so that `renox = "1"` never breaks your code
because of a dependency.

| Crate | Where it shows up |
|---|---|
| `axum` (0.8) | Handlers and extractors (`Query`, `Form`, `Json`, `State`), `Routes::route(MethodRouter)`, `From<axum::Router>`, `Kernel::router()`, re-exported as `renox::axum`. The prelude's `Path` is Renox's own `renox::Path` (a 404 when a value doesn't parse) |
| `clap` (4) | `command::AppCommand` (a `clap::Parser`), re-exported as `renox::clap` |
| `anyhow` (1) | `Error::Internal`, `Error::permanent`, re-exported as `renox::anyhow` |
| `tower` / `tower-http` (0.5 / 0.7) | `Routes::route_layer(L)`, `Routes::cors_layer(CorsLayer)` (`renox::cors`) |
| `minijinja` (2) | `context!`, template values |
| `tokio` (1), `serde` (1), `serde_json` (1), `chrono` (0.4) | Re-exported and used throughout |
| `fake` (5) | `Factory` definitions, re-exported as `renox::fake` |
| `uuid` (1) | `Uuid` model keys and fields, re-exported as `renox::uuid` (the `uuid` feature) |
| `chrono-tz` (0.10) | `renox::timezone::Zone::Named(chrono_tz::Tz)` (not re-exported; parse zones with `"Asia/Jakarta".parse::<Zone>()`) |

**`sqlx` is not part of the stable API.**
- Database errors are Renox's own `db::DbError`. Rows are `db::Row`, and values go through
  `ToDbValue`/`FromDb`.
- For anything Renox doesn't cover, these escape hatches hand out sqlx's own types:
  `Db::sqlite()`, `Db::postgres()`, `Row::sqlite()`, `Row::postgres()`, `DbError::sqlx()` and the
  re-export `renox::db::sqlx`.
- Renox may move to a new sqlx version in a minor release. Code that uses the escape hatches may
  then need a change.
- `FromDb` and `RowIndex` are implemented for whatever sqlx can decode, through sqlx's traits.
  The promise covers the types Renox lists: the integer and float types, `bool`, `String`,
  `Vec<u8>`, the chrono date and time types, `Option<T>` of those, `db::Json<T>`,
  `db::Encrypted<T>`, `db::Ulid`, `Uuid` (the `uuid` feature) and `#[derive(DbEnum)]` enums;
  column names (`&str`) and positions (`usize`) for `RowIndex`. A type of your own made
  decodable by implementing sqlx's traits works too, but is an escape hatch: a sqlx upgrade
  may need it changed.

## Also outside the promise

- Items marked `#[doc(hidden)]` (used by Renox's own macros).
- The HTML of the built-in pages under `renox/…` (override them in your views to fix their
  markup). For the UI kit (`renox/ui.html`), the macro names and keyword arguments, the `rx-*`
  class names apps use and the `--rx-*` tokens are kept; its inner markup may change.
  `rnx make:component --ui` copies the kit into the app to freeze it.
- The exact wording of built-in messages.
- The minimum supported Rust version (MSRV), now Rust 1.94 (`rust-version` in `Cargo.toml`,
  checked in CI): it may rise in a minor release, and CHANGELOG.md says so.
