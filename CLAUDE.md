# Renox: guide for agents

Read this before changing anything. It records how Renox is built, why, how work is done in this
repo, and every trap hit so far, so you don't have to rediscover them.

## 1. What Renox is

- A **batteries-included web framework for Rust, modelled on Laravel**: Axum + HTMX + Alpine.js +
  SQLite. One dependency (`renox = "…"`, `use renox::prelude::*`) gives routing, sessions, CSRF,
  views, a model layer, migrations, validation, auth, queue, scheduler, events, mail, notifications.
- **Apps depend on Renox; framework code is never copied into apps.** Upgrading is a version bump.
  This was a hard requirement from the owner ("modular, jangan ditanam codenya").
- **Owner:** Arif Rachim (GitHub `arif-rachim`, git identity `Arif Rachim <a.arif.r@gmail.com>`, set
  globally). Solo founder, fluent in Rust. **Talk to him in Indonesian**; write code, comments,
  commit messages, PRs and repo docs in English.
- **Open source**, `MIT OR Apache-2.0`. Repo: https://github.com/arif-rachim/renox (default branch
  `main`). crates.io: `renox` and `renox-cli` have placeholder `0.0.1` releases (published
  2026-09-26 to reserve the names). `renox-core` and `renox-macros` are not published yet.
