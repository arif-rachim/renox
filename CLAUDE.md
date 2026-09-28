# Renox: guide for agents and contributors

Read this before changing anything. It records how Renox is built, why, how work is done in this
repo, and every trap hit so far, so you don't have to rediscover them.

**Read next**, depending on the task:
- `ROADMAP.md`: the plan, per-milestone notes and the "Decisions" section.
- `CHANGELOG.md`: what changed, milestone by milestone.
- `CONTRIBUTING.md`: the checks every change needs. `SECURITY.md`: how vulnerabilities are reported.
- `CHEATSHEET.md` and `llms.txt`: the app author's view (patterns, and which example shows what).
- `docs/*.md`: guides (types, relations, PostgreSQL, operations, development, stability).
- `docs/audit/`: the pre-1.0 audit (finding IDs W*, D*, A* used in ROADMAP M13/M14).

## 1. What Renox is

- A **batteries-included web framework for Rust, modelled on Laravel**: Axum + HTMX + Alpine.js +
  SQLite (PostgreSQL optional). One dependency (`renox = "…"`, `use renox::prelude::*`) gives
  routing, sessions, CSRF, views, a model layer, migrations, validation, auth, queue, scheduler,
  events, mail, notifications, cache, storage, i18n, webhooks, SEO and analytics.
- **Apps depend on Renox; framework code is never copied into apps.** Upgrading is a version bump.
  This is a hard requirement from the owner.
- **The owner** is a solo founder, fluent in Rust, who reviews in Indonesian. Write code, comments,
  docs, commit messages and PRs in English.
- **Open source**, `MIT OR Apache-2.0`. Repo: https://github.com/arif-rachim/renox (default branch
  `main`). crates.io: `renox` and `renox-cli` have placeholder `0.0.1` releases (to reserve the
  names). `renox-core` and `renox-macros` are not published yet. Until real releases, `rnx new`
  pins apps to a git commit.
- **Names:** the framework is *Renox* (the name "Renoxium" was dropped: another repo of the owner's
  uses it). The CLI binary is **`rnx`** (crate `renox-cli`).
- `ROADMAP.md` is the plan and the record of decisions; keep its checkboxes, notes and
  "Decisions" section current in every milestone PR.

## 2. Workspace layout

