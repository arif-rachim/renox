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
  src/lib.rs               `pub use renox_core::*`, `pub use renox_macros::{Model, migrations}`, prelude
  tests/                   integration tests that need the derive/migrations! macros (see §7)
    migrations/, migrations_plain/   SQL fixtures for tests
crates/renox-core/         ALL runtime code (see §3 for why one crate)
  src/app.rs               App builder, boot(), Kernel, app-binary commands (migrate, queue:work…)
  src/config.rs            Config from env/.env (see §5)
  src/state.rs             AppState (Clone): config, routes, views, db, mailer, queue, listeners,
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
  src/db/                  pool (mod.rs), Model trait (model.rs), Query builder (query.rs), DbValue
                           (value.rs), Paginated/Page (paginate.rs), migrator (migrate.rs), Factory
  src/validation/          Validator/rules (mod.rs), Valid<T> extractor (extract.rs), en/id messages
  src/auth/                User, hashing, login/logout, CurrentUser middleware, AuthUser, guards,
                           Policy/gates (mod.rs), Auth module + pages (module.rs), password reset,
                           verification, API tokens, throttle, notifications
  src/queue/               Job trait, Queue (dispatch), Worker
  src/schedule.rs          Schedule + runner; APP_TIMEZONE offsets
  src/events.rs            Event, listeners, AppState::emit
  src/mail.rs              Mail, Mailer (smtp/log/memory), mail_view, queue_mail, /_renox/mail preview
  assets/                  vendored htmx.min.js (2.0.11), alpine.min.js (3.17.4)
  views/                   built-in templates (error, pagination, auth/*, mail/*) — see §4.4
  migrations/              framework-owned migrations (auth/*, queue/*) — see §4.6
  tests/                   core integration tests (support/mod.rs has TestApp)
crates/renox-macros/       proc macros: #[derive(Model)], migrations!()
crates/renox-cli/          `rnx`: new, serve, key:generate, make:migration, forwards everything else
  stubs/                   files `rnx new` writes (Cargo.toml.stub, env.stub, build.rs, views…)
examples/hello/            guestbook app exercising every feature; used for live/browser testing
.github/workflows/ci.yml   fmt+clippy+doc (Ubuntu) and tests on Ubuntu/macOS/Windows
```

## 3. Architecture and the decisions behind it

### Request pipeline (outermost first)
`TraceLayer` → `session::middleware` (loads/saves encrypted cookie) → `auth::middleware` (loads the
current user once per request from session or `Authorization: Bearer`, inserts `CurrentUser` and
`AppState` into request extensions) → `csrf::middleware` → `view::middleware` (renders `View`
responses, error pages, turns `ValidationError` into redirect-back for plain forms) → routes.
`assets::router()` (`/_renox/*.js`) is merged after the layers, so it skips sessions.
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
  schedule:work|help`, default `serve`. Migrations/jobs are compiled into the app, so only the app
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
- **Workers and scheduler run inside `serve`** by default (single-process deploys). Multiple
  instances would duplicate scheduled tasks → `SCHEDULER=false` on all but one.
- **Mail:** lettre with rustls (no OpenSSL). Drivers `smtp`, `log` (default), `memory` (tests).

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
Auth pages/mails take a `text` object from `auth/module.rs::text(locale)` (English and Indonesian);
add keys to **both** locales. Validation messages live in `validation/messages.rs` (keys like
`required`, `min.string`, `auth.failed`). Labels for auth fields are localised via
`module.rs::label()` using `Validator::locale()`.

### 4.5 Migrations owned by the framework
Names start with `0001…` so they sort before app migrations (`2026…`):
`00010101000000_create_users_table`, `…000001_create_password_reset_tokens_table`,
`…000002_create_personal_access_tokens_table`, `…000003_create_notifications_table` (Auth module)
and `00010101000100_create_jobs_table` (every app, registered in `App::boot`). Adding a framework
migration changes migration counts asserted in `crates/renox/tests/database.rs`.

### 4.6 Tests
- Tests using `#[derive(Model)]` / `migrations!()` must live in `crates/renox/tests/` (the macros
  emit `::renox::` paths). Core-only tests go in `crates/renox-core/tests/` or unit tests.
- HTTP tests drive `kernel.router()` with `tower::ServiceExt::oneshot`, keeping the session cookie
  by hand. For CSRF, add a route returning `session.token()` (e.g. `/token`) — `/login` redirects
  logged-in users, so it can't be used to read the token after login.
- `Kernel` helpers: `migrate()`, `run_jobs()` (drains the queue), `mailer().sent()` (memory driver),
  `state()`, `db()`, `worker(queues)`.
- The process-wide debug flag in `error.rs` means debug-on and debug-off error tests need separate
  test binaries (`production_errors.rs`).

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
production; `base64:…`, `rnx key:generate`), `APP_HOST`, `APP_PORT`, `APP_LOCALE` (en|id),
`APP_TIMEZONE` (`UTC` or offset like `+07:00`; IANA names are rejected), `VIEWS_PATH`
(resources/views), `PUBLIC_PATH` (public), `SESSION_LIFETIME` (minutes, 120), `SESSION_COOKIE`,
`REMEMBER_LIFETIME` (minutes, 43200), `DATABASE_URL` (sqlite://storage/app.db),
`DATABASE_POOL_SIZE`, `MAIL_MAILER` (smtp|log|memory), `MAIL_HOST`, `MAIL_PORT`,
`MAIL_ENCRYPTION` (tls|starttls|none), `MAIL_USERNAME`, `MAIL_PASSWORD`, `MAIL_FROM_ADDRESS`,
`MAIL_FROM_NAME`, `QUEUE_WORKERS` (2; 0 = none in serve), `SCHEDULER` (true).
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
- **Listener order:** `App::listen` listeners run before modules' (modules register at boot).
- **`Error`'s `Debug`** is hand-written so `fn main() -> renox::Result` prints readable errors, not
  `Internal(…)`.

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
| M0 foundation, M1 web layer, M2 database, M3 validation, M4 auth (a+b) | merged to `main` |
| M5a queue/scheduler/events + removal of unused `thiserror`/`futures-util` | PR #9 open (branch `m5-queue`) |
| M5b SMTP mail, templates, `/_renox/mail`, notifications, this guide | branch `m5b-mail` pushed, **no PR yet** — open it (base `main`, rebased) after #9 is merged |
| M6 infrastructure: cache, storage/uploads (+ multipart CSRF), i18n, rate limiting, maintenance mode, `/health` | next |
| M7 CLI/DX (`make:*`, `route:list`, `db:shell`, browser live reload), M8 testing helpers + deploy (`renox build` embedding views, Docker/systemd, Litestream), v1.0 docs | later |

Open items noted in ROADMAP: `#[derive(Validate)]`, more rules (regex, dates, files), route groups
with prefixes, SQLite session driver, pagination links that keep other query params.

Stats at the time of writing: ~8.2k lines of Rust in `crates/`, 100 tests, 34 direct dependencies
(stars and roles were reviewed with the owner; keep deps lean and remove unused ones).