- **Names:** the framework is *Renox* (the name "Renoxium" was dropped: it's taken by another repo
  of the owner's). The CLI binary is **`rnx`** (crate `renox-cli`, `cargo install renox-cli`).
- `ROADMAP.md` is the plan and the record of decisions; keep its checkboxes and "Decisions"
  section current in every milestone PR.

## 2. Workspace layout

```
Cargo.toml                 workspace; shared package metadata; workspace.dependencies for renox*
crates/renox/              facade crate apps depend on: re-exports renox-core, macros, prelude
  src/lib.rs               `pub use renox_core::*`, `pub use renox_macros::{Model, migrations}`, prelude,
                           and `CheatSheet` (cfg(doctest)): compiles every ```rust block of CHEATSHEET.md
  tests/it/                ONE integration-test binary (main.rs + a module per area) that needs
                           the derive/migrations! macros; add new areas as `mod x;` in main.rs
    migrations/, migrations_plain/   SQL fixtures for tests
crates/renox-core/         ALL runtime code (see §3 for why one crate)
  src/app.rs               App builder, boot(), Kernel, app-binary commands (migrate, queue:work…)
  src/config.rs            Config from env/.env (see §5)
  src/state.rs             AppState (Clone): config, routes, views, db, mailer, queue, cache, storage, translator, listeners,
                           key (cookie::Key), gates, throttle
  src/module.rs            Module trait: name, routes, migrations, register
  src/registry.rs          Registry: jobs, listeners, schedule (App-level and Module::register)
  src/routing.rs           Routes builder (get/post/…/name/require_auth/guest_only/require_verified/
                           route_layer/merge), RouteTable + URL generation
  src/session.rs           encrypted cookie session + middleware
  src/csrf.rs              CSRF middleware
  src/view.rs              MiniJinja env, View response, render middleware, globals, BUILTIN views
  src/htmx.rs              Htmx extractor, HxRedirect/HxRefresh/HxTrigger, Back
  src/assets.rs            embedded htmx/Alpine/renox.js with hashed URLs; renox.js source lives here
  src/error.rs             Error enum, IntoResponse, error pages, Debug for main()
  src/crypto.rs            APP_KEY parsing/generation, random tokens, constant_time_eq
  src/signed.rs            signed URLs (HMAC-SHA256) + ValidSignature extractor
  src/db/                  Db/Transaction/Row/sql() (conn.rs), connect + TEST_DATABASE_URL (mod.rs), Model trait (model.rs), Query builder (query.rs), DbValue
                           (value.rs), Paginated/Page (paginate.rs), migrator (migrate.rs), Factory
  src/validation/          Validator/rules (mod.rs), Valid<T> extractor (extract.rs), en/id messages
  src/auth/                User, hashing, login/logout, CurrentUser middleware, AuthUser, guards,
                           Policy/gates (mod.rs), Auth module + pages (module.rs), password reset,
                           verification, API tokens, LoginThrottle (pair/account/IP), notifications;
                           logout bumps users.sessions_revoked_at (checked in resolve)
  src/queue/               Job trait, Queue (dispatch), Worker
  src/schedule.rs          Schedule + runner; APP_TIMEZONE offsets
  src/events.rs            Event, listeners, AppState::emit
  src/mail.rs              Mail, Mailer (smtp/log/memory), mail_view, queue_mail, /_renox/mail preview
  src/cache.rs             Cache (memory / database store), remember()
  src/rate_limit.rs        Limiter + middleware behind Routes::throttle
  src/client_ip.rs         ClientIp extractor + TrustedProxies (TRUSTED_PROXIES); resolved once in
                           security::middleware (outermost), read by throttle, login lock, trace span
  src/maintenance.rs       down/up/status + middleware (bypass cookie)
  src/health.rs            GET /health
  src/upload.rs            Upload (multipart file field), sniffing, store/store_public, token registry
  src/storage.rs           Storage (local disk; S3 with the `s3` feature), temporary URLs, /_renox/files
  src/i18n.rs              Translator (lang JSON files), format(), RequestLocale middleware, Lang, set_locale
  src/live.rs              live reload: file-time polling, /_renox/live SSE, stop() on shutdown
  src/shell.rs             db:shell (run_with takes any input/output, for tests)
  src/testing.rs           TestApp / TestRequest / TestResponse for apps' tests (M8a)
  src/seo.rs               seo() tags, head tags (noindex / verification / GA4 / GTM), robots.txt, Sitemap
  src/analytics.rs         analytics::event (session → HX-Trigger / head meta), GaClientId, ServerEvent job
  src/webhook.rs           Webhook trait, receive route, webhook_calls store/retry, ProcessWebhook job
                           (`renox:webhook`), signature helpers
  src/security.rs          security headers + CSP (+ nonce), csrf-exempt and webhook route sets
  src/method.rs            method spoofing layer (in front of the router)
  src/embedded.rs          Embedded (views/lang/public compiled in), public-file serving + content types
  assets/                  vendored htmx.min.js (2.0.11), alpine.min.js (3.17.4)
  views/                   built-in templates (error, pagination, auth/*, mail/*) — see §4.4
  migrations/              framework-owned migrations (auth/*, queue/*, cache/*, webhook/*; each with a .postgres.up.sql) — see §4.5
  tests/                   core integration tests (support/mod.rs has TestApp)
crates/renox-macros/       proc macros: #[derive(Model)], migrations!()
crates/renox-cli/          `rnx`: new, serve, build, key:generate, make:* (generate.rs, make.rs, deploy.rs),
                           forwards the rest; stubs/deploy/ holds the Dockerfile/systemd/Litestream templates
  stubs/                   files `rnx new` writes (Cargo.toml.stub, env.stub, build.rs, views…,
                           AGENTS.md.stub + CLAUDE.md.stub (named .stub so agents in this repo don't load them) working on the app)
examples/hello/            guestbook app exercising many features in one file; used for live/browser testing
examples/webhooks/         Midtrans / Xendit / Stripe webhooks (M11b)
examples/fields/           every form field type ↔ Rust ↔ SQLite / PostgreSQL (M12); run in the PostgreSQL CI job
docs/types.md              the type mapping table (compiled as a doctest: `TypesGuide`)
examples/api/              JSON API with tokens (M10c)
examples/jobs/             events, queued mail, notifications, schedule (M10c)
examples/uploads/          public / private files (M10c)
examples/postgres/         one app on PostgreSQL + SQLite; package `postgres-app`; run in the PostgreSQL CI job (M10c)
examples/crud/             the reference CRUD module (policy, soft deletes, pagination) — M10a
CHEATSHEET.md              one-page patterns for app authors/agents; its Rust is compiled as doctests
llms.txt                   map for agents: which example/guide file shows what
docs/postgresql.md         PostgreSQL guide for app authors
.github/workflows/ci.yml   fmt+clippy+doc (Ubuntu), tests on Ubuntu/macOS/Windows, tests on PostgreSQL
```

## 3. Architecture and the decisions behind it

### Request pipeline (outermost first)
`security::middleware` (outermost with `DefaultBodyLimit`/Trace: puts a `CspNonce` in extensions,
adds nosniff / Referrer-Policy / X-Frame-Options / HSTS / CSP to every response unless the handler
set them; the policy is built once at boot in `security::Security`) wraps everything, and in front of
the router sits `method::middleware` (method spoofing: a POST with `_method=PUT|PATCH|DELETE` or
`X-HTTP-Method-Override` becomes that method; it wraps the whole router via
`Router::new().fallback_service(layer(router))` because route layers run after axum matched the
method) → `TraceLayer` → `session::middleware` (loads/saves encrypted cookie) → `i18n::middleware` (puts the
visitor's `RequestLocale` in extensions: session `_locale` if known, else `APP_LOCALE`) → `auth::middleware` (loads the
current user once per request from session or `Authorization: Bearer`, inserts `CurrentUser` and
`AppState` into request extensions) → `csrf::middleware` → `view::middleware` (renders `View`
responses, error pages, turns `ValidationError` into redirect-back for plain forms) →
`maintenance::middleware` (503 while `storage/framework/down` exists; inside the view layer so the
503 uses the error template) → routes. `assets::router()` (`/_renox/*.js`) and `health::router()`
(`/health`), `/_renox/live` (debug + local only) and the local public files (`/storage/...`) are
merged after the layers, so they skip
sessions and maintenance mode. `DefaultBodyLimit` (`UPLOAD_MAX_SIZE`) wraps everything.
`/_renox/mail` (mail preview) is merged only when `APP_DEBUG` is on. `public/` is the fallback
service (`ServeDir`) with a 404 handler.

Because `AppState` and `CurrentUser` are in request extensions, guards (`require_auth`, …) are
plain `from_fn` middlewares with no state parameter and can be added from `Module::routes()`.

### Key decisions (also in ROADMAP "Decisions")
- **One runtime crate (`renox-core`)**, not renox-http/-db/-view: those would all need `AppState`
  and `App` would need all of them (circular). `renox` is a thin facade.
- **Sessions are an encrypted, signed cookie** (`cookie` PrivateJar, key derived from `APP_KEY`),
  so no DB is needed for sessions. Keep session data small (<4 KB warning is logged). Flash data
  lives one request. Per-session lifetime override powers "remember me".
- **Views: MiniJinja** (runtime, overridable, autoreload in debug via `minijinja-autoreload`).
  App templates override built-ins by file name (loader checks `VIEWS_PATH` first, then `BUILTIN`).
- **HTML escaping uses a custom formatter** (`view.rs::format_value`): escapes `& < > " '` but not
  `/` (MiniJinja's default escapes `/` as `&#x2f;`, which uglified URLs in pages and mail).
- **The app binary is its own CLI** (like artisan): `my-app migrate|migrate:rollback|migrate:fresh|
  migrate:status|db:seed|queue:work|queue:failed|queue:retry|queue:flush|schedule:list|
  schedule:work|down|up|route:list|db:shell|help`, default `serve`. Migrations/jobs are compiled into the app, so only the app
  can run them. `rnx <anything unknown>` forwards to `cargo run --quiet -- <args>`.
- **Own migrator** (table `renox_migrations`, Laravel-style batches) instead of sqlx's, to support
  batches and module-owned migrations. `migrations!()` embeds `*.up.sql`/`*.down.sql` (or plain
  `*.sql`, not reversible). Apps need `build.rs` with `cargo:rerun-if-changed=migrations` so new
  files are picked up (`rnx new` writes it).
- **Models:** `#[derive(Model)]` generates `impl ::renox::db::Model` using `::renox::…` paths, values
  go through `DbValue`/`ToDbValue`, rows decode via `renox::sqlx::Row::try_get` — apps don't need
  sqlx directly. Primary key is always `id: i64`, `0` = unsaved. Table name = snake_case struct name
  (no pluralisation; Indonesian names don't pluralise with "s"). Query builder validates column
  names against `COLUMNS` and operators against a whitelist → SQL injection via names is an error.
- **Validation:** fluent rules in `impl Validate` (not a derive yet). `Valid<T>` handles form, JSON
  and GET query. HTMX/JSON failures → `422 {"message","errors"}`; the bundled `renox.js` places
  errors next to inputs (`data-error-for` slots or inserted `<p class="error">`), sets
  `aria-invalid`, focuses the first invalid input in *page* order. Plain posts → 303 back with
  errors + old input flashed (never passwords).
- **Auth sessions store the user id + a fingerprint of the password hash** (no remember-token
  column): changing the password logs out other sessions; "remember me" = longer session cookie.
- **Queue is Renox's own on SQLite** (`jobs`, `failed_jobs`, unix-second integers). apalis was the
  plan but its stable SQL backend needs sqlx 0.8 (can't link next to our 0.9: both link
  `libsqlite3-sys`) and its 0.9 backend is only an RC.
- **Workers and scheduler run inside `serve`** by default (single-process deploys). Several
  instances may share one database: workers reserve with `SKIP LOCKED` on PostgreSQL, and every
  scheduled run is claimed first (`schedule::claim`: insert `schedule:<task>:<slot>` into `cache`
  with `ON CONFLICT DO NOTHING`; stale claims deleted on the next claim).
- **SQLite now, PostgreSQL before 1.0.** The owner wants PostgreSQL for apps that outgrow one
  server. Since M9a, `Db` is Renox's own type (`db/conn.rs`): a private enum over `SqlitePool` and,
  with the `postgres` feature, `PgPool`, chosen by `DATABASE_URL`'s scheme in `db::connect`.
  - All framework SQL goes through `crate::db::sql("… ? …").bind(v)` and `.fetch_all/
    fetch_optional/fetch_one/execute/scalar/scalar_optional/scalars(executor)`; migration files go
    through `db::script` (multi-statement, no params). Never call `sqlx::query` on `&Db` directly.
  - Executors are `&Db` or `&mut Transaction` (trait `db::Executor`; `db.begin()` returns
    `Transaction`, pass `&mut tx`, **not** `&mut *tx`). Models take `E: Executor<'c>`.
  - Rows are `db::Row` (`try_get::<T>(name_or_index)`, `columns()`); `T: FromDb` means "decodes on
    every enabled backend". `derive(Model)` generates `from_row(row: &Row)`.
  - Always write `?` placeholders; `numbered_placeholders` turns them into `$n` on PostgreSQL
    (quotes/comments skipped). Engine-specific code matches on `db.dialect()`, or on
    `db.sqlite()` / `db.postgres()` for raw sqlx (see `migrate.rs` `drop_all_*`, `shell.rs` cells).
  - `cfg(feature = "postgres")` arms: check both `cargo clippy --all-targets` and
    `--all-features`.
  - M9b: framework migrations are `NAME.up.sql` (SQLite) + `NAME.postgres.up.sql` + a shared
    `NAME.down.sql`, included with `db::framework_migration!(dir, name)`. A new framework table
    needs both versions (BIGINT identity ids, BIGINT integers, TIMESTAMPTZ dates). Apps get the same
    through `migrations!()` (`*.postgres.up.sql` / `*.sqlite.up.sql` overrides → `Migration::
    up_for(dialect)`).
  - `DbValue` has typed variants (Bool, DateTime, NaiveDateTime, Date, Time). SQLite binds them as
    before via `DbValue::for_sqlite()` (0/1, the old text formats); PostgreSQL binds them typed, and
    `Null` as an OID-0 `UntypedNull`.
  - Query builder: `where_sql(dialect)` / `select_sql(dialect)`; `like` → `ILIKE` on PostgreSQL;
    `OFFSET` without `LIMIT -1` there. Get the dialect from `db.into_conn().dialect()` (Conn is an
    Executor too).
  - Emails: `auth::user::normalize_email` (trim + lowercase) on register, lookup, reset and the
    registration `unique` check; PostgreSQL has a unique index on `lower(email)`.
  - Testing on PostgreSQL: `TEST_DATABASE_URL` (env or `.env`) makes `db::connect` swap any
    in-memory SQLite URL for a fresh `renox_test_…` schema (pool capped at 3). Run the suite with
    `TEST_DATABASE_URL=postgres://postgres:postgres@localhost:55432/renox_test cargo test -p renox
    -p renox-core -p renox-cli --features renox/postgres` against
    `docker run -d --rm --name renox-pg -e POSTGRES_PASSWORD=postgres -e POSTGRES_DB=renox_test -p
    55432:5432 postgres:17-alpine`. Not `examples/hello` (SQLite migrations). CI job
    `test (PostgreSQL)` does this with a service container.
  - User-facing guide: `docs/postgresql.md`.
- **Single-file deploys:** `App::embed(renox::embedded!())` bakes views, lang files and `public/`
  into the binary; they're used only when `APP_DEBUG` is off (debug keeps disk + live reload).
  Embedded public files are served by the router's fallback (`embedded.rs`). New built-in behaviour
  that reads from `VIEWS_PATH`/`LANG_PATH`/`PUBLIC_PATH` must also handle the embedded source.
- **Mail:** lettre with rustls (no OpenSSL). Drivers `smtp`, `log` (default), `memory` (tests).
- **Uploads are form fields:** `Valid<T>` turns multipart files into tokens that `Upload`'s
  `Deserialize` resolves from a thread-local during the synchronous serde pass (`upload.rs`), so
  `struct Form { photo: Option<Upload> }` works with the normal validation path. `image()`/`mimes()`
  sniff the content; stored names are random with an extension from the content.
- **S3 is the opt-in `s3` feature** (object_store pulls reqwest + aws-lc-rs). CI runs clippy with
  `--all-features` so the S3 code keeps compiling; there is no S3 integration test (no server in CI).

## 4. Conventions you must follow

### 4.1 Public API style
- Laravel naming where it maps cleanly (route names like `password.reset`, `verification.notice`,
  commands like `queue:work`), Rust idioms otherwise.
- Every public item gets a doc comment; module docs show a usage example with ```` ```ignore ````.
- Handlers return `renox::Result<T>`; any `anyhow`-compatible error converts with `?`.
- `Config::default()` is the **test** config: `debug: true`, `database_url: sqlite::memory:`,
  `mail.mailer: "memory"`, `queue_workers: 0`, `scheduler: false`. Set `debug: false` explicitly
  when testing production behaviour.

### 4.2 Adding a built-in template
Add the file under `crates/renox-core/views/…` **and** register it in `BUILTIN` in `view.rs`.
Built-in names are prefixed `renox/` (e.g. `renox/auth/login.html`). Forgetting `BUILTIN` gives
"template not found" at runtime only.

### 4.3 Adding config
Add the field to `Config` (docs mention the env var), parse it in `from_env`, give it a test-friendly
value in `Default`, and document it in `crates/renox-cli/stubs/env.stub` and
`examples/hello/.env.example`.

### 4.4 Built-in views and texts
Auth pages/mails take a `text` object from `auth/module.rs::texts(&lang)`: the built-in en/id
dictionary (`text(locale)`) with the app's `renox.auth.*` translations on top. Add new keys to
**both** built-in locales. Auth handlers take a `Lang` extractor for the visitor's language
(background code uses `Lang::of(state, &state.config.locale)`). Validation messages live in
`validation/messages.rs` (keys like `required`, `min.string`, `max.file`, `auth.failed`); apps
override them with `renox.validation.<key>` and name fields with `renox.validation.attributes.<field>`
(`messages::template_for`). Built-in labels for Renox's own forms use `Field::fallback_label`, so an
app's attribute translation wins.

### 4.5 Migrations owned by the framework
Names start with `0001…` so they sort before app migrations (`2026…`):
`00010101000000_create_users_table`, `…000001_create_password_reset_tokens_table`,
`…000002_create_personal_access_tokens_table`, `…000003_create_notifications_table` (Auth module)
and `00010101000100_create_jobs_table` + `00010101000200_create_cache_table` (every app, registered
in `App::boot`). Adding a framework
migration changes migration counts asserted in `crates/renox/tests/database.rs`.

### 4.6 Tests
- Tests using `#[derive(Model)]` / `migrations!()` must live in `crates/renox/tests/it/` (the
  macros emit `::renox::` paths) as a module of `it/main.rs`. **Don't add new top-level
  `tests/*.rs` files**: each becomes its own binary linked against sqlx/axum, which made builds
  4x slower before they were merged. Core-only tests go in `crates/renox-core/tests/` or unit tests.
- HTTP tests drive `kernel.router()` with `tower::ServiceExt::oneshot`, keeping the session cookie
  by hand. For CSRF, add a route returning `session.token()` (e.g. `/token`) — `/login` redirects
  logged-in users, so it can't be used to read the token after login.
- Prefer `renox::testing::TestApp` in new tests (it keeps cookies, sends CSRF, has assertions);
  older tests use hand-written clients. `#[renox::test]` replaces `#[tokio::test]`.
- `TestApp` does **not** read `.env` (except `TEST_DATABASE_URL`, see §3): it starts from
  `Config::default()` (en locale, memory mail, in-memory DB) plus a temp storage dir; set anything
  else with `TestApp::with_config`.
- `Kernel` helpers: `migrate()`, `run_jobs()` (drains the queue), `mailer().sent()` (memory driver),
  `state()`, `db()`, `worker(queues)`.
- Apps are lib + bin: `src/lib.rs` has `pub fn app() -> App`, `main.rs` runs it, `tests/` boot it.
  `rnx make:module` registers modules in `src/lib.rs` (falls back to `main.rs` for older apps).
- Whether a 500 page shows the error chain is decided per app in the view middleware
  (`ErrorPage::shown_detail(config.debug)`); there is no process-wide debug flag any more, so apps
  with and without debug can run side by side in one test binary.
- Template-level helpers that need the page's `app`/`request` (e.g. `seo()`, `page_url()`) are
  Rust functions registered per render, not MiniJinja macros: an imported macro can't see the
  caller's context. Analytics events live in the session (`_renox_analytics`) until the view
  middleware delivers them: htmx 2xx swap → `HX-Trigger` `{"renox:analytics":{"events":[…]}}`
  (merged with the handler's own), full page → `<meta name="renox-analytics">` in `renox_head()`,
  anything else (redirects) → kept for the next page.
- Forms are deserialized with `serde_html_form` (repeated names → `Vec`). The retry loop in
  `validation/extract.rs` first rewrites browser values (`coerce_browser_value`: checkbox
  `on`/missing → bool, datetime-local + `:00`), then uses placeholders (an enum's first variant
  from the error's "expected one of", then `0`, `false`).
- `#[derive(DbEnum)]` emits `::renox::__db_text_type!(T)`; that macro_rules is defined twice in
  renox-core (with/without `postgres`) so the sqlx impls match renox-core's features, not the
  app's. Don't use `#[cfg(feature)]` inside exported macros: it would test the caller's features.
- `Valid<T>` for forms: a field that fails to parse gets its error plus a placeholder value
  (`validation/extract.rs` `PLACEHOLDERS`), so the other rules still run; rule errors on
  placeholder fields are dropped. `Parsed::Ok(T, Errors)` carries those parse errors.
- Templates: `request.query` holds the raw query string; `page_url(n)` (reads it through
  `minijinja::State::lookup`, which works inside imported macros) keeps filters in page links;
  `can(ability, target)` reads `target._can` (from `auth::Can`), `can(gate)` asks a gate;
  `method_field('PUT')`.

### 4.6b Docs for app authors and agents (M10)
- `README.md` is compiled the same way (`ReadMe` in crates/renox/src/lib.rs): keep its Rust blocks
  complete. It's the front page, so it sells: tagline, why, GIF, 3-line quick start, a short
  taste, fold-out feature tour, comparison with Loco/Axum, then status. Keep claims true (checked
  against the code) and keep the crates.io/docs.rs badges out until real crates are published
  (crates.io still has the 0.0.1 placeholders, so install stays `--git`).
- `docs/assets/demo.gif` was made by driving examples/hello (`APP_LOCALE=en`) in headless Chrome
  over CDP (`Page.captureScreenshot` per typed character, `Input.insertText`), then composing the
  frames with Pillow (browser bar, captions, 64-colour palette, ~210 KB). There's no ffmpeg here.
  Re-record it when the guestbook's look changes.
- renox-core's doc examples are doctests too (renox is its dev-dependency, so `use
  renox::prelude::*` and `derive(Model)` work there): never write ```ignore; hide setup with `# `
  lines and wrap statements in `# async fn demo(..) -> Result { … # Ok(()) }`; examples that
  define `fn main` and would start a server are `no_run`.
- `CHEATSHEET.md` is compiled: every ```rust block must build on its own (visible `use` lines, no
  `# ` hidden lines since GitHub shows them; define items only, no top-level statements, so the
  doctest's `main` does nothing). Check with `cargo test --doc -p renox`. When a public API
  changes, fix the cheat-sheet in the same PR.
- Examples are workspace members with their own tests, so CI runs them; each is one pattern,
  written the official way, and its module doc names the `rnx make:*` commands that made it.
  Keep `llms.txt` in step when adding or renaming example files.
- Browser-check example UIs. Things found that way in `examples/crud`: `hx-boost` on a whole
  section also boosts its edit links and delete forms (scope it to the page links), and boosted
  requests get full pages (by design, `Htmx::wants_fragment`), so pair them with `hx-select`.
- Under `CSP=strict`, Alpine's CSP build rejects statements in attributes (e.g. examples/hello's
  `@htmx:after-request="sending = false; if (…) $el.reset()"` → "CSP Parser Error: Unexpected
  token: if"). That's why relaxed is the default; strict apps move logic into `Alpine.data`.
  Browser-check CSP work by collecting `Log.entryAdded` / `Runtime.exceptionThrown` over CDP.
- Examples without a `.env` run with `APP_DEBUG` off, i.e. with the views embedded at build time:
  restart after editing templates.

### 4.7 Git, PRs, CI (how the owner works)
- One branch and one PR per milestone (`m1-web-layer`, `m2-database`, `m3-validation`, `m4-auth`,
  `m4b-auth-email`, `m5-queue`, `m5b-mail`). The owner reviews and **merges PRs himself**, then says
  "Done"/"lanjut". Don't merge unless he asks (exception so far: PR #5, a retarget of an already
  approved change).
- **Commit messages and PR bodies are long and structured** — the owner asked for "deskripsi sama
  notes yg bagus": summary paragraph, *What's included*, *Design notes*, *Deviations from the
  roadmap*, *Testing* (what was actually run, including browser checks). End commits with the
  `Co-Authored-By`/session trailer lines the harness gives you; PR bodies end with the
  "Generated with Claude Code" line.
- Before every commit: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
  && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && cargo test --workspace`.
  CI runs exactly these (fmt/clippy/doc on Ubuntu; tests on Ubuntu, macOS, Windows).
- After pushing, watch CI (`gh run watch <id> -R arif-rachim/renox --exit-status`) and tick the
  "CI green" box in the PR body.
- Don't stack PRs on unmerged branches (see §6.3). Push the next milestone's branch but open its PR
  only after the previous one is merged.

## 5. Configuration (env vars)
`APP_NAME`, `APP_ENV` (local|testing|production), `APP_DEBUG`, `APP_URL`, `APP_KEY` (required in
production; `base64:…`, `rnx key:generate`), `APP_HOST`, `APP_PORT`, `APP_LOCALE` (default language;
built-ins for en|id), `APP_FALLBACK_LOCALE` (en), `LANG_PATH` (resources/lang),
`APP_TIMEZONE` (`UTC` or offset like `+07:00`; IANA names are rejected), `VIEWS_PATH`
(resources/views), `PUBLIC_PATH` (public), `SESSION_LIFETIME` (minutes, 120), `SESSION_COOKIE`,
`REMEMBER_LIFETIME` (minutes, 43200), `DATABASE_URL` (sqlite://storage/app.db, or
`postgres://…` with the `postgres` feature), `DATABASE_POOL_SIZE` (8), `TEST_DATABASE_URL`
(PostgreSQL URL for tests; env or `.env`), `MAIL_MAILER` (smtp|log|memory), `MAIL_HOST`, `MAIL_PORT`,
`MAIL_ENCRYPTION` (tls|starttls|none), `MAIL_USERNAME`, `MAIL_PASSWORD`, `MAIL_FROM_ADDRESS`,
`MAIL_FROM_NAME`, `CSP` (relaxed|strict|off; relaxed is the owner's chosen default), `QUEUE_WORKERS` (2; 0 = none in serve), `SCHEDULER` (true), `CACHE_STORE`
(memory|database), `STORAGE_PATH` (storage; holds `framework/down` for maintenance mode and `app/`
for the local disk), `STORAGE_DISK` (local|s3), `S3_BUCKET`, `S3_REGION`, `S3_ENDPOINT`,
`S3_ACCESS_KEY_ID`, `S3_SECRET_ACCESS_KEY`, `STORAGE_URL`, `UPLOAD_MAX_SIZE` (MB, 10).
Paths are relative to the working directory: run apps from their own directory.

## 6. Problems hit so far, and their fixes (read this)

### 6.1 Tooling and environment
- **`pkill -f <pattern>` killed my own shell** (exit code 144) because the pattern matched the
  command line of the shell running it. Stale servers then kept port 3000 and new ones failed with
  "Address already in use" — the next test silently hit the *old* binary. Fix: start background
  processes with `& echo $! > pidfile` and `kill $(cat pidfile)`; check `ps` for leftovers.
- **Piping a node script into `| head -1` killed it with SIGPIPE** before it saved its screenshot.
  Redirect to a file and `head` the file instead.
- **`cargo test` doesn't rebuild example binaries** used for live tests; run `cargo build` first.
- **Editing with Python string replacement after `cargo fmt` silently did nothing** several times
  (fmt had re-wrapped the target text). Always `grep` for the new text after scripted edits, or use
  the Edit tool on the current file content. Also run `cargo fmt --all` after scripted edits: CI
  failed once on an unformatted test.
- **`gh pr edit` fails** with a GraphQL error about deprecated Projects (classic). Use REST:
  `gh api -X PATCH repos/arif-rachim/renox/pulls/N -F body=@file`.
- **Temporary files:** use the session scratchpad directory, not `/tmp` directly.
- **Browser testing** works with headless Chrome + the DevTools protocol from a small Node script
  (Node 24 has a global `WebSocket`): launch `google-chrome --headless=new --no-sandbox
  --remote-debugging-port=9222 --user-data-dir=<scratch>`, get the page's `webSocketDebuggerUrl`
  from `http://127.0.0.1:9222/json`, then `Page.navigate`, `Runtime.evaluate` (fill inputs, click,
  read DOM) and `Page.captureScreenshot`. This found bugs unit tests missed (§6.4). A sandboxed
  `<iframe>` (mail preview) can't be read from the parent — check it via screenshot.

### 6.2 `rnx serve` restart loop
The `notify` watcher reports **OPEN** events (inotify `OPEN` is in notify 8's mask), so `cargo build`
reading `src/` triggered a restart, which triggered a build… Fix in `serve.rs`: events only wake the
loop; a restart happens only if a **fingerprint** (path, mtime, size of watched files) changed.
Also: build first and swap the running app only on success; run `<exe> migrate` before starting.

### 6.3 GitHub PR stacking
PR #4 was based on `m2-database` (stacked on #3). #3 was merged **without deleting the branch**, so
GitHub didn't retarget #4, and merging #4 put the rename into `m2-database`, not `main`. Fixed by
opening #5 with the same commit to `main`. Lesson: don't stack; or if you must, retarget first.

### 6.4 Bugs found only in a real browser
- 422 validation responses were marked `isError = false` in `htmx:beforeSwap`, so htmx called the
  request "successful" and the form's Alpine `@htmx:after-request` handler **reset the form**, wiping
  the user's input. Keep 422s as errors; only set `shouldSwap = false`.
- A blank text field made serde report "missing field" before any rule ran → one unlabelled error.
  Fix in `validation/extract.rs`: drop empty inputs (so `Option` fields become `None`), and if
  deserialization reports a missing field, put it back as `""` and retry, so rules and labels run.
  A blank number then becomes "required", a wrong type "must be a number".
- Focus went to the alphabetically first field (errors are a `BTreeMap`); now the first
  `[aria-invalid]` in DOM order.
- Indonesian message said "Password minimal 8 karakter" while the page said "Kata sandi" → localised
  labels in auth forms.
- Reset-password mail button reused the page's "Simpan kata sandi" label → uses the subject.

### 6.4b Caught by the browser in M6b
A scripted edit meant to add `v.field("photo", ..).image()` to the guestbook silently didn't apply
(rustfmt had split the preceding line), so a text file named `.png` was accepted and stored. Unit
and integration tests of the framework passed; only the headless-Chrome upload check showed it.
Lesson: after scripted edits, `grep` for the added line; keep browser checks for UI features.

### 6.4c Editing files from scripts (M7)
Two more scripted edits went wrong: a Python heredoc with nested `\"` quoting failed to parse (so
*nothing* was applied), and replacements silently missed text rustfmt had re-wrapped. What works:
write the Python to a file, `assert old in s` before each replace (fail loudly), and use the Edit
tool for anything quote-heavy or already formatted. Rewriting a small file whole (as with
`shell.rs`) beats a pile of partial replacements.
M9a added one more: a multi-line `perl -0pi` regex with a repeated group (`(\s*\.bind(..))*?`)
kept only the *last* capture and silently dropped `.bind(key)` from the cache lookup; a test
caught it. After any regex rewrite of call chains, audit with
`git diff -U0 | grep -E '^[-+].*\.bind\('` (removed binds must match added ones).

### 6.4d Slow tests (measured, fixed)
Unoptimised Argon2 made every login/registration slow (auth tests 3.9 s) and ten integration-test
binaries each paid a full link. The workspace now builds `argon2`/`blake2` with `opt-level = 3` in
the dev profile (and `rnx new` apps get the same, since profiles only apply at a workspace root),
and the integration tests are one binary. Result: rebuild after a core change 29 s → 7 s, full run
19 s → 6 s. Don't run tests with `--release` (slow compile, no debug assertions).

### 6.5 Library/API traps
- **sqlx 0.9:** dynamic SQL needs `sqlx::AssertSqlSafe(string)`; `SqliteArguments` has no lifetime;
  multi-statement SQL uses `sqlx::raw_sql`. `sqlite::memory:` gives each pooled connection its own
  DB → pool of exactly 1 connection with no idle timeout (see `db::connect`). `PRAGMA foreign_keys`
  is per connection: `migrate:fresh` acquires one connection for the whole drop.
- **axum 0.8:** paths are `/{id}` and `/{*rest}`; `Option<Extractor>` needs
  `OptionalFromRequestParts`; `route_layer` only wraps routes added *before* it (so
  `.require_auth()` must come after the routes it guards); middleware futures must be `Send` — don't
  keep a closure borrowing `req` alive across `next.run(req).await` (scope it in a block).
- **MiniJinja 2.24:** `eval_to_state` is deprecated → `render_captured_to(ctx, io::sink())` +
  `with_state_mut(|s| s.render_block(..))` for fragments. `merge_maps([a, b])`: **first wins**.
  `tojson`/`urlencode` need the `json`/`urlencode` features (enabled). Booleans render as
  `True`/`False` — use `{% if %}` in templates/tests. `{{ none }}` handling is left to the default
  formatter.
- **argon2 0.6:** `Argon2::default().hash_password(bytes)` (needs default `getrandom` feature),
  verify with `password_hash::phc::PasswordHash`. Hashing runs in `spawn_blocking`. Unknown emails
  are verified against a static dummy hash to equalise timing.
- **Crypto crate versions:** our `sha2 0.11` + `hmac 0.13` share `digest 0.11`; `cookie` pulls the
  older `sha2 0.10/hmac 0.12` — that's fine, but don't mix types across them.
- **Clippy (Rust 1.98)** wants let-chains (`if let … && cond {}`) instead of nested ifs.
- **`cargo doc --workspace` output collision:** a binary and a library with the same name
  (`renox`) wrote to the same `target/doc/renox`, failing CI randomly. Resolved by the `rnx` rename.
- **Windows CI:** Git checks out with CRLF, so `include_str!` content ends in `\r\n`; compare
  trimmed text in tests.
- **Futures and `Send`:** `Validator::rules_of` applies rules synchronously before the async DB
  checks so `Valid<T>` doesn't require `T: Sync`. Trait methods returning futures are declared
  `-> impl Future<Output = …> + Send`.
- **`Option<T>` has a std `inspect` method,** so call `FieldValue::inspect(&opt)` explicitly in
  tests; generic code in the validator is unaffected.
- **`MultipartError` already carries the right status** (e.g. 413 over the body limit): return
  `err.into_response()`, don't wrap it in `Error::BadRequest`.
- **Test HTTP clients must update the session cookie from every response** — flashed errors and
  old input live in the cookie set by the redirect.
- **Scripted edits after rustfmt missed again in M6c** (guestbook `.label()` calls); grep caught it
  this time. Prefer the Edit tool on freshly read content for anything rustfmt may have wrapped.
- **Listener order:** `App::listen` listeners run before modules' (modules register at boot).
- **`Error`'s `Debug`** is hand-written so `fn main() -> renox::Result` prints readable errors, not
  `Internal(…)`.

### 6.5b PostgreSQL traps (M9b)
- sqlx decodes strictly: `i64` needs `BIGINT` (a plain `INTEGER` column is INT4 and fails), and
  `SELECT 1` is INT4 (`/health` returned 503 until it stopped decoding the ping). `SUM(bigint)` is
  `NUMERIC`: write `CAST(SUM(x) AS BIGINT)`.
- sqlx sends parameters in binary with a declared type: a text-typed `NULL` or date string can't
  go into a `TIMESTAMPTZ`, hence the typed `DbValue` variants and the OID-0 untyped `NULL`.
  (Sending text with OID 0 does *not* work for non-text columns: the bytes are binary-format.)
- PostgreSQL keeps microseconds: `db::now()` truncates to them, or a saved model won't equal the
  row read back.
- `citext` doesn't help: `citext_col = $1` with a text parameter compares as text (case-sensitive).
- Under a parallel test suite a PostgreSQL round trip is ~0.2 s; timing-based tests need slack on
  PostgreSQL (see `background_workers_pick_up_jobs_and_stop_cleanly`). The worker also now arms
  `Notify::notified()` *before* querying, so a dispatch during the query isn't lost.
- `pgrep -f`/`pkill -f` with a pattern that appears in your own command line kills your shell
  (exit 144) — happened again in M9b. Use the PID files.

### 6.5c Pre-1.0 audit (read before M13/M14)
- Findings with IDs (W* web, D* data/background, A* Laravel gaps) and the chaos baseline are in
  `docs/audit/2026-09-pre-1.0.md`. ROADMAP M13/M14 checklists use the same IDs.
- The probes are committed on **local** branches `probe-web` (probe_web.rs) and `probe-data`
  (probe_data.rs, probe_bg.rs, examples/probe-chaos): `git worktree add ../x probe-web`. They are not
  pushed and not on main; when an item is fixed, move its probe to main as a passing test.
- M13a moved all web probes to main as `tests/it/web_security.rs` (names without `probe_`).
  `probe-data` remains the source for M13b/M13c.
- `target/` grows to ~100 GB over a few milestones and fills the disk (link errors, "No space left
  on device"); `cargo clean` it before the full two-database run.
- This session's working directory (~/workspace/renoxium) isn't a git repo, so the Agent tool's
  `isolation: "worktree"` fails there; create worktrees by hand with `git -C ~/workspace/renox
  worktree add …` and point agents at them.

### 6.6 Security incidents and rules
- The owner once pasted a crates.io API token into chat; it was used (with his explicit authority)
  to publish the placeholders, and he was told to revoke it. Never store tokens; prefer the user
  running `cargo login` themselves.
- The user's email must not be sent to third-party services (it was once put in a crates.io
  User-Agent; don't repeat that — use a neutral UA like `renox-deps-check`).
- crates.io publishing needs a verified email on the account.

## 7. Where things stand (update this section when it changes)

| Milestone | Status |
|---|---|
| M0 foundation, M1 web layer, M2 database, M3 validation, M4 auth (a+b), M5 queue/scheduler/events/mail/notifications (a+b) | merged to `main` |
| M6a cache, `Routes::throttle`, maintenance mode (`down`/`up`), `/health` | merged to `main` |
| M6b uploads, file rules, storage (local + `s3` feature), multipart CSRF, body limit | merged to `main` |
| M6c i18n (`resources/lang`, `t()`, `Lang`, per-visitor locale, translatable built-ins) | merged to `main` |
| M7 generators (`make:*`), `route:list`, `db:shell`, browser live reload | merged to `main` |
| Faster tests (argon2 opt-level, one integration-test binary, per-app error detail) | merged to `main` (#15) |
| M8a testing helpers | merged to `main` |
| M8b single-binary deploys (`embedded!()`), `rnx build`, `rnx make:deploy` (Docker/systemd/Litestream) | merged to `main` (#18) |
| M9a Renox's own database layer (`Db`, `Transaction`, `Row`, `db::sql`, `postgres` feature) | merged to `main` (#19) |
| M9b PostgreSQL backend proper (dual-dialect migrations, typed binds, SKIP LOCKED, schedule claims, `rnx new --database postgres`, CI, guide) | merged to `main` (#20) |
| M10a CHEATSHEET.md (doctested), llms.txt, AGENTS.md/CLAUDE.md in new apps, `examples/crud` | merged to `main` (#21) |
| M10b method spoofing, all validation errors at once, `can()` for policies, pagination keeps query | merged to `main` (#22) |
| M11a security headers, CSP (relaxed default / strict with nonce + Alpine CSP build / off), CORS per route, `without_csrf()` | merged to `main` (#23) |
| README rewrite (tagline, why, demo GIF, compiled examples, comparison) | merged to `main` (#24) |
| M11b webhooks (`impl Webhook`, `webhook_calls`, signature helpers, `examples/webhooks`) | merged to `main` (#25) |
| M11c SEO & analytics (`seo()`, robots/sitemap, Search Console, GA4/GTM, events, Measurement Protocol) | merged to `main` (#26) |
| M10c examples api / jobs / uploads / postgres; JSON errors for API clients; JSON bodies report all errors; `User::attempt`; `post_multipart` | merged to `main` (#27) |
| M10d doctests on public APIs (all 35 renox-core examples compile) | merged to `main` (#28) |
| M12 types end to end (checkbox, datetime-local, multi-select, `DbEnum`, `Json<T>`, `uuid` feature, `examples/fields`, docs/types.md) | merged to `main` (#29) |
| Pre-1.0 audit (Laravel gaps, negative flows, chaos) → docs/audit/2026-09-pre-1.0.md; plan M13 + M14 in ROADMAP | PR from branch `pre-1.0-plan` |
| M13a web security (W1–W18): sandboxed user files, `ClientIp` + `TRUSTED_PROXIES`, logout revokes sessions, 3-way login lock, same-site redirects | PR from branch `m13a-web-security` |
| M13b resilience, M13c regression + chaos suite (ROADMAP M13, IDs D* in the audit) | next |
| M14 API freeze (ROADMAP M14, IDs A*) | after M13 |
| v1.0 docs site, starter kit, semver guarantee | last |

Before starting work, check open PRs with `gh pr list -R arif-rachim/renox` and base new branches on
an up-to-date `main`. Open the next milestone's PR only after the previous one is merged (§6.3).

Open items noted in ROADMAP: `#[derive(Validate)]`, more rules (regex, dates, files), route groups
with prefixes (M14), server-side sessions.

Stats at the time of writing: ~11.8k lines of Rust in `crates/`, 140 tests, 34 direct dependencies
(stars and roles were reviewed with the owner; keep deps lean and remove unused ones).