```
Cargo.toml                 workspace: members crates/*, examples/*, tests/chaos; shared package
                           metadata; rust-version (MSRV); argon2 opt-level for dev builds
README.md                  front page (compiled as doctests: `ReadMe`)
ROADMAP.md, CHANGELOG.md   plan + decisions; changes per milestone
CONTRIBUTING.md            checks every change needs; SECURITY.md: reporting vulnerabilities
CHEATSHEET.md              one-page patterns for app authors/agents (compiled: `CheatSheet`)
llms.txt                   map for agents: which example/guide file shows what
deny.toml                  cargo-deny: licenses, advisories, banned crates, sources
crates/renox/              facade crate apps depend on: re-exports renox-core, the macros, prelude
  src/lib.rs               `pub use renox_core::*`, macros (DbEnum, FromRow, Model, embedded!,
                           migrations!, #[renox::test]), prelude, and cfg(doctest) holders:
                           ReadMe, CheatSheet, TypesGuide, RelationsGuide, MacroCompileErrors
  tests/it/                ONE integration-test binary (main.rs + a module per area); add new areas
                           as `mod x;` in main.rs. Notable modules: send_handlers.rs (every data
                           API in a routed handler), web_security.rs, data_resilience.rs,
                           background_resilience.rs, direct.rs (APIs otherwise tested only
                           indirectly), database.rs, postgres.rs (`postgres` feature), s3.rs
                           (`s3` feature), extension_points.rs, api_foundations.rs, data_layer.rs
  tests/migrations/, migrations_plain/, migrations_types/, views/   fixtures (not under it/)
crates/renox-core/         ALL runtime code (see §3 for why one crate)
  src/app.rs               App builder, boot(), Kernel, router assembly, app-binary commands
  src/config.rs            Config from env/.env (see §5)
  src/state.rs             AppState (Clone): config, routes, views, db, mailer, queue, cache,
                           storage, translator, listeners, key, gates/async gates, shares,
                           channels, provided values, throttle, security, webhooks, live
  src/module.rs            Module trait: name, routes, migrations, register
  src/registry.rs          Registry: jobs, listeners, schedule, commands, channels, shares, templates
  src/routing.rs           Routes builder (get/post/…/name/group/require_auth/guest_only/
                           require_verified/throttle/cors/route_layer/merge), RouteTable + URLs
  src/session.rs           encrypted cookie session + middleware
  src/csrf.rs              CSRF middleware
  src/view.rs              MiniJinja env, View response, render middleware, globals, BUILTIN views
  src/view_filters.rs      built-in template filters `number` and `date`; pub format_number
  src/htmx.rs              Htmx extractor, HxRedirect/HxRefresh/HxTrigger, Back
  src/assets.rs            embedded htmx/Alpine/renox.js with hashed URLs; renox.js source lives here
  src/error.rs             Error enum, IntoResponse, error pages, Debug for main(), panic_message
  src/crypto.rs            APP_KEY parsing/generation, random tokens, constant_time_eq
  src/signed.rs            signed URLs (HMAC-SHA256) + ValidSignature extractor
  src/db/                  conn.rs (Db/Transaction/Row/sql(), Executor, SchemaEpoch), mod.rs
                           (connect, TEST_DATABASE_URL), model.rs, query.rs, from_row.rs,
                           relations.rs (belongs_to/has_many/Pivot/Morph), value.rs (DbValue),
                           paginate.rs, migrate.rs (migrator), factory.rs, json.rs, error.rs
  src/validation/          Validator/rules (mod.rs), Valid<T> (extract.rs), en/id messages
  src/auth/                User, hashing (Argon2id + bcrypt import), login/logout (per device),
                           change_password, CurrentUser middleware, AuthUser, guards, Access::check,
                           Policy/gates (mod.rs), Auth module + pages (module.rs), account.rs
                           (account pages, password confirmation), passwords.rs (reset),
                           permissions.rs (Permissions module: roles, permissions), events.rs
                           (LoggedIn, LoginFailed, …), verification, tokens.rs (API tokens,
                           abilities, prune), LoginThrottle (pair/account/IP), notifications
                           (Recipient, Channel::Custom, notify/notify_to/notify_later,
                           SendToChannel job)
  src/audit.rs             Audit module (audit_logs table, records auth events), audit::record,
                           audit:prune
  src/context.rs           renox::context: task-local values per request/job/task/command
                           (default scopes, context::app())
  src/queue/               Job trait, Queue (dispatch, dispatch_in), Worker
  src/schedule.rs          Schedule + runner, ScheduledTask builder, own cron parser, run claims
  src/timezone.rs          Zone (UTC / fixed offset / IANA via chrono-tz) for APP_TIMEZONE
  src/events.rs            Event, listeners, AppState::emit
  src/mail.rs              Mail (recipients, cc/bcc/reply_to/from, attachments), Mailer
                           (smtp/log/memory), mail_view, queue_mail, /_renox/mail preview
  src/cache.rs             Cache (memory / database store), remember(), add/pull/increment,
                           Lock/LockGuard (`renox:lock:*` rows), prune
  src/counters.rs          counters in the cache table (renox:count:…) for shared throttles/login lock
  src/provided.rs          App::provide values: Provided<T> extractor, AppState::provided
  src/cookies.rs           Cookies extractor (plain / encrypted), SetCookie response part
  src/download.rs          Download: bytes, streamed file, Storage key, stream; safe Content-Disposition
  src/command.rs           app commands: Args, Command; App::command / Registry::command, Kernel::call
  src/rate_limit.rs        Limiter + middleware behind Routes::throttle
  src/client_ip.rs         ClientIp extractor + TrustedProxies (TRUSTED_PROXIES); resolved once in
                           security::middleware, read by throttle, login lock, trace span
  src/maintenance.rs       down/up/status + middleware (bypass cookie)
  src/health.rs            GET /health
  src/upload.rs            Upload (multipart file field), sniffing, store/store_public, token registry
  src/storage.rs           Storage (local disk; S3 with the `s3` feature), temporary URLs, /_renox/files
  src/i18n.rs              Translator (lang JSON files), format(), RequestLocale middleware, Lang
  src/live.rs              live reload: file-time polling, /_renox/live SSE, stop() on shutdown
  src/shell.rs             db:shell (run_with takes any input/output, for tests)
  src/testing.rs           TestApp / TestRequest / TestResponse for apps' tests
  src/seo.rs               seo() tags, head tags (noindex / verification / GA4 / GTM), robots.txt, Sitemap
  src/analytics.rs         analytics::event, GaClientId, ServerEvent job (`server-events` feature)
  src/webhook.rs           Webhook trait, receive route, webhook_calls store/retry, ProcessWebhook job
  src/security.rs          security headers + CSP (+ nonce), csrf-exempt and webhook route sets
  src/method.rs            method spoofing layer (in front of the router)
  src/embedded.rs          Embedded (views/lang/public compiled in), public-file serving + content types
  assets/                  vendored htmx.min.js (2.0.11), alpine.min.js (3.17.4) + Alpine CSP build
  views/                   built-in templates (error, pagination, auth/*, mail/*); see §4.3
  migrations/              framework-owned migrations (auth/, permissions/, audit/, queue/,
                           cache/, webhook/); see §4.5
  tests/                   core-only integration tests (support/mod.rs has a small TestApp)
crates/renox-macros/       proc macros: derive Model, FromRow, DbEnum; embedded!(), migrations!(),
                           #[renox::test]
crates/renox-cli/          `rnx`: main.rs (key:generate, forwarding), new.rs, serve.rs, make.rs +
                           generate.rs (make:*), deploy.rs (make:deploy)
  build.rs                 sets RENOX_GIT_REV (the commit `rnx new` pins apps to)
  stubs/                   the files `rnx new` writes (Cargo.toml.stub, env.stub, build.rs, src/,
                           resources/, migrations/, tests/, AGENTS.md.stub + CLAUDE.md.stub: the
                           new app's agent guide, named .stub so agents in this repo don't load
                           it); stubs/deploy/: Dockerfile/systemd/Litestream templates
examples/                  workspace members, each with a README.md and its own tests:
  hello/                   guestbook exercising many features; used for live/browser testing
  crud/                    the reference CRUD module (policy, soft deletes, pagination)
  api/                     JSON API with tokens, CORS, rate limit
  jobs/                    events, queued mail, notifications, schedule
  uploads/                 public / private files, several files, Download
  postgres/                one app on PostgreSQL + SQLite (package `postgres-app`)
  fields/                  every form field type ↔ Rust ↔ SQLite / PostgreSQL
  webhooks/                Midtrans / Xendit / Stripe webhooks
  shop/                    a whole online shop (auth, admin, checkout, mail, queue, i18n, deploy)
  htmx-recipes/            modal form, inline edit, infinite scroll, tabs, HxRefresh/HxRedirect
  relations/               belongs to, has many, many to many, no N+1
tests/chaos/               app + run.sh (postgres|sqlite) that the `chaos` CI job injects faults
                           into (docker pause/stop/restart, python3 holding SQLite's lock)
tests/cli/run.sh           `rnx new` + every `make:*`, then build and test the app (CI `cli`/`docker`)
docs/types.md              HTML input ↔ Rust ↔ SQLite ↔ PostgreSQL (doctest `TypesGuide`)
docs/relations.md          relations without N+1, fetch_as/FromRow (doctest `RelationsGuide`)
docs/postgresql.md         PostgreSQL guide for app authors
docs/development.md        faster builds: profiles, linker, default features, sccache, cargo-chef
docs/stability.md          semver scope, #[non_exhaustive] types, public-dependency policy
docs/operations.md         production: timeouts, proxies, /health, failure table (kept in sync
                           with tests/chaos/run.sh), failed jobs/webhooks, backups
docs/audit/                pre-1.0 audit (2026-09-pre-1.0.md), Laravel parity review
                           (2026-09-laravel-parity.md) and gap report (2026-09-laravel-gap-report.pdf)
docs/assets/demo.gif       the README's demo (see §4.10)
.github/workflows/ci.yml   CI jobs (see §4.11)
```

## 3. Architecture and the decisions behind it

### 3.1 Request pipeline (outermost first; `build_router` in app.rs)
1. `security::middleware`: resolves `ClientIp`, puts a `CspNonce` in extensions, adds nosniff /
   Referrer-Policy / X-Frame-Options / HSTS / CSP to every response unless the handler set them
   (policy built once at boot in `security::Security`). Outermost so even the next layer's 413
   gets the headers.
2. `method::middleware`: method spoofing (`_method=PUT|PATCH|DELETE` in a POST, or
   `X-HTTP-Method-Override`). It wraps the whole router via
   `Router::new().fallback_service(layer(router))`, because route layers run after axum has
   matched the method.
3. `TraceLayer` (span with method, uri, client IP).
4. `DefaultBodyLimit` (`UPLOAD_MAX_SIZE`).
5. Merged at this level, so they skip everything below (sessions, maintenance): `assets::router()`
   (`/_renox/*.js`), `/health`, `/robots.txt` (unless `public/robots.txt` exists), `/_renox/live`
   (debug + local only) and the local disk's public files (`/storage/...`, sandboxed headers).
6. `context` (`renox::context`: a fresh task-local context per request holding the `AppState`
   for `context::app()`; jobs, scheduled tasks and app commands get one too via `scope_app`) → `session` (encrypted cookie) → `i18n` (`RequestLocale`: session `_locale`, else
   `APP_LOCALE`) → `auth` (loads the user once from session or `Authorization: Bearer`, with the
   token's abilities, and the user's roles/permissions when the `Permissions` module is on;
   inserts `CurrentUser` and `AppState` into extensions) → `csrf` → `view` (renders `View`s and error
   pages; `ValidationError` → redirect back for plain forms) → `maintenance` (503 while
   `storage/framework/down` exists; inside `view` so the 503 uses the error template) → `guard`
   (a handler panic or a run past `REQUEST_TIMEOUT` becomes a 500 with the error page).
