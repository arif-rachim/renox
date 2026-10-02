# Changelog

Notable changes to Renox. From 1.0 the project follows [semantic versioning]; see
[docs/stability.md](docs/stability.md) for what counts as a breaking change. Until the first
release on crates.io, apps made by `rnx new` are pinned to a commit, and this file lists
changes by milestone (each one pull request; details in its description and in
[ROADMAP.md](ROADMAP.md)).

[semantic versioning]: https://semver.org

## Unreleased

### M28e · Data grid: related columns, an advanced filter, remembered state, polling

- New: `Column::related`, `Column::count_of`, `Column::sum_of` (values from other tables,
  sorted, filtered and searched), `Grid::advanced_filter` (rules with operators per kind, all
  or any), `Grid::remember` (the grid's state in the session) and `Grid::poll`.
- Changed: grid sorting puts empty values last in either direction, on both databases.

### M28d · Data grid: cards on phones, more kinds of cells

- New: `Grid::cards_on_mobile()`: rows as cards on phones, with sorting and filters in the
  toolbar.
- New: `Column::image`, `Column::color`, and `badges`, `icons`, `description`, `tooltip`,
  `wrap`, `limit`, `link`, `copyable` and `round` on columns (`grid::Kind::Image`/`Color`).

### M28c · Data grid: summaries and groups

- New: `Column::summary` (`grid::Summary`: sum, average, range, count) in a footer that stays
  at the bottom, over every filtered row; `Grid::groups` / `Grid::group_by`: rows grouped with
  a folding heading and the group's own summaries.
- Fixed: a grid column whose value the row doesn't have shows empty instead of failing the
  page.

### M28b · Data grid: selecting rows, bulk and row actions

- New: `Grid::bulk_action` with row checkboxes, a "select all matching" choice and
  `Grid::selected` (`grid::Selection` from the form); `Grid::row_action` for each row's menu;
  `grid::Action` (`new`, `link`, `method`, `confirm`, `danger`) with a confirmation dialog.

### M28a · Data grid: search, filter chips, row links, several grids on a page

- New: `Column::searchable()` and the toolbar's search box (every word, any searchable column,
  as you type); the active filters as chips that clear one at a time.
- New: `Grid::row_url` (a click on a row opens it), `Grid::empty_state`, and `Grid::prefix`
  for several grids on one page; a grid keeps the page's other query string values.

### M27d · Data grid: moving and resizing columns

- New: drag a heading to move its column; drag a heading's edge to resize it (double-click for
  the automatic width, arrow keys from the keyboard). Widths are kept per user with the other
  column choices (`GridPrefs::widths`).

### M27c · Data grid exports

- New: `Grid::exports()` and `Grid::export`: CSV, Excel and a print page (for PDF) of every
  row the filters match, in the user's columns.
- New: the `xlsx` feature (`rust_xlsxwriter`, off by default) for Excel exports, with merged
  headings, typed numbers and dates, and frozen panes.

### M27b · Data grid: details, editing, row order, merged cells

- New: `Grid::audit()` opens who created and last changed a row (and when) under it;
  `Grid::details()` adds what the page draws for `_details`.
- New: `Column::editable()` with `Grid::edit_url`: cells edited in place (double-click, Enter,
  F2) or a whole row in edit mode, sent as `PATCH` and validated by the app's `Valid<T>`,
  errors shown in the cells.
- New: `Grid::reorder(column, url)`: rows dragged (or moved with the arrow keys) into order
  while sorted by that column, saved with `grid::RowOrder::save`.
- New: `Column::merge()`: neighbouring equal values share one cell, nested from left to right.
- `Grid::sort_by` takes several keys (`"region,city,-total"`); the default sort no longer
  goes into the URL.

### M27a · Data grid

- New: `renox::grid`, a data grid for dashboards: `Grid` and `Column` in Rust (text, number,
  money, date, datetime, bool, select, tags and custom columns), `GridRequest`, and the `grid`
  macro of `renox/grid.html`. It fills its container with only the rows scrolling, filters each
  column by its kind from the query string (text with `%` patterns, ranges, a date range
  calendar, choices), sorts and pages on the server, groups headings (`Column::under`), shows
  different columns on phones and desktops, and freezes columns left or right.
- New: the column menu's choices are kept per user in the new framework table
  `grid_preferences` (migration `00010101000220_create_grid_preferences_table`, in every app),
  or in the session for guests (`POST`/`DELETE /_renox/grid/{grid}/prefs`).
- New: `{{ sparkline(values) }}`, a small line or bar chart as inline SVG.
- Bundled: Cally 0.9.2 (MIT), the calendar web components the date filters use
  (`renox::CALLY_VERSION`).