7. `App::layer` layers (first added = outermost), around the modules' routes only.
8. Routes. Also inside the layers: `/_renox/files` (storage), `/_renox/mail` (only with
   `APP_DEBUG`), and the fallback: `public/` (`ServeDir`, or the embedded files) with a 404.

Because `AppState` and `CurrentUser` are in request extensions, guards (`require_auth`, …) are
plain `from_fn` middlewares with no state parameter and can be added from `Module::routes()`.

### 3.2 Key decisions (also in ROADMAP "Decisions")
- **One runtime crate (`renox-core`)**, not renox-http/-db/-view: those would all need `AppState`
  and `App` would need all of them (circular). `renox` is a thin facade.
- **Cargo features:** `renox` defaults to `fake` and `server-events`; optional `postgres`, `s3`,
  `uuid`. renox-core is `default-features = false` in the workspace deps; the `renox` crate owns
  the defaults.
- **Sessions are an encrypted, signed cookie** (`cookie` PrivateJar, key derived from `APP_KEY`),
  so no DB is needed. Keep session data small (a warning is logged over 4 KB). Flash data lives
  one request. A per-session lifetime override powers "remember me". Logout bumps
  `users.sessions_revoked_at`, checked on every request, so it ends every session of the user.
- **Auth sessions store the user id + a fingerprint of the password hash** (no remember-token
  column): changing the password logs out other sessions.
- **Views: MiniJinja** (runtime, overridable, autoreload in debug via `minijinja-autoreload`).
  App templates override built-ins by file name (loader checks `VIEWS_PATH` first, then `BUILTIN`).
- **HTML escaping uses a custom formatter** (`view.rs::format_value`): escapes `& < > " '` but not
  `/` (MiniJinja's default escapes `/` as `&#x2f;`, which uglified URLs in pages and mail).
- **The app binary is its own CLI** (like artisan): `my-app migrate|migrate:rollback|migrate:fresh|
  migrate:status|db:seed|queue:work|queue:failed|queue:retry|queue:flush|webhook:failed|
  webhook:retry|cache:prune|schedule:list|schedule:run|schedule:work|route:list|db:shell|down|
  up|help`, default `serve` (modules add more: `tokens:prune` from Auth, `audit:prune` from Audit),
  plus the app's own commands (`App::command`; names can't clash with built-ins). Migrations and
  jobs are compiled into the app, so only the app can run them. `rnx <anything unknown>` forwards
  to `cargo run --quiet -- <args>`.
- **Own migrator** (table `renox_migrations`, Laravel-style batches) instead of sqlx's, to support
  batches and module-owned migrations. `migrations!()` embeds `*.up.sql`/`*.down.sql` (or plain
  `*.sql`, not reversible). Apps need `build.rs` with `cargo:rerun-if-changed=migrations` so new
  files are picked up (`rnx new` writes it).
- **Models:** `#[derive(Model)]` generates `impl ::renox::db::Model` (and `FromRow`) using
  `::renox::…` paths; values go through `DbValue`/`ToDbValue`, rows decode via `Row::try_get`, so
  apps don't need sqlx directly. Primary key is always `id: i64`, `0` = unsaved. Table name =
  snake_case struct name (no pluralisation; Indonesian names don't pluralise with "s"). The query
  builder validates column names against `COLUMNS` and operators against a whitelist, so SQL
  injection via names is an error.
- **Relations are explicit loaders, no lazy relations** (`db::relations`: `belongs_to`,
  `has_many`, `Pivot`, `Morph`; each loads a page's related rows in one query). See
  docs/relations.md.
- **Model hooks are opt-in:** `#[model(hooks)]` makes the derive forward `Model::saving/saved/
  deleting/deleted` to `impl ModelHooks`. Only `save`, `save_only`, `save_changes`, `delete`
  and `force_delete` call them; bulk query methods never do. Keep it that way (documented).
- **Validation:** fluent rules in `impl Validate` (no derive yet). `Valid<T>` handles form, JSON
  and GET query. HTMX/JSON failures → `422 {"message","errors"}`; the bundled `renox.js` places
  errors next to inputs (`data-error-for` slots or inserted `<p class="error">`), sets
  `aria-invalid`, focuses the first invalid input in *page* order. Plain posts → 303 back with
  errors + old input flashed (never passwords).
- **Queue is Renox's own** (`jobs`, `failed_jobs`, unix-second integers) on SQLite and PostgreSQL;
  PostgreSQL workers reserve with `FOR UPDATE SKIP LOCKED`. apalis was the plan but its stable SQL
  backend needs sqlx 0.8 (can't link next to our 0.9: both link `libsqlite3-sys`).
- **Workers and scheduler run inside `serve`** by default (single-process deploys). Several
  instances may share one database: every scheduled run is claimed first (`schedule::claim`:
  insert `renox:schedule:<task>:<slot>` into `cache` with `ON CONFLICT DO NOTHING`, kept for the
  interval + 1 min (a day for daily tasks); expired claims are pruned at most once a minute).
- **Single-file deploys:** `App::embed(renox::embedded!())` bakes views, lang files and `public/`
  into the binary; they're used only when `APP_DEBUG` is off (debug keeps disk + live reload).
  New built-in behaviour that reads `VIEWS_PATH`/`LANG_PATH`/`PUBLIC_PATH` must also handle the
  embedded source.
- **Mail:** lettre with rustls (no OpenSSL). Drivers `smtp`, `log` (default), `memory` (tests).
- **Uploads are form fields:** `Valid<T>` turns multipart files into tokens that `Upload`'s
  `Deserialize` resolves from a thread-local during the synchronous serde pass (`upload.rs`), so
  `struct Form { photo: Option<Upload> }` works with the normal validation path. `image()`/`mimes()`
  sniff the content; stored names are random with an extension from the content.
- **S3 is the opt-in `s3` feature** (object_store pulls reqwest + aws-lc-rs). It is tested against
  SeaweedFS in the `s3` CI job (`crates/renox/tests/it/s3.rs`).
- **No C crypto in default builds:** reqwest uses `rustls-no-provider` and `analytics` installs
  the ring provider, which lettre uses too. CI checks `cargo tree -p hello -e normal -i aws-lc-rs` is
  empty. `deny.toml` doesn't ban aws-lc, since the `s3` feature brings it.

### 3.3 The database layer (SQLite and PostgreSQL)
SQLite is the default; PostgreSQL (the `postgres` feature, shipped in M9b) is for apps that
outgrow one server. `Db` is Renox's own type (`db/conn.rs`): a private enum over `SqlitePool` and
`PgPool`, chosen by `DATABASE_URL`'s scheme in `db::connect`. User-facing guide: docs/postgresql.md.
- All framework SQL goes through `crate::db::sql("… ? …").bind(v)` and `.fetch_all/
  fetch_optional/fetch_one/execute/scalar/scalar_optional/scalars/fetch_as(executor)`; migration
  files go through `db::script` (multi-statement, no params). Never call `sqlx::query` on `&Db`.
- Executors are `&Db` or `&mut Transaction` (trait `db::Executor`; `db.begin()` returns
  `Transaction`, pass `&mut tx`, **not** `&mut *tx`). Models take `E: Executor<'c>`. Statements
  that run several times on one executor use `Conn::reborrow()`.
- Rows are `db::Row` (`try_get::<T>(name_or_index)`, `columns()`); `T: FromDb` means "decodes on
  every enabled backend". `from_row` lives in `db::FromRow` (supertrait of `Model`).
- Always write `?` placeholders; `numbered_placeholders` turns them into `$n` on PostgreSQL
  (quotes/comments skipped). Engine-specific code matches on `db.dialect()`, or on
  `db.sqlite()` / `db.postgres()` for raw sqlx (see `migrate.rs` `drop_all_*`, `shell.rs` cells).
- `DbValue` has typed variants (Bool, DateTime, NaiveDateTime, Date, Time, Json, Uuid). SQLite
  binds them via `DbValue::for_sqlite()` (0/1, text formats); PostgreSQL binds them typed, and
  `Null` as an OID-0 `UntypedNull`.
- Query conditions are `query::Filter` values (Sql, Like, JsonIn, Group, Not, InQuery) rendered
  per dialect at execution, with binds kept in render order. `like` → `ILIKE` on PostgreSQL;
  `OFFSET` without `LIMIT -1` there. Aggregate sums are cast (`db::Number`, sealed) so
  PostgreSQL's NUMERIC doesn't leak.
- Public database APIs return `db::DbError`, never `sqlx::Error`; macros reach sqlx through the
  hidden `renox::__sqlx`.
- Emails: `auth::user::normalize_email` (trim + lowercase) on register, lookup, reset and the
  registration `unique` check; PostgreSQL has a unique index on `lower(email)`.
- `cfg(feature = "postgres")` arms: check both `cargo clippy --all-targets` and `--all-features`.

## 4. Conventions you must follow

### 4.1 Public API and docs
- Laravel naming where it maps cleanly (route names like `password.reset`, `verification.notice`,
  commands like `queue:work`), Rust idioms otherwise.
- Every public item gets a doc comment. Doc examples are doctests: never write ```` ```ignore ````.
  renox is a dev-dependency of renox-core and renox-macros, so examples `use renox::prelude::*`
  as apps do. Hide setup with `# ` lines and wrap statements in
  `# async fn demo(..) -> Result { … # Ok(()) }`; examples that would start a server are `no_run`.
- Handlers return `renox::Result<T>`; any `anyhow`-compatible error converts with `?`.
- New public structs/enums that may grow get `#[non_exhaustive]` and a line in docs/stability.md
  (also keep its public-dependency list in sync when adding re-exports).
- When a public API changes, fix CHEATSHEET.md, README.md, llms.txt and the examples in the same PR.

### 4.2 Handler futures must be `Send`
axum needs `Send` handler futures, and rustc (issue #100013) can't prove it when a generic future
holds a closure over `&T`, or a generic iterator, across an `.await`. Doctests and plain tests
don't route handlers, so they miss it. Data APIs that take closures or iterators are plain `fn`s
returning `impl Future<Output = …> + Send + 'a` that read what they need first (`relations.rs`,
`Query::first_or_create`). Trait methods returning futures are declared `-> impl Future + Send`.
In Rust 2024 such a return type captures every lifetime in the arguments (`children: &[C]` too),
so a helper can't build a temporary `Vec` and pass `&tmp` to a loader: inline the query instead
(see `Morph::parents`).
**Add every new data API to `crates/renox/tests/it/send_handlers.rs`.** In middleware, don't keep
a closure borrowing `req` alive across `next.run(req).await` (scope it in a block).

### 4.3 Built-in templates, texts and template helpers
- Add the file under `crates/renox-core/views/…` **and** register it in `BUILTIN` in `view.rs`.
  Built-in names are prefixed `renox/` (e.g. `renox/auth/login.html`). Forgetting `BUILTIN` gives
  "template not found" at runtime only.
- Templates are semi-strict while debugging, and the whole test suite runs with `debug = true`,
  so built-in templates must only print defined values (`flash` is an object returning "" for
  missing keys).
- Template context: `merge_maps([shared, view ctx, globals])`; the **last** map wins (shared
  values, then the handler's, then Renox's globals).
- Auth pages/mails take a `text` object from `auth/module.rs::texts(&lang)`: the built-in en/id
  dictionary with the app's `renox.auth.*` translations on top. Add new keys to **both** built-in
  locales. Auth handlers take a `Lang` extractor (background code uses
  `Lang::of(state, &state.config.locale)`).
- Validation messages live in `validation/messages.rs` (keys like `required`, `min.string`,
  `max.file`, `auth.failed`); apps override them with `renox.validation.<key>` and name fields with
  `renox.validation.attributes.<field>` (`messages::template_for`). Built-in labels for Renox's own
  forms use `Field::fallback_label`, so an app's attribute translation wins.
- Helpers that need the page's `app`/`request` (`seo()`, `page_url()`) are Rust functions
  registered per render, not MiniJinja macros: an imported macro can't see the caller's context.
  `page_url(n)` reads `request.query` through `minijinja::State::lookup`, which works inside
  imported macros. `can(ability, target)` reads `target._can` (from `auth::Can`), `can(gate)` asks
  a gate; `method_field('PUT')`.
- Analytics events live in the session (`_renox_analytics`) until the view middleware delivers
  them: htmx 2xx swap → `HX-Trigger` `{"renox:analytics":{"events":[…]}}` (merged with the
  handler's own), full page → `<meta name="renox-analytics">` in `renox_head()`, anything else
  (redirects) → kept for the next page.
- Whether a 500 page shows the error chain is decided per app (`ErrorPage::shown_detail(debug)`);
  there is no process-wide debug flag, so apps with and without debug can share a test binary.

### 4.4 Adding config
Add the field to `Config` (its doc names the env var), parse it in `Config::from_vars`, add a case
to the config tests there, give it a test-friendly value in `Default`, and document it in
`crates/renox-cli/stubs/env.stub` (and `examples/hello/.env.example` when the guestbook uses it).
App-specific settings need no field: `config.var(name)` reads `config.vars`, then the environment.

### 4.5 Migrations owned by the framework
Names start with `0001…` so they sort before app migrations (`2026…`). There are thirteen:
- Auth module (`auth/module.rs` `MIGRATIONS`): `00010101000000_create_users_table`,
  `…000001_create_password_reset_tokens_table`, `…000002_create_personal_access_tokens_table`,
  `…000003_create_notifications_table`, `…000004_add_sessions_revoked_at_to_users`,
  `…000005_add_abilities_to_personal_access_tokens`, `…000006_create_revoked_sessions_table`.
- Permissions module (`auth/permissions.rs`): `00010101000500_create_roles_and_permissions_tables`
  (roles, permissions, permission_role, role_user).
- Audit module (`audit.rs`): `00010101000600_create_audit_logs_table`.
- Every app (registered in `App::boot`): `00010101000100_create_jobs_table` (queue),
  `00010101000200_create_cache_table` (cache), `00010101000300_create_webhook_calls_table` and
  `00010101000301_store_webhook_payloads_as_bytes` (webhook.rs `MIGRATIONS`).

Each is `NAME.up.sql` (SQLite) + `NAME.postgres.up.sql` + a shared `NAME.down.sql`, included with
`db::framework_migration!(dir, name)`. PostgreSQL versions use BIGINT identity ids, BIGINT
integers and TIMESTAMPTZ dates. A migration that needs its own PostgreSQL `down` is written as a
`Migration` literal (see `webhook::MIGRATIONS`), since `framework_migration!` has none. Apps get
the same through `migrations!()` (`*.postgres.up.sql` / `*.sqlite.up.sql` overrides →
`Migration::up_for(dialect)`). Adding a framework migration changes migration counts asserted in
`crates/renox/tests/it/database.rs`.

**Schema changes vs pooled connections:** `Db` carries a `SchemaEpoch`; the migrator marks it once
per batch (each rollback step, `fresh`), and pools from `db::connect` drop connections opened
before it (`before_acquire`). Without it a pre-migration connection's `SELECT *` panicked in
sqlx-sqlite and returned no rows. Mark per batch, not per migration: per migration made the
PostgreSQL suite 2.5x slower (reconnects).

### 4.6 Background work and errors
- Framework rows in the `cache` table start with `renox:` (`Cache::flush` keeps them).
- `Error::permanent` marks errors the queue won't retry; `error::panic_message` formats caught
  panics. Jobs run in their own task, so a panic is a failed attempt.
- `App::listen` listeners run before modules' (modules register at boot).
- `Error`'s `Debug` is hand-written so `fn main() -> renox::Result` prints readable errors.
- Throttle ids come from the covered routes, so they're the same in every process.

### 4.7 Tests
- Tests using the macros must live in `crates/renox/tests/it/` (the macros emit `::renox::` paths)
  as a module of `it/main.rs`. **Don't add new top-level `tests/*.rs` files**: each becomes its own
  binary linked against sqlx/axum, which made builds 4x slower before they were merged. Core-only
  tests go in `crates/renox-core/tests/` or unit tests.
- Prefer `renox::testing::TestApp` (keeps cookies, sends CSRF, has assertions); older tests drive
  `kernel.router()` with `tower::ServiceExt::oneshot` and keep the session cookie by hand (update
  it from **every** response: flashed errors and old input live in the redirect's cookie). For
  CSRF there, add a route returning `session.token()`; `/login` redirects logged-in users.
- `#[renox::test]` replaces `#[tokio::test]`.
- `TestApp` does **not** read `.env` (except `TEST_DATABASE_URL`): it starts from
  `Config::default()` (en locale, memory mail, in-memory DB, no workers, no scheduler, debug on)
  plus a temp storage dir; change it with `TestApp::with_config`. Set `debug: false` explicitly to
  test production behaviour.
- Build configs by mutation (`let mut c = Config::default(); c.x = …;`); `Config` is
  `#[non_exhaustive]`. Test env parsing through `Config::from_vars(|name| …)`, never by setting
  env vars (tests run in parallel).
- `Config::default()` keeps a 30 s acquire timeout (parallel tests open many SQLite files; the
  `.env` default is 5 s), while in-memory SQLite caps it at 2 s so a task waiting on its own
  transaction fails fast.
- `Kernel` helpers: `migrate()`, `run_jobs()` (drains the queue), `mailer().sent()` (memory
  driver), `state()`, `db()`, `worker(queues)`, `call(command, args)`.
- Macro misuse is tested with `compile_fail` doctests on `#[cfg(doctest)] struct
  MacroCompileErrors` in `crates/renox/src/lib.rs` (not trybuild, and not in renox-macros: a
  proc-macro crate can't export that struct).
- Timing-based tests need slack on PostgreSQL (a round trip is ~0.2 s under a parallel suite).
- **On PostgreSQL:** `TEST_DATABASE_URL` (env or `.env`, read in `db/mod.rs`) makes `db::connect`
  swap any in-memory SQLite URL for a fresh `renox_test_…` schema (pool capped at 3). Run:
  ```
  docker run -d --rm --name renox-pg -e POSTGRES_PASSWORD=postgres -e POSTGRES_DB=renox_test \
      -p 55432:5432 postgres:17-alpine
  TEST_DATABASE_URL=postgres://postgres:postgres@localhost:55432/renox_test \
      cargo test -p renox -p renox-core -p renox-cli -p postgres-app -p fields --features renox/postgres
  ```
  Not the other examples (SQLite migrations).
- **S3:** `cargo test -p renox --features s3 --test it s3` with `TEST_S3_ENDPOINT`,
  `TEST_S3_BUCKET`, `TEST_S3_ACCESS_KEY_ID`, `TEST_S3_SECRET_ACCESS_KEY` (without them the tests do
  nothing). The SeaweedFS commands are at the top of `it/s3.rs`.
- Don't run tests with `--release` (slow compile, no debug assertions).

### 4.8 Forms and validation internals
- Forms are deserialized with `serde_html_form` (repeated names → `Vec`). The retry loop in
  `validation/extract.rs` first rewrites browser values (`coerce_browser_value`: checkbox
  `on`/missing → bool, datetime-local + `:00`), then uses placeholders (`PLACEHOLDERS`: an enum's
  first variant from the error's "expected one of", then `0`, `false`), so the other rules still
  run; rule errors on placeholder fields are dropped. `Parsed::Ok(T, Errors)` carries the parse
  errors. `Valid`'s logic is `validation::extract::validate_request` (with an extra-rules hook,
  used by `/register`).
- `Validator::rules_of` applies rules synchronously before the async DB checks, so `Valid<T>`
  doesn't require `T: Sync`.
- `User` selects `*` (`Model::SELECT_ALL`) and keeps unknown columns in `extra` (never
  password/sessions_revoked_at); `User::register` reads the row back.
- `#[derive(DbEnum)]` emits `::renox::__db_text_type!(T)`; that macro_rules is defined twice in
  renox-core (with/without `postgres`) so the sqlx impls match renox-core's features, not the
  app's. Don't use `#[cfg(feature)]` inside exported macros: it would test the caller's features.

### 4.9 The CLI (`rnx`)
- Apps are lib + bin: `src/lib.rs` has `pub fn app() -> App`, `main.rs` runs it, `tests/` boot it.
  `rnx make:module` registers modules in `src/lib.rs` (falls back to `main.rs` for older apps).
- `rnx make:job` / `make:command` insert their `app.job::<…>()` / `app.command(…)` into the
  module's `fn register` (creating it before `fn routes`); `make:migration` bumps the timestamp
  past the newest migration.
- `tests/cli/run.sh [postgres]` makes an app with every generator and builds/tests it
  (`FROM_GIT=1 DOCKER=1` for the Docker job). **Add every new `make:*` there.**
- `rnx new` pins `rev` from `renox-cli/build.rs` (`git rev-parse HEAD`, else the cargo checkout
  directory's short rev). Testing it through `cargo install --git` needs the change committed,
  since that builds the committed tree.

### 4.10 Docs and examples for app authors and agents
- `README.md` is compiled (`ReadMe`): keep its Rust blocks complete. It's the front page, so it
  sells: tagline, why, GIF, 3-line quick start, a short taste, fold-out feature tour, comparison
  with Loco/Axum, then status. Keep claims true (checked against the code) and keep the
  crates.io/docs.rs badges out until real crates are published (install stays `--git`).
- `CHEATSHEET.md` is compiled: every ```rust block must build on its own (visible `use` lines, no
  `# ` hidden lines since GitHub shows them; define items only, no top-level statements, so the
  doctest's `main` does nothing). Check with `cargo test --doc -p renox`.
- Examples are workspace members with their own tests and a README.md; each shows one pattern the
  official way, and its module doc names the `rnx make:*` commands that made it. Keep `llms.txt`
  and the example READMEs in step when adding or changing example files.
- A route group's index is `.get("/", …)` (not `""`, which panics).
- Examples use `renox.workspace = true`, so their `make:deploy` Dockerfile builds only in a copy
  made by `rnx new` (the CI docker job covers that).
- Examples without a `.env` run with `APP_DEBUG` off, i.e. with the views embedded at build time:
  restart after editing templates.
- Browser-check example UIs. Found that way in `examples/crud`: `hx-boost` on a whole section also
  boosts its edit links and delete forms (scope it to the page links), and boosted requests get
  full pages (by design, `Htmx::wants_fragment`), so pair them with `hx-select`.
- Under `CSP=strict`, Alpine's CSP build rejects statements in attributes (e.g. examples/hello's
  `@htmx:after-request="sending = false; if (…) $el.reset()"` → "CSP Parser Error: Unexpected
  token: if"). That's why relaxed is the default; strict apps move logic into `Alpine.data`.
  Browser-check CSP work by collecting `Log.entryAdded` / `Runtime.exceptionThrown` over CDP.
- `docs/assets/demo.gif` was made by driving examples/hello (`APP_LOCALE=en`) in headless Chrome
  over CDP (`Page.captureScreenshot` per typed character, `Input.insertText`), then composing the
  frames with Pillow (browser bar, captions, 64-colour palette, ~210 KB). Re-record it when the
  guestbook's look changes.

### 4.11 Git, PRs, CI (how the owner works)
- One branch and one PR per milestone or fix (branch names like `m17b-examples`, `fix-…`). The
  owner reviews and **merges PRs themselves**, then says "Done"/"lanjut". Don't merge unless asked.
- Before starting, check open PRs (`gh pr list -R arif-rachim/renox`) and base new branches on an
  up-to-date `main`. **Don't stack PRs** on unmerged branches (§6.3): push the next branch, but open
  its PR only after the previous one is merged.
- Each milestone PR also updates: `CHANGELOG.md` (under "Unreleased", a `### Mxx · title`
  section; breaking changes marked), ROADMAP checkboxes and notes, §7 of this file, `llms.txt`,
  and the example READMEs when examples change.
- **Commit messages and PR bodies are long and structured** (the owner asked for good
  descriptions and notes): summary paragraph, *What's included*, *Design notes*, *Deviations from
  the roadmap*, *Testing* (what was actually run, including browser checks). End commits and PR
  bodies with the attribution lines the harness gives you.
- Before every commit (the list in CONTRIBUTING.md "What every change needs"):
  ```
  cargo fmt --all
  cargo clippy --workspace --all-targets -- -D warnings
  cargo clippy --workspace --all-targets --all-features -- -D warnings
  RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
  cargo test --workspace
  ```
  plus the PostgreSQL run (§4.7) when SQL changed.
- CI (`.github/workflows/ci.yml`) runs more: **lint** (fmt, clippy default / `--all-features` /
  `-p renox --no-default-features`, the aws-lc guard `! cargo tree -p hello -e normal -i
  aws-lc-rs`, doc), **test** on Ubuntu/macOS/Windows, **test (PostgreSQL)**, **chaos** (sqlite,
  postgres; `tests/chaos/run.sh`), **MSRV (1.94)**, **feature matrix** (`cargo hack check -p
  renox-core -p renox --each-feature --no-dev-deps`), **cli** (sqlite, postgres;
  `tests/cli/run.sh`), **docker** (`make:deploy` image answers `/health`), **s3** (SeaweedFS),
  **cargo-deny**, **coverage** (informational).
- MSRV is `rust-version` in the workspace `Cargo.toml` (1.94, set by sqlx 0.9); the `msrv` job
  uses the same number, so raise both together and note it in CHANGELOG.md.
- After pushing, watch CI (`gh run watch <id> -R arif-rachim/renox --exit-status`) and tick the
  "CI green" box in the PR body.

## 5. Configuration (env vars)
Parsed in `crates/renox-core/src/config.rs`; defaults in parentheses.
- **App:** `APP_NAME` (Renox), `APP_ENV` (local; accepts local|dev|development,
  testing|test, production|prod; anything else fails at boot), `APP_DEBUG` (on in local),
  `APP_URL`, `APP_KEY` (required in production; `base64:…`, `rnx key:generate`), `APP_HOST`
  (127.0.0.1, an IP), `APP_PORT` (3000), `APP_LOCALE` (en; built-ins for en|id),
  `APP_FALLBACK_LOCALE` (en), `APP_TIMEZONE` (`UTC`, an offset like `+07:00`, or an IANA name like
  `Asia/Jakarta`, with DST).
- **Paths:** `VIEWS_PATH` (resources/views), `LANG_PATH` (resources/lang), `PUBLIC_PATH`
  (public), `STORAGE_PATH` (storage; holds `framework/down` for maintenance mode and `app/` for the
  local disk). Paths are relative to the working directory: run apps from their own directory.
- **Sessions:** `SESSION_LIFETIME` (minutes, 120), `SESSION_COOKIE` (renox_session),
  `REMEMBER_LIFETIME` (minutes, 43200).
- **Database:** `DATABASE_URL` (sqlite://storage/app.db, or `postgres://…` with the `postgres`
  feature; other schemes fail), `DATABASE_POOL_SIZE` (8, at least 1), `DATABASE_ACQUIRE_TIMEOUT`
  (seconds, 5), `DATABASE_STATEMENT_TIMEOUT` (seconds, 30, PostgreSQL; 0 = none).
- **Requests:** `REQUEST_TIMEOUT` (seconds, 60; 0 = none), `UPLOAD_MAX_SIZE` (MB, 10),
  `TRUSTED_PROXIES` (addresses, CIDR ranges or `*`), `CSP` (relaxed|strict|off, also `false`/`none`
  for off; relaxed is the owner's chosen default).
- **Mail:** `MAIL_MAILER` (log; smtp|log|memory), `MAIL_HOST`, `MAIL_PORT`, `MAIL_ENCRYPTION`
  (starttls; tls|starttls|none), `MAIL_USERNAME`, `MAIL_PASSWORD`, `MAIL_FROM_ADDRESS`,
  `MAIL_FROM_NAME`, `MAIL_TIMEOUT` (seconds, 10, the whole send).
- **Background:** `QUEUE_WORKERS` (2; 0 = none in serve), `SCHEDULER` (true), `CACHE_STORE`
  (memory|database; database also shares throttles and the login lock between servers).
- **Storage:** `STORAGE_DISK` (local|s3), `S3_BUCKET`, `S3_REGION`, `S3_ENDPOINT`,
  `S3_ACCESS_KEY_ID`, `S3_SECRET_ACCESS_KEY`, `STORAGE_URL`.
- **SEO/analytics** (used in production only): `GOOGLE_SITE_VERIFICATION`, `GA4_MEASUREMENT_ID`,
  `GA4_API_SECRET`, `GTM_CONTAINER_ID`.
- **App-specific:** anything else through `config.var(name)` (`config.vars` first, then the
  environment; empty counts as missing).
- **Tests only:** `TEST_DATABASE_URL` (read in `db/mod.rs`, env or `.env`; see §4.7) and
  `TEST_S3_*` (the S3 test).

## 6. Problems hit so far, and their fixes (read this)

### 6.1 Tooling and environment
- **`pkill -f <pattern>` / `pgrep -f` killed my own shell** (exit code 144, twice) because the
  pattern matched the command line of the shell running it. Stale servers then kept port 3000 and
  new ones failed with "Address already in use", so the next test silently hit the *old* binary.
  Fix: start background processes with `& echo $! > pidfile` and `kill $(cat pidfile)`; check
  `ps` for leftovers. Never use `pkill -f`/`pgrep -f`.
- **Piping a node script into `| head -1` killed it with SIGPIPE** before it saved its screenshot.
  Redirect to a file and `head` the file instead.
- **`cargo test` doesn't rebuild example binaries** used for live tests; run `cargo build` first.
- **`gh pr edit` fails** with a GraphQL error about deprecated Projects (classic). Use REST:
  `gh api -X PATCH repos/arif-rachim/renox/pulls/N -F body=@file`.
- **`target/` grows to ~100 GB** over a few milestones and fills the disk (link errors, "No space
  left on device"); `cargo clean` before the full two-database run.
- **Browser testing** works with headless Chrome + the DevTools protocol from a small Node script
  (Node 24 has a global `WebSocket`): launch `google-chrome --headless=new --no-sandbox
  --remote-debugging-port=9222 --user-data-dir=<temp dir>`, get the page's `webSocketDebuggerUrl`
  from `http://127.0.0.1:9222/json`, then `Page.navigate`, `Runtime.evaluate` (fill inputs, click,
  read DOM) and `Page.captureScreenshot`. This found bugs unit tests missed (§6.4). A sandboxed
  `<iframe>` (mail preview) can't be read from the parent: check it via screenshot.

### 6.2 `rnx serve` restart loop
The `notify` watcher reports **OPEN** events (inotify `OPEN` is in notify 8's mask), so `cargo build`
reading `src/` triggered a restart, which triggered a build… Fix in `serve.rs`: events only wake the
loop; a restart happens only if a **fingerprint** (path, mtime, size of watched files) changed.
Also: build first and swap the running app only on success; run `<exe> migrate` before starting.

### 6.3 GitHub PR stacking
PR #4 was based on `m2-database` (stacked on #3). #3 was merged **without deleting the branch**, so
GitHub didn't retarget #4, and merging #4 put the change into `m2-database`, not `main`. Fixed by
opening #5 with the same commit to `main`. Lesson: don't stack; if you must, retarget first.

### 6.4 Bugs found only in a real browser
- 422 validation responses were marked `isError = false` in `htmx:beforeSwap`, so htmx called the
  request "successful" and the form's Alpine `@htmx:after-request` handler **reset the form**,
  wiping the user's input. Keep 422s as errors; only set `shouldSwap = false`.
- A blank text field made serde report "missing field" before any rule ran → one unlabelled error.
  Fix in `validation/extract.rs`: drop empty inputs (so `Option` fields become `None`), and if
  deserialization reports a missing field, put it back as `""` and retry, so rules and labels run.
  A blank number then becomes "required", a wrong type "must be a number".
- Focus went to the alphabetically first field (errors are a `BTreeMap`); now the first
  `[aria-invalid]` in DOM order.
- Indonesian message said "Password minimal 8 karakter" while the page said "Kata sandi" →
  localised labels in auth forms.
- Reset-password mail button reused the page's "Simpan kata sandi" label → uses the subject.
- A scripted edit meant to add `.image()` to the guestbook's photo rule silently didn't apply, so a
  text file named `.png` was accepted. All framework tests passed; only the headless-Chrome upload
  check showed it. Keep browser checks for UI features.

### 6.5 Scripted edits go wrong silently
This happened in M6b, M6c, M7 and M9a:
- Python string replacements after `cargo fmt` did nothing (rustfmt had re-wrapped the target).
- A Python heredoc with nested `\"` quoting failed to parse, so *nothing* was applied.
- A multi-line `perl -0pi` regex with a repeated group (`(\s*\.bind(..))*?`) kept only the *last*
  capture and dropped `.bind(key)` from the cache lookup; a test caught it.

What works: prefer the Edit tool on freshly read content; for scripts, write the Python to a file
and `assert old in s` before each replace; rewrite a small file whole rather than patch it in many
places. Afterwards `grep` for the new text, run `cargo fmt --all` (CI failed once on an unformatted
test), and after regex rewrites of call chains audit with `git diff -U0 | grep -E '^[-+].*\.bind\('`
(removed binds must match added ones).

### 6.6 Slow tests (measured, fixed)
Unoptimised Argon2 made every login/registration slow (auth tests 3.9 s) and ten integration-test
binaries each paid a full link. The workspace `Cargo.toml` builds `argon2` with `opt-level = 3` in
the dev profile; new apps get the same plus `blake2` from `stubs/Cargo.toml.stub` (profiles only
apply at a workspace root). The integration tests are one binary. Result: rebuild after a core
change 29 s → 7 s, full run 19 s → 6 s.

### 6.7 Library/API traps
- **sqlx 0.9:** dynamic SQL needs `sqlx::AssertSqlSafe(string)`; `SqliteArguments` has no lifetime;
  multi-statement SQL uses `sqlx::raw_sql`. `sqlite::memory:` gives each pooled connection its own
  DB → pool of exactly 1 connection with no idle timeout (see `db::connect`). `PRAGMA foreign_keys`
  is per connection: `migrate:fresh` acquires one connection for the whole drop.
- **axum 0.8:** paths are `/{id}` and `/{*rest}`; `Option<Extractor>` needs
  `OptionalFromRequestParts`; `route_layer` only wraps routes added *before* it (so
  `.require_auth()` must come after the routes it guards); middleware and handler futures must be
  `Send` (§4.2).
- **MiniJinja 2.24:** `eval_to_state` is deprecated → `render_captured_to(ctx, io::sink())` +
  `with_state_mut(|s| s.render_block(..))` for fragments. `merge_maps`: the **last** map wins
  (lookups go in reverse). `tojson`/`urlencode` need the `json`/`urlencode` features (enabled).
  Booleans render as `True`/`False`: use `{% if %}` in templates/tests.
- **argon2 0.6:** `Argon2::default().hash_password(bytes)` (needs default `getrandom` feature),
  verify with `password_hash::phc::PasswordHash`. Hashing runs in `spawn_blocking`. Unknown emails
  are verified against a static dummy hash to equalise timing.
- **Crypto crate versions:** our `sha2 0.11` + `hmac 0.13` share `digest 0.11`; `cookie` pulls the
  older `sha2 0.10/hmac 0.12`. That's fine, but don't mix types across them.
- **Stable clippy** wants let-chains (`if let … && cond {}`) instead of nested ifs.
- **`cargo doc --workspace` output collision:** a binary and a library with the same name
  (`renox`) wrote to the same `target/doc/renox`, failing CI randomly. Resolved by the `rnx` rename.
- **Windows CI:** Git checks out with CRLF, so `include_str!` content ends in `\r\n`; compare
  trimmed text in tests.
- **`Option<T>` has a std `inspect` method,** so call `FieldValue::inspect(&opt)` explicitly in
  tests; generic code in the validator is unaffected.
- **`MultipartError` already carries the right status** (e.g. 413 over the body limit): return
  `err.into_response()`, don't wrap it in `Error::BadRequest`.
- **object_store** needs `with_allow_http` for `http://` endpoints (a local MinIO/SeaweedFS failed
  on every request until the S3 CI job caught it). MinIO's images are gone from Docker Hub/quay;
  use `chrislusf/seaweedfs` (bucket via `weed shell`).

### 6.8 PostgreSQL traps
- sqlx decodes strictly: `i64` needs `BIGINT` (a plain `INTEGER` column is INT4 and fails), and
  `SELECT 1` is INT4 (`/health` returned 503 until it stopped decoding the ping). `SUM(bigint)` is
  `NUMERIC`: write `CAST(SUM(x) AS BIGINT)`.
- sqlx sends parameters in binary with a declared type: a text-typed `NULL` or date string can't
  go into a `TIMESTAMPTZ`, hence the typed `DbValue` variants and the OID-0 untyped `NULL`.
  (Sending text with OID 0 does *not* work for non-text columns: the bytes are binary-format.)
- PostgreSQL keeps microseconds: `db::now()` truncates to them, or a saved model won't equal the
  row read back.
- `citext` doesn't help: `citext_col = $1` with a text parameter compares as text (case-sensitive).
- The worker arms `Notify::notified()` *before* querying, so a dispatch during the query isn't
  lost (it was, under PostgreSQL's slower round trips).

### 6.9 Pre-1.0 audit and hardening (M13–M16)
- Findings with IDs (W* web, D* data/background, A* Laravel gaps) and the chaos baseline are in
  `docs/audit/2026-09-pre-1.0.md`; ROADMAP M13/M14 use the same IDs.
- Every audit probe is now a passing test on main: `it/web_security.rs`, `it/data_resilience.rs`,
  `it/background_resilience.rs`, and the `tests/chaos` app. A new finding lands the same way: a
  failing test first, then the fix.
- docs/operations.md's failure table is kept in sync with `tests/chaos/run.sh`.

### 6.10 Security rules
- Never store API tokens or secrets in the repo, memory or notes. Publishing to crates.io needs
  the owner to run `cargo login` themselves (and a verified email on the account).
- Never send the owner's email address to third-party services (e.g. in a User-Agent); use a
  neutral one like `renox-deps-check`.

## 7. Where things stand (update this section when it changes)

- **All milestones M0–M20a are merged to `main`**; the last was M20a (#51). History:
  `CHANGELOG.md` (per milestone) and `ROADMAP.md` (per-milestone notes and decisions).
- After M17: a docs refresh (#45) and the Laravel parity review with M18–M21 planned (#46).
  After M20a: a docs and examples catch-up (branch `claude/laravel-project-feature-report-i6wgz0`:
  README, operations, examples moved to the M18–M20a APIs, the gap report PDF).
- **M18a** (tenancy: `renox::context`, default scopes, scoped `unique`/`exists`; `require_gate`,
  `gate_before`; the `Permissions` module; token abilities): merged (#47). Authorization is in one place: `auth::Access::check` (gate_before →
  gate → permission); `gate_before` doesn't answer role membership.
- **M18b** (account pages, `Password` policy, password confirmation, per-device logout with a
  `revoked_sessions` denylist, auth events + the `Audit` module, bcrypt import): merged (#48).
  Sessions: each login stores `_auth_session_id`; `resolve` checks the password
  fingerprint, `sessions_revoked_at` and the denylist in one query.
- **M19a** (query builder: raw fragments, group/having/select_as, locks, EXISTS, count/sum
  loaders, simple/cursor pagination, update_or_create, refresh, transaction helpers): merged
  (#49). `Query` keeps `having_binds` apart and `all_binds()` joins them
  after the WHERE binds; use it in every terminal method.
- **M19b** (model hooks, `context::app()`, `save_only`/`save_changes`, `state.encrypt`/
  `decrypt`, pivot data/timestamps/toggle, `Morph`): merged (#50). Non-integer
  keys and `Encrypted<T>` are deferred (ROADMAP notes say why).
- **M20a** (scheduler: cron/weekly/monthly, filters, IANA zones with DST, on_failure/on_success,
  `schedule:run`; cache add/pull/increment, locks, prune): merged (#51). A cron
  time skipped by DST runs right after the jump; intervals follow the current offset. Schedule
  methods return `ScheduledTask` (DerefMut to `Schedule`) so add-chains still compile.
- **Next: M20b (queue), M20c (dashboard, mail, HTTP client, storage), then M21 (views and DX)**, from the Laravel parity review
  (`docs/audit/2026-09-laravel-parity.md`); the ROADMAP lists each milestone's items. **v1.0 is
  on hold** until the owner says to start it (docs site, starter kit, semver checks, real
  crates.io releases; the owner runs `cargo login`).
- **Other open items** noted in ROADMAP: `#[derive(Validate)]`, choosing the locale from
  `Accept-Language` (opt-in).
- As of M20a: ~40k lines of Rust in `crates/` (stubs excluded), ~435 `#[test]`/`#[renox::test]`/
  `#[tokio::test]` functions in `crates/` and `examples/` (plus doctests), and 39 direct
  dependencies in renox-core (5 optional). Keep dependencies lean and remove unused ones.