- New example: examples/grid, a sales dashboard.

### M26c · Tests for the CLI and the weak spots

- New: `App::run_args(["migrate:status"])` runs any command of the app binary from code (a
  test, or a program driving the app), as `my-app migrate:status` would.
- Fixed: a handler taking `renox::Path` on a route without that parameter answered 404; that
  is the app's mistake, so it is a 500 now. A value that doesn't parse is still a 404.
- Tests for `rnx`: argument parsing, `make:module --resource` (fields, plurals, every file
  written and registered), `rnx new` (every placeholder filled, PostgreSQL apps), the
  `serve` fingerprint, `key:generate`'s `.env` edit, Tailwind detection. Tests for the
  binary's built-in commands, `DbError`'s questions and `db::Json`.

### M26b · Docs brought up to date

- Every public item has a doc comment now (379 were missing, mostly struct fields and
  methods); `#![warn(missing_docs)]` keeps it that way in CI.
- `RedirectExt` is sealed and `db::InvalidUlid` is `#[non_exhaustive]` (neither was meant to
  be implemented or built by apps).
- Guides, README, CHEATSHEET, llms.txt, the example READMEs and the files `rnx new` writes
  now cover M22–M25: model keys, savepoints, `Encrypted`, domain and fallback routes,
  `route_is`, `class_names`, loops, plural ranges, factory states, `#[derive(Validate)]` and
  `detect_locale`.

### M26a · Key fixes, and examples for M22–M25

- Fixed: `insert_many` and `upsert` left the key out for `Ulid`, `Uuid` and `String` keys (a
  NULL key on SQLite, an error on PostgreSQL). They now make ULIDs and UUIDs, write `String`
  keys (a missing one is an error), and `upsert` may use `id` as its conflict target for
  such keys.
- Fixed: `unique(…).ignore(id)` took only an `i64`; it takes any key now.
- Fixed: a `Routes::fallback` answer became a 404 whenever the app had a `public/`
  directory (tower-http's `not_found_service` overrides the status); a fallback's own
  status (a redirect, a 200 page) is kept now.
- examples/crud: the form derives `Validate` with a `prepare` hook; `products:import` imports
  a CSV in one transaction with a savepoint per line; the seeder uses factory states and a
  sequence.
- examples/api: products are keyed by `Ulid` (public ids, and the list's cursors).
- examples/shop: "recently viewed" with `session.push` and `{% break %}`, stock texts with
  plural ranges, `class_names` on sold-out cards, `Redirect::route` after checkout and in the
  admin.
- examples/teams: each team's public page on its own host (`Routes::domain("{team}.localhost",
  …)`, `DomainParams`, a domain fallback), with a `slug` column (a new migration).

### M25 · Derived validation and the browser's language

- `#[derive(Validate)]`: rules as attributes on the form's fields,
  `#[validate(required, max = 100, unique("users", "email"))]`; `each(…)`, `distinct`,
  `rename = "…"`, `label = "…"`. `#[validate(hooks)]` with `impl ValidateHooks` for
  `prepare`, `authorize` and `after`. `impl Validate` by hand still works.
- `App::detect_locale()`: visitors who haven't chosen a language get their browser's
  (`Accept-Language`) when the app has texts for it; responses carry
  `Vary: Accept-Language`.
- `rnx make:module --resource` writes the form with `#[derive(Validate)]`.
- examples/hello: its form uses the derive, and the guestbook follows the browser's
  language until a visitor picks one.

### M24 · Laravel's leftovers from M21

- `Routes::domain("admin.example.com", routes)` and `Routes::domain("{account}.example.com",
  …)` with the `DomainParams` extractor: routes for other hosts, where the same path may
  mean another page. A host that matches a domain gets that domain's routes (plus Renox's
  own and the public files); other hosts get the routes without a domain. `route:list`
  shows a DOMAIN column when there are any.
- `Routes::fallback(handler)`: what answers when no route and no public file does.
- `route_is('admin.*', …)` and `request.route` in views; the `CurrentRoute` extractor.
- `Redirect::route("products.show", &[&id])?` and `Redirect::intended(&session, "/")`
  (`RedirectExt`, in the prelude).
- `session.push(key, value)` and `session.increment(key, by)`.
- `Product::factory().count(3).state(f).sequence(|i, p| …).create(&db)` (and `make`,
  `make_one`, `create_one`).
- Plural ranges in translations: `"{0} Sold out|[1,5] Only :count left|[6,*] In stock"`.
- `{% break %}` and `{% continue %}` in templates, and `class_names('tab', {'active': on})`.
- `RouteInfo` has a `domain` field (it's `#[non_exhaustive]`); `RouteTable::name_of`
  (new) takes the domain.
- examples/shop's admin nav marks its section with `route_is`.

### M23 · Savepoints and encrypted fields

- `Transaction::savepoint(|tx| Box::pin(async move { … }))`: a transaction inside the
  transaction; on `Err` only its changes are undone and the transaction goes on (on
  PostgreSQL too, after a failed statement). Savepoints nest.
- `renox::db::Encrypted<T>`: a model field stored encrypted with `APP_KEY` (AES-256-GCM,
  sealed JSON in a `TEXT` column) and read as `T`; `Option<Encrypted<T>>` for nullable
  columns. Works in handlers, jobs, commands, seeders and tests alike: the app's `Db` carries
  the key. Its `Debug` hides the value; the column can't be searched.
- `DbValue` has a new variant, `Encrypted` (it's `#[non_exhaustive]`).
- examples/teams keeps its webhook secret in an `Encrypted<String>` field.

### M22 · Model keys other than integers

- A model's key is its `id` field's type: `i64` as before, or `renox::db::Ulid` and `Uuid`
  (renox's `uuid` feature; a v7), both made on insert, or a `String` the app sets. `Model` has
  `type Key: ModelKey`; `id()`, `set_id`, `find`, `find_or_404` and `find_many` use it.
- `renox::db::Ulid`: sortable 26-character ids (monotonic within a millisecond), stored as
  text, serialized as text; a malformed one in `Path<Ulid>` is a 404. `renox::uuid` is
  re-exported with the `uuid` feature.
- `Model::insert`: always an INSERT, with the key set or a new one.
- Relation loaders are generic over keys: `belongs_to`, `has_many`, `count_many`, `sum_many`
  and `Morph` return maps keyed by the parent's key; `Pivot<L = i64, R = i64>` names the two
  sides' key types (`Pivot` alone is unchanged).
- `chunk` and `cursor_paginate` work with any key type (cursors are the key's text).
- `rnx make:model Invoice --key ulid|uuid|string [-m]` writes the model and its migration.
- examples/fields keys its products by `Uuid` instead of an extra `public_id` column (run
  `migrate:fresh` there).
- **Breaking:** generic code over models that uses `id()` as an `i64` needs
  `M: Model<Key = i64>`. `ForeignKey` takes the key type (`ForeignKey<K>`). Hand-written
  `impl Model` blocks add `type Key = i64;`. `create` now always inserts: a model created with
  a non-zero `id` keeps it (it used to update that row).

### After M21 · Small fixes

- `/favicon.ico` answers `204 No Content` (cached for a day) unless the app has
  `public/favicon.ico`. Browsers ask for it on every site; the 404 it got before ran the
  whole middleware stack, rendered the error page and logged a console error each time.

### M21i · The examples' tests, typed commands and the stubs

- Fixed: `TestApp::travel` didn't reach the in-memory rate limits (`Routes::throttle`,
  `throttle_by`) or the login lock: they measured time with `Instant`. They now read Renox's
  clock.
- Fixed: after `TestApp::travel` past the session lifetime, `TestApp`'s CSRF token and
  `acting_as` still used the old session, so the next form post got a 419; they now see the
  moved clock, as the server does.
- examples/jobs: `App::report` posts errors to a chat webhook (`ERROR_WEBHOOK_URL`); tests use
  time travel for retries and unique jobs, `fake_events`, `fake_notifications` and
  `fake_http` for the reporter.
- examples/api, shop, hello: tests use `travel` instead of rewriting dates in SQL, plus
  `assert_view`, `assert_json_path` and `assert_json`; shop runs its daily task by name.
- examples/hello: `entries:prune` is a typed command that asks before deleting (`--force`).
- examples/fields: colours checked with `each` + `one_of` and `distinct`.
- `rnx new`: `tests/home.rs` shows `assert_view` and time travel; `.env` lists `APP_HOST`,
  `DATABASE_POOL_SIZE`, `SESSION_LIFETIME`, `REMEMBER_LIFETIME`, `SESSION_COOKIE` and the
  paths; `AGENTS.md` lists every guide and adds traps about the clock and test fakes.

### M21h · The examples on the UI kit

- examples/shop and examples/teams: every page on the UI kit (navigation bar with an account
  menu, kit fields and tables, badges, toasts instead of flashed messages, confirmation sheets
  for deleting and cancelling, live validation, an error page in the layout); the shop
  rebrands the kit's accent. `route()` query arguments for filter links; teams' secret uses
  `renox::random_token()`.
- examples/htmx-recipes: an out-of-band count (`.fragment("row").also("count")`),
  `HxRetarget`/`HxReswap` for a duplicate task, toasts over htmx, `route()` for the scroll
  loader.
- Fixed: toasts with non-ASCII text (curly quotes, accents, emoji) were dropped from htmx
  responses, because a header can't hold them raw; `HX-Trigger` now escapes them in its JSON.
- Fixed: a toast returned with `HxRefresh` was sent in `HX-Trigger` and lost on the reload; it
  now waits in the session, as with `HxRedirect`.
- Fixed: error pages didn't get `App::share` values, so a layout using one (a cart count)
  failed to render them with `APP_DEBUG` (Renox's page showed instead).

### M21g · Database sessions and deploys without refused connections

- `SESSION_DRIVER=database`: sessions in a `sessions` table (new framework migration
  `00010101000210`), the cookie holds only an id; a new id at each login and logout; rows keyed
  by the id's SHA-256; `user_id` column. Cookie sessions carry over when switching.
  `session:prune`, `Session::prune_expired(&db)`.
- Every app gets the `sessions` table (with either driver): run `migrate` after upgrading.
- `serve` accepts a listening socket from systemd (socket activation, `LISTEN_FDS`). `rnx make:deploy` writes
  `deploy/<app>.socket`; its README explains deploys without refused connections, migrations
  that old and new code both accept, and two copies behind Caddy.
- `Toast` and `ToastKind` are `#[non_exhaustive]` (build toasts with `Toast::success(…)` and
  friends).
- Docs and examples checked against M21: the socket recipe stops the service before enabling
  the socket (systemd can't listen while the app holds the port); the shop admin's product delete
  asks in the kit's confirmation sheet (its `hx-confirm` never ran); the shop's order policy
  checks the admin role itself and uses `users_with_role`; crud's layout shows flashed messages;
  `rnx make:mail` prints how to actually send the mail; authorization.md, stability.md, the README,
  llms.txt and the ROADMAP no longer describe pre-M21 limits.

### M21f · Form requests, more rules, auth pages on the kit

- Form requests: `Validate` gains optional `prepare`, `authorize` (403 before the rules) and
  `after` (async checks once the rules pass, errors shown on the field), with `FormContext`.
- Rules: `alpha`, `alpha_num`, `alpha_dash`, `lowercase`, `uppercase`, `starts_with`,
  `ends_with`, `uuid`, `ip`, `size`, `required_without`, `prohibited_if`,
  `Validator::distinct`; English and Indonesian messages.
- Renox's sign-in and account pages use the UI kit (and have stacks). Deleting the account asks
  in a kit sheet instead of a browser dialog. The kit's `input` takes `id=`.
- examples/teams adds members through a form request.
- **Changed:** apps that override `renox/auth/*.html` keep their files; apps that styled the
  old markup (`.card`, `.error`, `.status` in `renox/auth/layout.html`) should restyle for the
  kit's classes.

### M21e · Tailwind, stacks and typed commands

- Tailwind CSS without Node: `rnx new --tailwind`; an app with `resources/css/app.css` gets
  Tailwind in `rnx serve` (watch) and `rnx build` (minified) into `public/css/app.css`. `rnx
  tailwind [--watch] [--minify]`, `rnx tailwind:install`. The standalone CLI (v4.3.3) is
  downloaded once and checked by SHA-256; `TAILWIND_BIN` and `RNX_CACHE_DIR` override.
- Stacks: `{{ stack('scripts') }}` in the layout; `{% call push('scripts') %}…{% endcall %}`,
  `prepend`, and `once='key'` from pages, blocks and components. `rnx new`'s layout has
  `stack('head')` and `stack('scripts')`.
- Typed commands: `impl AppCommand` on a clap `Parser`, `App::typed_command::<T>()`; `--help`,
  argument errors with the usage. `renox::clap` is re-exported. `rnx make:command` writes one.
- `renox::prompt`: `ask`, `ask_or`, `secret`, `confirm`, `choice`; `answering(…)` for tests.
- examples: shop's `shop:make-admin` is typed and asks for a missing email; crud's form pushes
  into the head.

### M21d · Errors, logs and debugging

- A request id per request: kept from a proxy's `X-Request-Id` when it looks like one, else
  generated; in the log span, the response header, error reports and the `RequestId` extractor.
- `LOG_FORMAT=json` (one object per line) and `LOG_FILE` (append to a file).
- `App::report(|report: ErrorReport, state| async { … })` for 500s, jobs that failed for good and
  failed scheduled tasks, run in the background.
- Error pages use the app's layout: `errors/{status}.html`, then `errors/default.html` (new in
  `rnx new`), with every page global plus `status`, `reason` and `detail`. Renox's own error
  page uses the UI kit.
- `route('name', id, q=…, page=2)`: named arguments become the query string.
- Named rate limiters: `App::rate_limiter("api", |req| Limit::per_minute(60).by(…))` and
  `Routes::throttle_by("api")`; `renox::rate_limit` is public.
- `/_renox/debug` while developing (`APP_DEBUG` and `APP_ENV=local`): the last 50 requests with
  status, time, view and SQL, flagging likely N+1 queries.
- Fixed: JSON error responses dropped the headers of the error, e.g. `Retry-After` on a 429.
- examples/crud has an error page in its layout; examples/api uses a named limiter (per user,
  per IP for guests).
- **Changed:** in the error template's context, the debug request line is `request_line` (it
  was `request`, which hid the `request` global). An app's own `renox/error.html` that printed
  `{{ request }}` should print `{{ request_line }}`. An existing `errors/default.html` is now
  used for every error status.

### M21c · Scaffolding and test tools

- `Routes::resource(path, name, Resource::new().index(..).create(..).store(..).show(..).edit(..)
  .update(..).destroy(..))` with Laravel's route names.
- `rnx make:module products --resource --fields "name:string price:money …"`: model, factory,
  migration, validated form, handlers, UI-kit views and tests. New generators: `make:factory`,
  `make:seeder`, `make:test`, `make:notification`, `make:event`, `make:rule`,
  `make:middleware`.
- `rnx new` apps use the UI kit (a navigation bar, an account menu, toasts).
- `TestApp`: `travel`, `travel_back`, `at_travelled_time`; `fake_events` (`emitted`,
  `assert_emitted`, `assert_not_emitted`); `fake_notifications` (`notifications`,
  `assert_notified`, `assert_notified_to`, `assert_nothing_notified`); `assert_session_has`,
  `assert_session_missing`, `session_get`, `assert_authenticated`, `assert_guest`; `serve()` for
  browser tests.
- `TestResponse`: `assert_view`, `json_path`, `assert_json_path`, `assert_json`, and a `view`
  field. **Changed:** code building a `TestResponse` literal must add `view`.
- docs/testing.md.

### M21b · Views, components and the UI kit

- Components see the request: `old`, `error`, `errors`, `t`, `can`, `auth`, `request`, `flash`,
  `csrf_field` work inside imported macros; `once(key)`; `rnx make:component name`.
- The UI kit `renox/ui.html` after Apple's Human Interface Guidelines, with its styles and
  script (`{{ renox_ui() }}`):
  - fields (`input`, `textarea`, `select`, `checkbox` with `switch`) and `button`/`link_button`;
  - `card`, `group`, `alert`, `badge`, `form_errors`;
  - `sheet`/`open_button`, `confirm`, `menu`, `tabs`/`tab_panel`, `table`, `empty`;
  - dark mode, WCAG AA contrast, 44 pt targets, keyboard support, reduced motion.
  `rnx make:component --ui` (`ui:publish`) copies the kit into the app. See docs/ui.md.
- `Toast` (`success`, `info`, `warning`, `error`) as a response part, and `{{ toasts() }}`.
- `View::also(block)` for out-of-band fragments; `HxRetarget`, `HxReswap`, `HxPushUrl`.
- Live validation: `<form data-live-validate>` checks fields against `Valid<T>` without running
  the handler.
- Built-in `ui.*` texts in English and Indonesian.
- examples/crud uses the kit and toasts.

### M21a · Rough edges

- `User::has_role` / `User::has_permission` (the current request's roles) for policies and
  `gate_before`; `permissions::users_with_role(&db, role)`.
- `/confirm-password` returns a guarded POST/PUT/DELETE to the page its form was on;
  `TestApp::confirm_password()`.
- `Db::retrying(n, || async { … })`: retried on conflicts, borrows from the caller, may roll
  back and return a value.
- A batch's `then`/`catch`/`finally` jobs see the batch in `JobContext::batch_id` (framework
  migration `00010101000120_add_callback_of_to_jobs`: run `migrate`); `TestApp::run_all_jobs()`.
- `renox::anyhow` re-exported; `Error::permanent_message`.
- `renox::db::capture_queries(future)` returns the SQL a future ran (requests through `TestApp`
  included).
- `Morph::count_many`; `renox::random_token()`; `renox::context::Current<T>` as a handler
  argument; seeders run in the app's context (`renox::context::app()`).
- A `ValidationError` from a model hook or a handler keeps the old input on plain forms.
- **Changed:** the prelude's `Path` is `renox::Path`: a route value that doesn't parse
  (`/orders/abc` for `Path<i64>`) is a 404 page instead of a plain-text 400.
- Examples: shop's checkout uses `db.retrying`; relations counts queries with
  `capture_queries` and likes with `Morph::count_many`; jobs uses `run_all_jobs`.

### M20c · Dashboard, localized mail, HTTP client, storage

- `renox::http` (`state.http`): get/post/put/patch/delete with query, headers, bearer/basic auth,
  JSON/form/raw bodies, timeouts, retries; `Response::json/text/error_for_status`. A new default
  feature `http` (`server-events` needs it).
- `TestApp::fake_http()`: `on(pattern, FakeResponse)`, `sent()`, `assert_sent`,
  `assert_not_sent`, `assert_sent_count`; requests without a fake fail.
- Schedule pings: `ping_before`, `then_ping`, `ping_on_success`, `ping_on_failure`.
- The queue dashboard: `.module(renox::queue::Dashboard)` at `/_renox/queue`, gated by
  `view-queue-dashboard`; `Queue::stats()` (`QueueStats`) and `Queue::recent_batches`.
- Localized mail and notifications: `t()` and `app.locale` in mail views,
  `Recipient::in_locale` / `Recipient::locale()` (a `users.locale` column is read if present),
  `Notification::channels_for(to)`, `state.mail_view_in(locale, …)`, `state.lang(locale)`,
  `state.current_lang()`, `renox::i18n::{current_locale, with_locale, set_current_locale}`.
- Mail components: `renox/mail/components.html` with `button`, `panel`, `table`, `divider`.
- Storage: `list(prefix)` (`FileInfo`), `copy`, `rename`, `size`, `delete_all(prefix)`, on the
  local disk and S3.
- examples/jobs: the charge job calls the payment gateway with `state.http` (basic auth, an
  idempotency key, 402 → permanent, 5xx → retried); unset, `PAYMENT_GATEWAY_URL` is a sandbox
  route in the example; the tests fake it with `app.fake_http()`. The admin gets the queue
  dashboard. README, docs/queue.md, docs/operations.md, llms.txt and the new-app `AGENTS.md`
  cover the HTTP client, the dashboard and localized notifications.

### Guides and examples for M18–M20b

- Guides, compiled as doctests: [docs/authorization.md](docs/authorization.md) (gates,
  policies, roles and permissions, `gate_before`, token abilities, tenants, password
  confirmation, audit) and [docs/queue.md](docs/queue.md) (retries, `dispatch_in`, priority,
  unique jobs, middleware, encrypted payloads, chains, batches, testing).
- New example `examples/teams`: a multi-tenant SaaS with a default scope that fails closed,
  the current team in `renox::context`, per-team roles on the pivot, `unscoped()` for admin
  code, names unique per team, `gate_before`, and an encrypted secret behind password
  confirmation.
- examples/jobs: payment as a chain with an encrypted, rate-limited job and a `failed` hook, a
  `high` queue, unique reminders, and statements as a batch with an htmx progress bar.
  **Changed:** the receipt goes out after payment, not when the order is placed.
- examples/crud: model hooks (a slug, a cached count) and `save_changes`. examples/relations:
  likes through `Morph`, and a query-count test. examples/shop: product delete behind
  `require_password_confirmed`, checkout in `transaction_retrying`.
- Fixed: `User::get::<bool>` on an app's own BOOLEAN column returned `None` on SQLite (stored
  as 0/1), so a `gate_before` built on it never fired.
- CLAUDE.md §4.11 and CONTRIBUTING ask every milestone to keep README, the guides, the agent
  stub and the examples in step; the parity review got a status note; the gap report PDF
  covers M20b; ROADMAP M21 lists the rough edges the examples ran into.

### Docs and examples catch-up after M20b

- Docs brought in line with M18–M20a: the README's feature tour, Laravel table and status;
  docs/operations.md (what `APP_KEY` now protects, which failure rows the chaos test really
  checks, the scheduler's time zone, tables to prune, cache locks across servers);
  docs/postgresql.md (row locks); docs/stability.md (`chrono-tz`); SECURITY.md (raw SQL
  fragments); the new-app `AGENTS.md` (traps from M18–M20a); llms.txt; CLAUDE.md.
- ROADMAP: M18/M19 items ticked but not built are corrected (no `exists_many`, no savepoints,
  no breached-password check yet) and API names fixed (`EmailVerified`, `load_with_pivot`,
  `transaction_retrying`, `create_token_with`).
- `User::set_password`'s doc says it ends this session too (use `auth::change_password`).
- Examples use the new APIs:
  - shop: roles from the `Permissions` module (`require_role`), the `Audit` module on order
    status changes, `save_only`. **Changed:** the `users.role` migration is removed; recreate a
    local shop database (`migrate:fresh --seed`).
  - api: tokens with abilities and an expiry (`create_token_with`, `require_ability`), a nightly
    token prune, `cursor_paginate`, `DELETE /api/products/{id}`.
  - jobs: daily (weekdays) and weekly (`cron`) reports in `Asia/Jakarta` with `on_failure` and
    a cache lock; counts and sums in SQL. The mail template is now `mail/sales.html`.
  - relations: `count_many`, `where_has`, a `group_by` + `select_as` report, pivot columns
    (pinned, timestamps) with `attach_with` / `update_pivot` / `load_with_pivot`.
  - hello: the account page (`Auth::new().account()`).
- Tests: M20a's cache APIs are routed in `send_handlers.rs`; the read-only storage test skips
  when run as root (root ignores directory permissions).
- `docs/audit/2026-09-laravel-gap-report.pdf`: what Renox still lacks against Laravel after
  M20a (in Indonesian).

### M20b · Queue

- Priority: `queue:work --queue high,default` drains `high` first; `dispatch_on(queue, job)`.
- Unique jobs: `const UNIQUE_FOR` and `fn unique_id`; a second dispatch returns the queued id.
- Encrypted payloads: `const ENCRYPTED: bool = true`.
- Middleware: `fn middleware(&self) -> Vec<Middleware>` with `Middleware::without_overlapping(key)`
  and `Middleware::rate_limited(key, max, per)`; a held-back job keeps its attempts.
- `async fn failed(self, state, error)` on `Job`, run once a job fails for good.
- Chains (`state.queue.chain().then(a).then(b).dispatch()`) and batches
  (`state.queue.batch(name).push(job)…then/catch/finally/allow_failures().dispatch()`,
  `batch_status` with `progress()`, `cancel_batch`).
- `state.dispatch_sync(job)`; `queue:forget`, `queue:prune-failed`, `queue:prune-batches`.
- `JobContext` has `id` and `batch_id`.
- **Migration:** a new framework migration adds `chain`/`batch_id` columns and `job_batches`;
  run `migrate`.

### M20a · Scheduler, locks and cache

- `APP_TIMEZONE` takes IANA names (`Asia/Jakarta`, `Europe/Amsterdam`) with daylight saving
  time, as well as offsets and `UTC`; `renox::timezone::Zone`. The `date` filter uses it too.
- Schedules: `cron("30 9 * * 1-5")`, `weekly_on`, `monthly_on`; per task `.weekdays()`,
  `.weekends()`, `.days(&[…])`, `.between("08:00", "17:00")`, `.timezone("…")`,
  `.on_failure(|state, err| …)`, `.on_success(…)`. `schedule:run NAME` and
  `Kernel::run_scheduled`. A duplicate task name is a boot error.
- **Changed:** schedule methods return `ScheduledTask` (derefs to `Schedule`);
  `Schedule::upcoming` takes a `Zone` and returns `(name, at, zone)`.
- Cache: `add`, `pull`, `increment`/`decrement` (atomic), `prune` and `cache:prune`; the database
  store prunes expired rows hourly on its own.
- Locks: `state.cache.lock(name, ttl)` with `try_acquire`, `block(wait)` (423 on timeout),
  `is_held`, `force_release`; `LockGuard::release` or drop.

### M19b · Model features

- Model hooks: `#[model(hooks)]` and `impl renox::db::ModelHooks` with `saving` (may stop the
  save or fill fields), `saved`, `deleting` (may stop the delete) and `deleted`. Bulk queries
  don't run them.
- `renox::context::app()`: the `AppState` of the current request, job, scheduled task or
  command.
- `Model::save_only(&db, &["col"])` and `Model::save_changes(&db, &original)` (the columns that
  differ; returns whether anything was written).
- `AppState::encrypt` / `AppState::decrypt` (AES-256-GCM under `APP_KEY`).
- Pivot data: `Pivot::with_timestamps()`, `attach_with`, `update_pivot`, `toggle`,
  `load_with_pivot::<T, PivotRow>`.
- Polymorphic relations: `relations::Morph` with `of`, `load_many` and `parents`.
- **Deferred:** non-integer primary keys and an `Encrypted<T>` field type (ROADMAP explains why).

### M19a · Query builder

- `where_raw(sql, values)`, `order_by_raw(sql)`, `group_by(col)`, `having_raw(sql, values)` and
  `select_as::<T>(db, "col, COUNT(*)")` into a `FromRow` struct or tuple; `count` counts groups.
- `to_sql(dialect)` returns the SELECT and its values.
- `lock_for_update()` / `shared_lock()` (PostgreSQL); `Db::begin_immediate()` is public (SQLite).
- `where_has(children, fk)` / `where_doesnt_have` (EXISTS), `where_not_in_query`.
- `relations::count_many` and `sum_many` (withCount / withSum, 0 for rows without children).
- `simple_paginate` (`SimplePage`, `simple_pagination` macro) and `cursor_paginate`
  (`CursorPage`, keyset on id).
- `first_or_new`, `update_or_create`, `Model::refresh`.
- `Db::transaction(|tx| Box::pin(async move { … }))` and `transaction_retrying(n, …)`;
  `DbError::is_retryable`, `Error::is_retryable`.

### M18b · Accounts and security

- `Auth::account()`: `/account` with profile (a new email is verified again), password change,
  "log out other devices" and account deletion; overridable `renox/auth/account.html`.
- `Auth::password_rules(Password::min(12).mixed_case().numbers().symbols())` for the register,
  reset and account forms; `Field::password(&policy)` for any form; `Password::min(8)` stays the
  default.
- `Routes::require_password_confirmed()` and `/confirm-password` (three hours, like Laravel).
- **Behaviour change:** `auth::logout` ends this device only (a copied cookie dies too); new
  `auth::logout_other_devices` and `auth::change_password` keep this session; `User::delete_account`.
- Auth events in `renox::auth::events` (`Registered`, `LoggedIn`, `LoginFailed`, `LockedOut`,
  `LoggedOut`, `PasswordReset`, `PasswordChanged`, `EmailVerified`, `ProfileUpdated`,
  `OtherDevicesLoggedOut`, `AccountDeleted`).
- The `Audit` module (`renox::audit`): an `audit_logs` table recording every auth event,
  `audit::record(Entry::new(..).user(..).subject(..).data(..).ip(..))`, `latest`, `for_user`,
  `for_subject`, `prune`, and `rnx audit:prune --days N`.
- Users imported from Laravel log in with their bcrypt hashes and are rehashed to Argon2id
  (`auth::needs_rehash`); `User::attempt` rehashes too.
- `TestApp::session_cookie` / `use_session_cookie` to play several devices in tests.
- New apps from `rnx new` turn the account page on and link it from the layout.
- New framework migrations: `…000006_create_revoked_sessions_table` (Auth) and
  `…000600_create_audit_logs_table` (Audit).

### M18a · Tenancy, roles, gates, token abilities

- `renox::context`: values for the current request, job, task or command (e.g. the current
  team), with `set`, `get`, `remove` and `scope`.
- Default scopes: `#[model(default_scope = "team_only")]`, applied by `query()`, `find`,
  `where_eq` and the relation loaders; `Model::unscoped()`; `Query::none()` to fail closed.
- `unique`/`exists` rules take `.where_eq(col, value)`, `.where_null(col)` and
  `.where_not_null(col)`, e.g. unique per team and ignoring soft-deleted rows.
- `Routes::require_gate("admin")` (async gates too), `App::gate_before(|user, ability| …)` for
  super-admins, applied to gates, permissions and policies.
- The `Permissions` module: roles and permissions tables, `permissions::define_role`, `grant`,
  `revoke`, `delete_role`, `roles`; `user.assign_role`, `remove_role`, `sync_roles`, `roles`,
  `permissions`; `AuthUser::has_role`, `has_permission`, `role_names`;
  `Routes::require_role`, `require_permission`; `can('…')` and `auth.roles` in templates.
- API token abilities: `create_token_with(&db, name, &["orders:read"], expires)`,
  `AuthUser::token_can`, `Routes::require_ability`, `AccessToken.abilities`; `tokens:prune` and
  `auth::prune_expired_tokens` delete expired tokens.
- `Can::new` takes a `User` or an `AuthUser` (the latter applies `gate_before`).
- examples/shop guards `/admin` with `require_gate("admin")` instead of its own extractor.
- New framework migrations: `…000005_add_abilities_to_personal_access_tokens` (Auth) and
  `…000500_create_roles_and_permissions_tables` (Permissions).

### Laravel parity review

- docs/audit/2026-09-laravel-parity.md: every Laravel feature area compared with Renox, the
  verdict, and the gaps; ROADMAP plans them as M18–M21 before v1.0.
- CHEATSHEET: `set_password` ends every session, this one too (log it in again).

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
