# Renox: guide for agents and contributors

Read this before changing anything. It records how Renox is built, why, how work is done in this
repo, and every trap hit so far, so you don't have to rediscover them.

**Read next**, depending on the task:
- `ROADMAP.md`: the plan, per-milestone notes and the "Decisions" section.
- `CHANGELOG.md`: what changed, milestone by milestone. **Read only: agents never edit it, nor
  `README.md` (§4.11).**
- `CONTRIBUTING.md`: the checks every change needs. `SECURITY.md`: how vulnerabilities are reported.
  `RELEASING.md`: how a release goes to crates.io (the owner publishes).
- `CHEATSHEET.md` and `llms.txt`: the app author's view (patterns, and which example shows what).
- `docs/*.md`: guides (routing, validation, types, relations, authorization, queue, mail, scheduling, ui, grid, testing, PostgreSQL, operations, development, stability, and the plugins: two-factor, editors, blocks, oauth, admin, billing).
- `docs/audit/`: the pre-1.0 audit (finding IDs W*, D*, A* used in ROADMAP M13/M14).

## 1. What Renox is

- A **batteries-included web framework for Rust, modelled on Laravel**: Axum + HTMX + Alpine.js +
  SQLite (PostgreSQL optional). One dependency (`renox = "…"`, `use renox::prelude::*`) gives
  routing, sessions, CSRF, views, a model layer, migrations, validation, auth, queue, scheduler,
  events, mail, notifications, cache, storage, i18n, webhooks, SEO and analytics.
- **Apps depend on Renox; framework code is never copied into apps.** Upgrading is a version bump.
  This is a hard requirement from the owner.
- **The owner** is a solo founder, fluent in Rust, who discusses and reviews in Indonesian. Everything
  in the repo is English: code, comments, docs, example content (seed data, names, pages,
  mails), tests, commit messages and PRs. No Indonesian anywhere, and no built-in Indonesian
  locale (M32): Renox ships English texts; an example that needs a second language uses
  Spanish (hello, shop). Prices are in US dollars by default (`APP_CURRENCY=USD`, amounts
  in cents); other currencies through `APP_CURRENCY` (IDR stays for Xendit/Midtrans, which
  charge rupiah).
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
CONTRIBUTING.md            checks every change needs; SECURITY.md: reporting vulnerabilities;
                           RELEASING.md: publishing to crates.io (the owner runs it)
CHEATSHEET.md              one-page patterns for app authors/agents (compiled: `CheatSheet`)
llms.txt                   map for agents: which example/guide file shows what
deny.toml                  cargo-deny: licenses, advisories, banned crates, sources
crates/renox/              facade crate apps depend on: re-exports renox-core, the macros, prelude
  src/lib.rs               `pub use renox_core::*`, macros (DbEnum, FromRow, Model, Validate, embedded!,
                           migrations!, #[renox::test]), prelude, and cfg(doctest) holders:
                           ReadMe, CheatSheet, TypesGuide, RelationsGuide, AuthorizationGuide,
                           QueueGuide, UiGuide, GridGuide, ValidationGuide, RoutingGuide,
                           MailGuide, SchedulingGuide, TestingGuide, OperationsGuide,
                           TutorialGuide, LaravelGuide, MacroCompileErrors
  tests/it/                ONE integration-test binary (main.rs + a module per area); add new areas
                           as `mod x;` in main.rs. Notable modules: send_handlers.rs (every data
                           API in a routed handler), web_security.rs, data_resilience.rs,
                           background_resilience.rs, direct.rs (APIs otherwise tested only
                           indirectly), database.rs, postgres.rs (`postgres` feature), s3.rs
                           (`s3` feature), extension_points.rs, api_foundations.rs, data_layer.rs,
                           commands.rs (the binary's built-in commands via `App::run_args`)
  tests/migrations/, migrations_plain/, migrations_types/, views/   fixtures (not under it/)
crates/renox-core/         ALL runtime code (see §3 for why one crate)
  src/app.rs               App builder, boot(), Kernel, router assembly, app-binary commands;
                           serve takes systemd's socket (listenfd, LISTEN_FDS) before binding
  src/config.rs            Config from env/.env (see §5)
  src/state.rs             AppState (Clone): config, routes, views, db, mailer, queue, cache,
                           storage, translator, listeners, key, gates/async gates, shares,
                           channels, provided values, throttle, security, webhooks, live
  src/module.rs            Module trait: name, routes, migrations, register
  src/registry.rs          Registry: jobs, listeners, schedule, commands, channels, shares, templates,
                           assets (`Registry::asset`: files served before the session),
                           provide (a module's values for `state.provided`, under the app's)
  src/routing.rs           Routes builder (get/post/…/name/group/require_auth/guest_only/
                           require_verified/throttle/cors/route_layer/merge/domain/fallback),
                           RouteTable + URLs (name_of for route_is), CurrentRoute
  src/domain.rs            Routes::domain: DomainPattern, host dispatch between routers,
                           DomainParams / MatchedDomain
  src/redirect.rs          RedirectExt: Redirect::route / Redirect::intended
  src/session.rs           session + middleware: an encrypted cookie holding the whole session
                           (cookie driver) or its id (`Stored::Handle`, database driver:
                           `sessions` table keyed by sha256(id), rotation on login/logout,
                           push/increment helpers,
                           a test mirror in `AppState::session_mirror`)
  src/csrf.rs              CSRF middleware; the XSRF-TOKEN cookie (App::xsrf_cookie) and
                           X-XSRF-TOKEN header
  src/view.rs              MiniJinja env, View response (fragment/also), render middleware, globals
                           (request.route, route_is, loop controls), RequestGlobal (request globals
                           inside imported macros), BUILTIN views
  src/toast.rs             Toast (body, ToastAction, duration, id) response part, the toast
                           region markup (positions), safe_url for links built from data
  src/clock.rs             the current time with a test offset (TestApp::travel); Stamp for
                           in-memory windows (rate limits, login lock), never Instant
  views/ui.html            the UI kit (renox/ui.html): fields, buttons, sheets, menus, tables,
                           infolists, dashboards, and the page's frame (navbar, sidebar +
                           rx-shell, page_header, toolbar, list, card_grid…); assets/renox-ui.css|js
                           (+ renox-ui-{chart,wizard,repeater,tags}.js: modules renox-ui.js loads
                           when it finds their markup; assets.rs fills their hashed URLs in, #349)
  views/grid.html          the data grid macro (renox/grid.html), grid_print.html (its print
                           export); assets/renox-grid.css|js, and
                           assets/cally.js (Cally 0.9.2, MIT: the date range calendar)
  src/view_stack.rs        push/prepend/stack: markers filled in after the page renders (Scope)
  src/icons.rs             the kit's icons: Lucide paths (lucide-static 1.53.0, ISC; assets/NOTICE,
                           assets/lucide-LICENSE) drawn by `renox_icon`, behind ui.html's `icon(…)`
  src/view_filters.rs      built-in template filters `number`, `money`, `date`, `since`, `words`,
                           `markdown`; pub format_number, format_money (whole units;
                           the `money` filter takes the smallest unit, like the grid)
  src/htmx.rs              Htmx extractor, HxRedirect/HxRefresh/HxTrigger/HxRetarget/HxReswap/
                           HxPushUrl, Back; add_trigger (JSON HX-Trigger, non-ASCII \u-escaped)
  src/assets.rs            embedded htmx/Alpine/renox.js with hashed URLs; renox.js source lives here
  src/error.rs             Error enum, IntoResponse, error pages (the app's errors/{status}.html,
                           errors/default.html with the page globals, else renox/error.html),
                           Debug for main(), panic_message
  src/crypto.rs            APP_KEY parsing/generation, random tokens, constant_time_eq
  src/signed.rs            signed URLs (HMAC-SHA256) + ValidSignature extractor
  src/db/                  conn.rs (Db/Transaction/Row/sql(), Executor, SchemaEpoch, savepoints,
                           the key for Encrypted columns), key.rs (ModelKey, Ulid), encrypted.rs
                           (Encrypted<T>, Unsealed), mod.rs
                           (connect, TEST_DATABASE_URL), model.rs, query.rs, from_row.rs,
                           relations.rs (belongs_to/has_many/has_many_through/Pivot/Morph), value.rs (DbValue),
                           paginate.rs, migrate.rs (migrator), factory.rs, json.rs, error.rs,
                           query_log.rs (capture_queries: a task-local statement log, also
                           feeding /_renox/debug), search.rs (full-text search: the index
                           migration (FTS5 + triggers / generated tsvector + GIN), the
                           filter and rank SQL behind Query::search, rebuild)
  src/path.rs              renox::Path: axum's Path with a 404 (not 400) when a value won't parse
                           (a parameter the route lacks is a 500); Found<M> (route model binding:
                           the parameter named after the table, else the only one; by key or
                           by a column named like the parameter)
  src/validation/          Validator/rules (mod.rs, ValidateHooks for the derive), Valid<T>
                           (extract.rs: prepare → authorize → rules → after, FormContext), English
                           messages, nested.rs (form names like `lines[0][qty]` read as a
                           tree), key_values.rs (KeyValues)
  src/auth/                User, hashing (Argon2id + bcrypt import), login/logout (per device),
                           change_password, CurrentUser middleware, AuthUser, guards, Access::check,
                           Policy/gates (mod.rs), Auth module + pages (module.rs), account.rs
                           (account pages and the sections modules add to them, #168; password
                           confirmation), passwords.rs (reset),
                           permissions.rs (Permissions module: roles, permissions, roles
                           per scope with dates: Scope, set_scope, #244), events.rs
                           (LoggedIn, LoginFailed, …), second_factor.rs (Registry::second_factor:
                           a module's step after the password; pending_login/complete_login,
                           #167), external.rs (sign_in / register_verified /
                           registration_open / confirm_identity: logging in another way than
                           the password, for renox-oauth, #147), verification, tokens.rs (API tokens,
                           abilities, prune), LoginThrottle (pair/account/IP), notifications
                           (Recipient, Channel::Custom, notify/notify_later,
                           SendToChannel job, DatabaseMessage, Hub), inbox.rs (the
                           notifications.* routes and the SSE stream of `.notifications()`,
                           which also carries `AppState::broadcast` events through the
                           in-process `Hub`)
  src/audit.rs             Audit module (audit_logs table, records auth events), audit::record,
                           audit:prune
  src/context.rs           renox::context: task-local values per request/job/task/command
                           (default scopes, context::app())
  src/queue/               Job trait, Queue (dispatch, chain, batch), Middleware, Worker,
                           dashboard.rs (Dashboard module, stats)
  src/schedule.rs          Schedule + runner, ScheduledTask builder, own cron parser, run claims
  src/timezone.rs          Zone (UTC / fixed offset / IANA via chrono-tz) for APP_TIMEZONE
  src/events.rs            Event, listeners, AppState::emit
  src/grid/                mod.rs: renox::grid: Grid/Column, GridRequest, filters from the query string,
                           header rows, GridPage (serialized for renox/grid.html), preferences
                           (grid_preferences / session) and their /_renox/grid/{grid}/prefs route,
                           merged cells, RowOrder; searchable/prefix/row_url/empty_state;
                           Action/Selection (bulk_action, row_action, selected); Summary
                           (Column::summary), groups/group_by; cards_on_mobile and cell kinds
                           (image, color, badges, icons, description, tooltip, wrap, limit,
                           link, copyable); related/count_of/sum_of, advanced_filter,
                           remember, poll; export.rs: CSV, Excel (`xlsx`), print page,
                           export_as (any query) + ExportFormat
  src/import.rs            renox::import: Import (CSV → rows checked as forms via
                           validation::extract::parse_pairs, written in savepoints),
                           ImportReport (IntoResponse: toast or views/import_report.html),
                           template()
  src/mail.rs              Mail (recipients, cc/bcc/reply_to/from, attachments), Mailer
                           (smtp/log/memory, MAIL_FAILOVER), named mailers (App::mailer,
                           mailer_named, queue_mail_via), mail_view, queue_mail, /_renox/mail
  src/cache.rs             Cache (memory / database store), remember(), add/pull/increment,
                           Lock/LockGuard (`renox:lock:*` rows), prune
  src/counters.rs          counters in the cache table (renox:count:…) for shared throttles/login lock
  src/provided.rs          App::provide values: Provided<T> extractor, AppState::provided
  src/cookies.rs           Cookies extractor (plain / encrypted), SetCookie response part
  src/download.rs          Download: bytes, streamed file, Storage key, stream; safe Content-Disposition
  src/command.rs           app commands: Args, Command; App::command / Registry::command, Kernel::call;
                           AppCommand (a clap Parser) + App::typed_command (renox::clap re-exported)
  src/prompt.rs            ask/ask_or/secret/confirm/choice for commands; `answering` for tests
  src/rate_limit.rs        Limiter + middleware behind Routes::throttle; named limiters
                           (App::rate_limiter: LimitRequest → Limit, Routes::throttle_by)
  src/request_id.rs        RequestId extractor + middleware (X-Request-Id)
  src/report.rs            ErrorReport/ReportKind, App::report reporters (500s, jobs failed for
                           good, failed scheduled tasks), sent in the background
  src/inspector.rs         /_renox/debug: ring buffer of the last 50 requests (views/debug.html)
  src/client_ip.rs         ClientIp extractor + TrustedProxies (TRUSTED_PROXIES); resolved once in
                           security::middleware, read by throttle, login lock, trace span
  src/maintenance.rs       down/up/status + middleware (bypass cookie)
  src/health.rs            GET /health
  src/upload.rs            Upload (multipart file field), sniffing, store/store_public, token registry
  src/storage.rs           Storage (local disk; S3 with the `s3` feature), temporary URLs, /_renox/files,
                           list/copy/rename/size/delete_all; named disks (App::disk,
                           state.disk_named, StorageConfig::from_env) served at /_renox/disks/<name>
  src/http.rs              renox::http client (reqwest behind the `http` feature) + FakeHttp
  src/i18n.rs              Translator (lang JSON files), format() with plurals and ranges,
                           RequestLocale middleware (Accept-Language with App::detect_locale), Lang
  src/live.rs              live reload: file-time polling, /_renox/live SSE, stop() on shutdown
  src/shell.rs             db:shell (run_with takes any input/output, for tests)
  src/testing.rs           TestApp / TestRequest / TestResponse for apps' tests
  src/seo.rs               seo() tags, head tags (noindex / verification / GA4 / GTM), robots.txt, Sitemap
  src/analytics.rs         analytics::event, GaClientId, ServerEvent job (`server-events` feature,
                           sent through `state.http`)
  src/webhook.rs           Webhook trait, receive route, webhook_calls store/retry, ProcessWebhook job
  src/security.rs          security headers + CSP (+ nonce), csrf-exempt and webhook route sets,
                           TRUSTED_HOSTS (400 for other hosts), ETags for Routes::etag (hashed
                           here, after the view layer rendered the page)
  src/chart.rs             renox::chart: Period (extractor), Trend (count/sum/average per day or
                           month via Query::buckets), Series; the `chart(…)` template function
  src/select.rs            renox::select: SelectOption, OptionQuery (the kit's select with
                           options_url / editable)
  src/method.rs            method spoofing layer (in front of the router)
  src/embedded.rs          Embedded (views/lang/public compiled in), public-file serving + content types
  assets/                  vendored htmx.min.js (2.0.11), alpine.min.js (3.17.4) + Alpine CSP build,
                           cally.js (0.9.2); fonts/ (Inter 4.1 and Poppins 4.003 Latin woff2,
                           OFL 1.1, served by assets.rs from /_renox/fonts)
  views/                   built-in templates (error, pagination, auth/*, mail/*, ui.html (the kit),
                           debug.html (/_renox/debug), queue/dashboard.html,
                           notifications.html (the bell's page and panel)); see §4.3
  migrations/              framework-owned migrations (auth/, permissions/, audit/, queue/,
                           cache/, session/, grid/, webhook/); see §4.5
  tests/                   core-only integration tests (support/mod.rs has a small TestApp)
crates/renox-macros/       proc macros: derive Model, FromRow, DbEnum, Validate (validate.rs);
                           embedded!(), migrations!(), #[renox::test]
crates/renox-2fa/          the first plugin (#146), a separate crate versioned with renox:
                           TwoFactor module (lib.rs: second_factor, account_section, its
                           views registered in `templates` unless the app has a file of the
                           name, events to the audit log when `audit_logs` exists),
                           handlers.rs (/two-factor/*), views/, two_factor table
                           (migrations/ there, prefix 00010101000700), totp.rs (RFC 6238 +
                           base32), recovery.rs (8 codes, SHA-256), qr.rs (SVG); its own
                           tests/ (it can use the macros: it depends on renox); guide
                           docs/two-factor.md (doctested from lib.rs `Guide`)
crates/renox-editors/      the editors plugin (#149, #150): Editors module (lib.rs: views
                           registered in `templates`, the `rich_text` filter, POST
                           /_renox/editors/preview), rich_text.rs (RichText, sanitize:
                           ammonia allowlist), assets.rs (files served with
                           `Registry::asset`), views/editors.html (rich_editor,
                           markdown_editor, code_editor, code_entry), assets/editors.js (one
                           module that imports Trix / Prism + CodeJar when a page needs them)
                           and editors.css, assets/vendor/ (pinned files + NOTICE + licences);
                           guide docs/editors.md (doctested from lib.rs `Guide`)
crates/renox-blocks/       the blocks plugin (#347): Blocks module (lib.rs: views registered
                           in `templates` unless the app has a file of the name,
                           `renox_blocks()` (the tags, once per page via `once`),
                           `renox_blocks_t(key, **params)`: the app's `t`, else the English
                           in texts.rs `TEXTS`, keys `blocks.*`), assets.rs (hashed files
                           served with `Registry::asset` under /_renox/blocks/),
                           views/blocks.html (quantity, range_slider, keypad, swatches,
                           datetime_range, gallery, history, compare_plans, month_calendar,
                           availability, kanban + kanban_card; `rx-` classes,
                           `data-rx-<block>-*` attributes: the kit owns data-rx-key/-step),
                           assets/blocks.css, assets/blocks.js (the loader: imports
                           assets/parts/<block>.js only when a page has that block; Web
                           Animations, no library); its own tests/; guide docs/blocks.md
                           (doctested from lib.rs `Guide`); browser tests
                           tests/browser/blocks.test.mjs on examples/bikeshop's /about/blocks
crates/renox-oauth/        the social login plugin (#147): OAuth module (lib.rs: providers,
                           `oauth_providers` shared with every view for the buttons, the
                           account section, events to the audit log when `audit_logs`
                           exists, views registered in `templates` unless the app has a file
                           of the name: renox/auth/login_options.html, oauth/section.html),
                           handlers.rs (/auth/{provider}/redirect|callback, DELETE
                           /auth/{provider}; state + PKCE S256 in the session key `_oauth`,
                           single use; linking rules), provider.rs (Provider trait, BoxFuture,
                           Credentials, Token, Profile, exchange_code), google.rs, github.rs,
                           model.rs (OAuthAccount, `oauth_accounts`, prefix 00010101000800,
                           no tokens stored); its own tests/ on FakeHttp; guide docs/oauth.md
                           (doctested from lib.rs `Guide`)
crates/renox-admin/        the admin panel plugin (#148, Filament's resources): Admin module
                           (lib.rs: path, title, authorize/gate, resources; views registered
                           in `templates` unless the app has a file of the name),
                           resource.rs (AdminResource trait: Model + Policy + Default, Form:
                           Validate + Serialize, columns/fields/fill/rules/entries/filters/
                           actions/query/grid/allows; AdminAction, ActionContext, Filter,
                           BoxFuture), field.rs (Field, FieldKind), entry.rs (Entry),
                           panel.rs (routes `admin.{slug}.*` in a Routes::group, generic
                           handlers behind `Extension(Ctx<R>)`; the panel's gate then the
                           resource's `allows` before `Valid<T>` is read; bulk/row actions
                           with the built-ins delete/restore/force-delete; the trash is
                           `?filter=trashed`), views/ (renox-admin/layout, dashboard, index,
                           form, fields, show; renox-admin/{slug}/cells.html from the app for
                           custom columns); its own tests/ (+ tests/migrations); guide
                           docs/admin.md (doctested from lib.rs `Guide`)
crates/renox-billing/      the subscriptions plugin (#155, Laravel's Cashier): Billing module
                           (lib.rs: plans, gateways, `Setup` given to the app with
                           `Registry::provide`, the account section, listeners: AccountDeleted
                           cancels at the gateway and deletes the rows, audit entries; views
                           registered in `templates` unless the app has a file of the name),
                           plan.rs (Plan, Interval), customer.rs (Billing::of → Customer,
                           Billable, Owner `kind:id`), gateway.rs (Gateway trait, BoxFuture,
                           Remote: only what's set changes a row, Notice, Payment, Checkout,
                           CheckoutRequest), stripe.rs (Checkout Sessions, Subscriptions API,
                           Stripe-Signature), xendit.rs (recurring plans, x-callback-token),
                           model.rs (Subscription, BillingCustomer, SubscriptionStatus;
                           tables `subscriptions` + `billing_customers`, prefix
                           00010101000900), sync.rs (apply a Remote: metadata or customer →
                           owner, `synced_at` orders webhooks, events), webhook.rs (one
                           Webhook `billing` at /billing/webhooks/{gateway}; a route layer
                           names the gateway and the event id in headers, the event id stored
                           as `{gateway}:{id}`), handlers.rs (billing.* pages), guard.rs
                           (SubscriptionRoutes); its own tests/ on FakeHttp + signed
                           webhooks; guide docs/billing.md (doctested from lib.rs `Guide`)
crates/renox-cli/          `rnx`: main.rs (key:generate, forwarding), new.rs, serve.rs, make.rs +
                           generate.rs (make:*), scaffold.rs (make:module --resource --fields),
                           deploy.rs (build, make:deploy), tailwind.rs (the pinned
                           standalone CLI: download via curl + SHA-256 check, build/watch; used by
                           new --tailwind, serve, build)
  build.rs                 sets RENOX_GIT_REV (the commit `rnx new` pins apps to)
  stubs/starter/           `rnx new --starter`: the starter kit's files, written over the
                           stubs below (same path) or next to them (`STARTER` in new.rs)
  stubs/                   the files `rnx new` writes (Cargo.toml.stub, env.stub, build.rs, src/,
                           resources/, migrations/, tests/, AGENTS.md.stub + CLAUDE.md.stub: the
                           new app's agent guide, named .stub so agents in this repo don't load
                           it); stubs/deploy/: Dockerfile, systemd service and socket (socket
                           activation), Litestream templates
examples/                  two workspace members (#351 removed the other fifteen), each with a
                           README.md and its own tests:
  hello/                   the smallest app: a guestbook (derive Validate, detect_locale, an
                           upload, a typed command); the quick start and the README's GIF
  bikeshop/                THE FLAGSHIP (epic #231): three bike stores that sell, rent and
                           service, working together; every page explains itself. src/lib.rs
                           (modules, plugins, layers, `App::report` → src/report.rs);
                           src/explain.rs (the "About this page" registry and panel) + one
                           folder per area in src/app/<area>/ (mod.rs routes, model.rs,
                           factories.rs, explain.rs: an entry per GET route, tests/about.rs
                           fails without one): about, access (permission catalogue, active
                           store, ABAC policy: owner/location/operating store), accounts, api,
                           catalog, home, multistore, plans, rentals, reports, sales, staff,
                           stock, workshop; src/seed/ (`db:seed`, `demo:seed --size large`,
                           test fixtures); the blocks from renox-blocks (#347; /about/blocks
                           shows them; public/blocks/blocks.css styles that page);
                           public/vendor/motion (motion.dev, vendored); errors/
                           (default + 503); migrations for SQLite and PostgreSQL;
                           Dockerfile + deploy/ (make:deploy); tests/<area>.rs,
                           tests/queries.rs (main pages on the large seed, no N+1),
                           tests/operations.rs; browser tests tests/browser/bikeshop-*.test.mjs.
                           Code checks permissions, never role names (tests/access.rs).
                           What the removed examples showed lives here too: each store's
                           page on its own host (home/stores.rs, `Routes::domain`,
                           `BIKESHOP_STORE_DOMAIN`), /about/fields (about/fields.rs: every
                           input ↔ Rust ↔ SQLite ↔ PostgreSQL, files public and private,
                           `Uuid` keys; docs/types.md points there; tests/fields.rs, also on
                           S3 with the `s3` feature) and /about/htmx (about/htmx.rs: the
                           htmx recipes, live)
site/                      the documentation site (package `renox-site`, publish = false): a Renox
                           app that compiles the repo's Markdown in (src/content.rs lists the
                           pages, src/render.rs: pulldown-cmark, anchors, TOC, hidden doctest
                           lines, links → /docs/{slug} or GitHub, callouts, code panels),
                           src/highlight.rs (its own syntax colours, no dependency),
                           src/icons.rs (Lucide SVGs, `icon()` in templates; each page in
                           content.rs has an icon and a one-line blurb), search, sitemap, ETags;
                           deploy/ has its systemd units; .github/workflows/release-site.yml
                           builds it for https://docs.renox.rs, whose server pulls
                           each new build (#141). A new guide in docs/ needs a line in
                           content.rs PAGES (and site/build.rs already watches docs/).
                           The owner hosts it on their own server (not GitHub Pages: the
                           account's user site maps project sites to a personal domain)
www/                       renox.rs: the landing page and blog (package `renox-www`, publish =
                           false; #337), a Renox app compiled into one binary. src/landing.rs (the
                           page's content as data, code coloured on the server with the docs'
                           highlighter), src/blog.rs (posts = content/blog/*.md with front matter,
                           compiled in by build.rs; adding a file adds the page, feed and sitemap
                           entries), src/lib.rs (routes, JSON-LD, Atom feed, sitemap, llms.txt,
                           llms-full.txt, /blog/{slug}.md), public/www.js (motion only: every word
                           is in the HTML), content/benchmarks.json (from benchmarks/run.sh; the
                           section hides while it is null), deploy/ (systemd, port 3090);
                           .github/workflows/release-www.yml builds it for the owner's server.
                           Links into the docs must name a page in site/src/content.rs PAGES
                           (tests/www.rs checks)
benchmarks/                Renox vs bare Axum vs Laravel (FPM and Octane) in Docker, pinned to
                           2 cores and 1 GB, driven by oha (#333): its own cargo workspace
                           (renox-app, axum-app; not a member of the root one), laravel/
                           (Dockerfile + overlay), run.sh, summarize.py, results/*.json,
                           RESULTS.md (the latest run; www/content/benchmarks.json copies it)
tests/chaos/               app + run.sh (postgres|sqlite) that the `chaos` CI job injects faults
                           into (docker pause/stop/restart, python3 holding SQLite's lock)
tests/cli/run.sh           `rnx new` + every `make:*`, then build and test the app (CI `cli`/`docker`);
                           with SQLite it serves the apps and drives them with tests/cli/smoke.py
                           (every GET page, a `--resource` module's forms, the starter's sign-up,
                           verification and roles over HTTP, #142), and the `rnx new` option matrix (plain, --tailwind, --starter, names
                           on both sides of "renox", #143)
tests/browser/            browser tests (#262, CI `browser`): run.sh builds the binaries one at a
                           time, then `node --test` (Node 24, no npm packages) drives headless
                           Chrome over CDP. lib/cdp.mjs (pages, clicks, keys, waitFor, settle,
                           problems: console errors, exceptions, CSP violations), lib/app.mjs
                           (start an app binary on a free port, migrated/seeded), fixture/ (a
                           workspace member: pages using the kit, Auth with notifications, jobs,
                           commands `jobs:push`/`jobs:nap`/`ask:me`, a route on its own host, a
                           per-second task with FIXTURE_TICK, and the data grid's pages at
                           /grid (src/grid.rs, its own migrations and `db:seed`: 480 orders,
                           demo@example.com); *.test.mjs for renox.js, the kit's forms and
                           overlays, the grid (grid, grid-more: the fixture's /grid), the
                           editors (on bikeshop's /about/fields), the guestbook (examples,
                           examples-flows), a11y (an accessibility smoke), assets.test.mjs
                           (no top-level JS function declared twice) and bikeshop-*.test.mjs
                           (each area of examples/bikeshop; bikeshop-walk: the main pages
                           under CSP=strict). `run.sh 'bikeshop-*'` takes quoted patterns
tests/process/            process e2e (#270): process.py (signals with a request in flight,
                           queue:work, two schedule:work, systemd's socket, LOG_FORMAT/LOG_FILE,
                           APP_KEY in production, db:shell, prompts from a pipe and a terminal
                           via `script`, the commands' output, `rnx serve`/forwarding/Tailwind
                           with a stand-in TAILWIND_BIN, `rnx build` with RNX_BUILD=1) on the
                           fixture; examples.py serves both example binaries and GETs their
                           pages as a guest and logged in; `run.sh postgres` with
                           PROCESS_POSTGRES runs the database checks and bikeshop on
                           PostgreSQL (EXTRA_ENV: per-example variables, e.g. bikeshop's
                           BIKESHOP_STAFF_2FA=optional)
tests/tutorial/           run.sh + follow.py: docs/tutorial.md followed as a reader does (steps
                           found by their lead-in sentence, never line numbers), then fmt,
                           clippy, the tutorial's tests, seed, the app answering (CI `tutorial`).
                           Changing a tutorial step's wording may need follow.py changed too
docs/ui.md                 components, the UI kit, toasts, fragments, htmx headers, live
                           validation, stacks, Tailwind (doctest `UiGuide`)
docs/grid.md               the data grid (renox::grid): columns, filters, actions, summaries,
                           exports (doctest `GridGuide`)
docs/routing.md            routes, groups, domains, extractors, middleware, guards, sessions,
                           CSRF, cookies, signed URLs (doctest `RoutingGuide`)
docs/validation.md         Valid<T>, rules, derive Validate, hooks, messages (doctest
                           `ValidationGuide`)
docs/mail.md               mail and notifications (doctest `MailGuide`)
docs/scheduling.md         scheduler, events, cache and locks, app commands (doctest
                           `SchedulingGuide`)
docs/testing.md            TestApp: requests, assertions, fakes, time travel, browser tests
                           (doctest `TestingGuide`)
docs/tutorial.md           one app (Stash) from `rnx new` to deploy (doctest `TutorialGuide`)
docs/laravel.md            Laravel → Renox, concept by concept (doctest `LaravelGuide`)
docs/types.md              HTML input ↔ Rust ↔ SQLite ↔ PostgreSQL (doctest `TypesGuide`)
docs/relations.md          relations without N+1, fetch_as/FromRow (doctest `RelationsGuide`)
docs/search.md             full-text search: #[model(search)], the index migration, ranking
                           (doctest `SearchGuide`)
docs/authorization.md      gates, policies, roles/permissions, token abilities, tenants, the
                           second login step, logging in another way (doctest
                           `AuthorizationGuide`)
docs/oauth.md              social login with renox-oauth (doctest: renox-oauth's `Guide`)
docs/blocks.md             blocks beyond the kit with renox-blocks (doctest: its `Guide`)
docs/admin.md              the admin panel with renox-admin (doctest: renox-admin's `Guide`)
docs/billing.md            subscriptions with renox-billing (doctest: renox-billing's `Guide`)
docs/queue.md              jobs, retries, priority, unique, middleware, chains, batches (doctest
                           `QueueGuide`)
docs/postgresql.md         PostgreSQL guide for app authors
docs/development.md        faster builds: profiles, linker, default features, sccache, cargo-chef
docs/stability.md          semver scope, #[non_exhaustive] types, public-dependency policy
docs/operations.md         production: timeouts, proxies, /health, failure table (kept in sync
                           with tests/chaos/run.sh), failed jobs/webhooks, backups, deploys
                           without refused connections, sessions, logs, error reports, error
                           pages, the debug inspector (doctest `OperationsGuide`)
docs/audit/                pre-1.0 audit (2026-09-pre-1.0.md, closed), Laravel parity review
                           (2026-10-laravel-parity.md, current as of M32) and its English
                           summary (2026-10-laravel-gap-report.pdf, printed from HTML by
                           headless Chrome)
docs/assets/demo.gif       the README's demo (see §4.10)
.github/workflows/ci.yml   CI jobs (see §4.11)
.github/ISSUE_TEMPLATE/    issue forms: bug.yml, story.yml (user story + acceptance criteria),
                           task.yml; config.yml (no blank issues; Discussions, security, docs)
.github/                   pull_request_template.md (the PR body's sections), dependabot.yml
                           (grouped cargo/actions updates), release.yml (release-note sections)
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
3. `request_id::middleware` (keeps an acceptable incoming `X-Request-Id` or makes one; sets it
   on the request, the response and as the `RequestId` extension), then `TraceLayer` (span
   `request` with id, method, uri, client IP; `LOG_FORMAT=json` prints it as `span`).
4. `DefaultBodyLimit` (`UPLOAD_MAX_SIZE`).
5. Merged at this level, so they skip everything below (sessions, maintenance): `assets::router()`
   (`/_renox/*.js`), `/health`, `/robots.txt` (unless `public/robots.txt` exists), `/favicon.ico`
   (204 unless `public/favicon.ico` exists), `/_renox/live`
   (debug + local only) and the local disk's public files (`/storage/...`, sandboxed headers).
6. `inspector` (only with debug + local: records the request's status, time, view and SQL via
   `capture_queries`, skipping `/_renox/*`) → `context` (`renox::context`: a fresh task-local context per request holding the `AppState`
   for `context::app()`, and the `RequestInfo` error reports read; jobs, scheduled tasks and app commands get one too via `scope_app`) → `session` (cookie or database driver) → `i18n` (`RequestLocale`: session `_locale`, else
   `Accept-Language` with `App::detect_locale` (adds `Vary`), else `APP_LOCALE`) → `auth` (loads the user once from session or `Authorization: Bearer`, with the
   token's abilities, and the user's roles/permissions when the `Permissions` module is on;
   inserts `CurrentUser` and `AppState` into extensions) → `csrf` → `view` (renders `View`s and error
   pages; `ValidationError` → redirect back for plain forms) → `maintenance` (503 while
   `storage/framework/down` exists; inside `view` so the 503 uses the error template) → `guard`
   (a handler panic or a run past `REQUEST_TIMEOUT` becomes a 500 with the error page).
7. `App::layer` layers (first added = outermost), around the modules' routes only.
8. Routes. Also inside the layers: `/_renox/files` (storage), `/_renox/grid/{grid}/prefs` (data
   grid preferences), `/_renox/mail` and
   `/_renox/debug` (only with `APP_DEBUG`; the inspector answers 404 unless also local), and the fallback: `public/` (`ServeDir`, or the embedded files) with a 404.

With `Routes::domain`, `build_router` runs once per domain pattern (each with the whole stack
above and its own fallback), and `domain::dispatch` picks one router per request by `Host`.

Because `AppState` and `CurrentUser` are in request extensions, guards (`require_auth`, …) are
plain `from_fn` middlewares with no state parameter and can be added from `Module::routes()`.

### 3.2 Key decisions (also in ROADMAP "Decisions")
- **One runtime crate (`renox-core`)**, not renox-http/-db/-view: those would all need `AppState`
  and `App` would need all of them (circular). `renox` is a thin facade.
- **Cargo features:** `renox` defaults to `fake`, `http` (`renox::http` sends real requests
  through reqwest; the test fake works without it) and `server-events` (needs `http`); optional
  `postgres`, `s3`, `uuid`, `xlsx` (Excel exports of data grids). renox-core is `default-features = false` in the workspace deps; the `renox` crate owns
  the defaults.
- **Sessions are an encrypted, signed cookie by default** (`cookie` PrivateJar, key derived from
  `APP_KEY`), so no DB is needed; keep that small (a warning is logged over 4 KB).
  `SESSION_DRIVER=database` (M21g) puts only an id in the cookie and the session in `sessions`
  (keyed by sha256(id), a new id at each login and logout). Flash data lives
  one request. A per-session lifetime override powers "remember me". Logout ends this device
  only (its session id goes to the `revoked_sessions` denylist, so a copied cookie dies too);
  `logout_other_devices` / `User::revoke_sessions` bump `users.sessions_revoked_at`, checked on
  every request, to end the user's other sessions.
- **Auth sessions store the user id + a fingerprint of the password hash** (no remember-token
  column): changing the password logs out other sessions.
- **Views: MiniJinja** (runtime, overridable, autoreload in debug via `minijinja-autoreload`).
  App templates override built-ins by file name (loader checks `VIEWS_PATH` first, then `BUILTIN`).
- **HTML escaping uses a custom formatter** (`view.rs::format_value`): escapes `& < > " '` but not
  `/` (MiniJinja's default escapes `/` as `&#x2f;`, which uglified URLs in pages and mail).
- **The app binary is its own CLI** (like artisan): `my-app migrate|migrate:rollback|migrate:fresh|
  migrate:status|db:seed|queue:work|queue:failed|queue:retry|queue:flush|queue:forget|
  queue:prune-failed|queue:prune-batches|webhook:failed|webhook:retry|cache:prune|session:prune|
  ui:publish|schedule:list|schedule:run|schedule:work|route:list|db:shell|down|
  up|help`, default `serve` (modules add more: `tokens:prune` and `notifications:prune` from Auth, `audit:prune` from Audit),
  plus the app's own commands (`App::command`, or `App::typed_command` for a clap `AppCommand`;
  names can't clash with built-ins). Migrations and
  jobs are compiled into the app, so only the app can run them. `rnx <anything unknown>` forwards
  to `cargo run --quiet -- <args>`.
- **Own migrator** (table `renox_migrations`, Laravel-style batches) instead of sqlx's, to support
  batches and module-owned migrations. `migrations!()` embeds `*.up.sql`/`*.down.sql` (or plain
  `*.sql`, not reversible). Apps need `build.rs` with `cargo:rerun-if-changed=migrations` so new
  files are picked up (`rnx new` writes it).
- **Models:** `#[derive(Model)]` generates `impl ::renox::db::Model` (and `FromRow`) using
  `::renox::…` paths; values go through `DbValue`/`ToDbValue`, rows decode via `Row::try_get`, so
  apps don't need sqlx directly. The primary key column is always `id`; its type is the key
  (`Model::Key`, sealed `ModelKey`: `i64`, `Ulid`, `Uuid`, `String`, M22); an empty key (`0`,
  nil, `""`) = unsaved, ULIDs/UUID v7s are made on insert. Table name =
  snake_case struct name (no pluralisation in the derive: not every language plurals with
  "s"); the generators write the plural explicitly (`make:model`, `--resource`: the model's
  name through `scaffold::plural`, #127), as the docs and examples have it. The query
  builder validates column names against `COLUMNS` and operators against a whitelist, so SQL
  injection via names is an error.
- **Relations are explicit loaders, no lazy relations** (`db::relations`: `belongs_to`,
  `has_many`, `Pivot`, `Morph`; each loads a page's related rows in one query). See
  docs/relations.md.
- **Model hooks are opt-in:** `#[model(hooks)]` makes the derive forward `Model::saving/saved/
  deleting/deleted` to `impl ModelHooks`. Only `save`, `insert` (and `create`), `save_only`,
  `save_changes`, `delete` and `force_delete` call them; bulk query methods never do. Keep it that way (documented).
- **Validation:** fluent rules in `impl Validate`, or `#[derive(Validate)]` (M25:
  each `#[validate(item)]` becomes a call on the field's rules, so every `Field` rule works;
  `label` goes first; `each`/`distinct`/`rename` are special; `#[validate(hooks)]` forwards
  to `ValidateHooks`). `Valid<T>` handles form, JSON
  and GET query. HTMX/JSON failures → `422 {"message","errors"}`; the bundled `renox.js` places
  errors next to inputs (`data-error-for` slots or inserted `<p class="error">`), sets
  `aria-invalid`, focuses the first invalid input in *page* order. Plain posts → 303 back with
  errors + old input flashed (never passwords).
- **Queue is Renox's own** (`jobs`, `failed_jobs`, `job_batches`, unix-second integers) on SQLite
  and PostgreSQL; chains ride in `jobs.chain`, batches count in `job_batches` inside the same
  transaction that finishes a job (`worker.rs::record`), unique claims are `renox:unique:*` cache
  rows, encrypted payloads start with `enc:`;
  PostgreSQL workers reserve with `FOR UPDATE SKIP LOCKED`. apalis was the plan but its stable SQL
  backend needs sqlx 0.8 (can't link next to our 0.9: both link `libsqlite3-sys`).
- **Workers and scheduler run inside `serve`** by default (single-process deploys). Several
  instances may share one database: every scheduled run is claimed first (`schedule::claim`:
  insert `renox:schedule:<task>:<slot>` into `cache` with `ON CONFLICT DO NOTHING`, kept for the
  interval + 1 min, or an hour for cron-style tasks such as `daily_at`; expired claims are pruned at most once a minute).
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
- **No C crypto in default builds:** reqwest uses `rustls-no-provider` and `http.rs` installs
  rustls' ring provider when it builds the client, which lettre uses too. CI checks `cargo tree -p hello -e normal -i aws-lc-rs` is
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
- `Db` (and every `Transaction` it begins) carries the `APP_KEY`-derived key (`with_key` at
  boot); `Sql::execute`/`fetch_*` seal `DbValue::Encrypted` with it and give each `Row` the key
  for `Encrypted` columns. A new place that makes rows or binds values must keep that.
- Public database APIs return `db::DbError`, never `sqlx::Error`; macros reach sqlx through the
  hidden `renox::__sqlx`.
- Emails: `auth::user::normalize_email` (trim + lowercase) on register, lookup, reset and the
  registration `unique` check; PostgreSQL has a unique index on `lower(email)`.
- `cfg(feature = "postgres")` arms: check both `cargo clippy --all-targets` and `--all-features`.

## 4. Conventions you must follow

### 4.1 Public API and docs
- Laravel naming where it maps cleanly (route names like `password.reset`, `verification.notice`,
  commands like `queue:work`), Rust idioms otherwise.
- Every public item gets a doc comment (`#![warn(missing_docs)]` in the three library crates;
  clippy `-D warnings` fails without one). Doc examples are doctests: never write ```` ```ignore ````.
  renox is a dev-dependency of renox-core and renox-macros, so examples `use renox::prelude::*`
  as apps do. Hide setup with `# ` lines and wrap statements in
  `# async fn demo(..) -> Result { … # Ok(()) }`; examples that would start a server are `no_run`.
- Handlers return `renox::Result<T>`; any `anyhow`-compatible error converts with `?`.
- New public structs/enums that may grow get `#[non_exhaustive]` and a line in docs/stability.md
  (also keep its public-dependency list in sync when adding re-exports).
- When a public API changes, fix CHEATSHEET.md, llms.txt, the guides and the examples in the same
  PR (not README.md or CHANGELOG.md: §4.11).

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
- Auth pages/mails take a `text` object from `auth/module.rs::texts(&lang)`: the built-in
  English dictionary with the app's `renox.auth.*` translations on top. Auth handlers take a
  `Lang` extractor (background code uses `Lang::of(state, &state.config.locale)`).
- Money in templates is in the currency's smallest unit everywhere: the `money` filter,
  `entry(…, format="money")`, `chart(…, format="money")` and the grid's `Column::money` divide
  by `10^currency_decimals(code)` (`view_filters::money_divisor`; `divide_by=` overrides it,
  `divide_by=1` for whole units). `renox::format_money` (Rust) takes whole units. renox-admin's
  money fields show and take whole units and convert in `read_form` (#335).
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
A setting that takes one of a few words (`SESSION_DRIVER=database`) is an enum made with
`setting_enum!` (lib.rs: `#[non_exhaustive]`, `as_str`, `Display`, `Serialize`, and
`parse`/`parse_as` that name the variable in errors), never a `String` compared in code.
Durations are `Duration` fields even when `.env` has minutes or seconds.

### 4.5 Migrations owned by the framework
Names start with `0001…` so they sort before app migrations (`2026…`). There are eighteen:
- Auth module (`auth/module.rs` `MIGRATIONS`): `00010101000000_create_users_table`,
  `…000001_create_password_reset_tokens_table`, `…000002_create_personal_access_tokens_table`,
  `…000003_create_notifications_table`, `…000004_add_sessions_revoked_at_to_users`,
  `…000005_add_abilities_to_personal_access_tokens`, `…000006_create_revoked_sessions_table`.
- Permissions module (`auth/permissions.rs`): `00010101000500_create_roles_and_permissions_tables`
  (roles, permissions, permission_role, role_user) and `00010101000510_add_scope_to_role_user`
  (scope_type/scope_id, '' for global, and starts_at/ends_at; SQLite rebuilds the table to
  replace its UNIQUE, so it is a `Migration` literal with its own PostgreSQL down, #244).
- Audit module (`audit.rs`): `00010101000600_create_audit_logs_table`.
- Every app (registered in `App::boot`; eight, which tests/it/database.rs lists):
  `00010101000100_create_jobs_table`, `00010101000110_add_chains_and_batches_to_jobs` and
  `00010101000120_add_callback_of_to_jobs` (queue), `00010101000200_create_cache_table` (cache),
  `00010101000210_create_sessions_table` (session.rs, installed whatever `SESSION_DRIVER` is),
  `00010101000220_create_grid_preferences_table` (grid/mod.rs),
  `00010101000300_create_webhook_calls_table` and
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
  docker run -d --rm --name renox-pg --shm-size=512m -e POSTGRES_PASSWORD=postgres \
      -e POSTGRES_DB=renox_test -p 55432:5432 postgres:17-alpine
  TEST_DATABASE_URL=postgres://postgres:postgres@localhost:55432/renox_test \
      cargo test -p renox -p renox-core -p renox-cli -p bikeshop --features renox/postgres
  ```
  Not examples/hello (SQLite migrations); examples/bikeshop has both.
- **S3:** `cargo test -p renox --features s3 --test it s3` with `TEST_S3_ENDPOINT`,
  `TEST_S3_BUCKET`, `TEST_S3_ACCESS_KEY_ID`, `TEST_S3_SECRET_ACCESS_KEY` (without them the tests do
  nothing). The SeaweedFS commands are at the top of `it/s3.rs`. With the same variables,
  `cargo test -p bikeshop --features s3 --test fields` runs the bike shop's /about/fields files
  on that bucket (CI's `s3` job runs both).
- Don't run tests with `--release` (slow compile, no debug assertions).
- Run tests scoped while working (`cargo test -p renox --test it -- module::name`, `-p
  renox-core --lib path`, a plugin's `--test file name`), not the whole workspace: a full run
  rebuilds and fills the disk, and CI runs everything. `cargo test --doc -p renox
  MacroCompileErrors` runs the macros' `compile_fail` blocks; a `compile_fail` passes for any
  error, so check a new one fails with its own message.
- JavaScript (renox.js, renox-ui.js, renox-grid.js, editors.js) is tested in a browser:
  `tests/browser/run.sh [file…]` (§2). Processes (`serve` and signals, workers, the scheduler,
  `rnx serve`, every example served): `tests/process/run.sh [fixture|examples|postgres]`.
- A warning that matters is asserted: `crate::logs::capture()` in `crates/renox/tests/it/`
  (`crate::test_logs::capture()` in renox-core's unit tests) collects what the test's thread
  logs; check it with `logs.has(&[…])`.
- Coverage: the CI `coverage` job (every crate, SQLite + PostgreSQL merged, HTML artifact);
  locally see CONTRIBUTING.md "Coverage". Stable `llvm-cov` doesn't count doctests.

### 4.8 Forms and validation internals
- Forms are deserialized with `serde_html_form` (repeated names → `Vec`), or, when a name has
  a `[` (`lines[0][name]`), with `validation/nested.rs` (a tree of text, numbers parsed when
  asked for, "" → `None`, empty values kept so rows keep their numbers). The retry loop in
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
- `rnx make:job` / `make:command` insert their `app.job::<…>()` / `app.typed_command::<…>()` into the
  module's `fn register` (creating it before `fn routes`); `make:migration` bumps the timestamp
  past the newest migration.
- `tests/cli/run.sh [postgres]` makes an app with every generator and builds/tests it
  (`FROM_GIT=1 DOCKER=1` for the Docker job). **Add every new `make:*` there.**
  It also makes apps with the option combinations people use, with names before and after
  "renox" (#143): `atlas` (plain), `pulse` (`--notifications`, #151), `studio` (`--starter`, with the database) and, with
  sqlite, `site` (`--tailwind`: downloads the pinned Tailwind CLI with `curl`, so it needs
  the network) and `desk` (`--starter --tailwind`); each passes `cargo fmt --check`, clippy
  and its tests (the every-generator app skips clippy: its output is dead code until used).
  With SQLite, `tests/cli/smoke.py` serves the shop and starter apps and drives them over
  HTTP (#142; it reads their SQLite file, so not on PostgreSQL). On postgres,
  `E2E_POSTGRES=postgres://…:5432` (CI sets it) runs the apps' tests and commands against
  that server (each app in a fresh `renox_e2e_<app>` database, tests in `renox_test`);
  without it, build and lint only.
- Every `.rs` file a command writes or edits goes through `format::touched` (in
  `write_new` and the in-place edits of generate.rs, and `rnx new`'s stubs); `main` runs
  rustfmt on them at the end, one file at a time through stdin (with a path, rustfmt
  follows `mod` lines into the rest of the crate). New apps and generator output must pass
  `cargo fmt --check`: tests/cli/run.sh checks it, and new.rs has a test with names on both
  sides of `renox` (#124). A new place that writes Rust files must call `touched`.
- `rnx new` pins `rev` from `renox-cli/build.rs` (`git rev-parse HEAD`, else the cargo checkout
  directory's short rev). Testing it through `cargo install --git` needs the change committed,
  since that builds the committed tree.

### 4.10 Docs and examples for app authors and agents
- `README.md`: agents don't edit it (§4.11); the owner keeps it. What follows is for the owner.
  It is compiled (`ReadMe`): keep its Rust blocks complete. It's the front page, so it
  sells: tagline, why, GIF, 3-line quick start, a short taste, fold-out feature tour, comparison
  with Loco/Axum, then status. Keep claims true (checked against the code). Since
  1.0.0-rc.1 it has the crates.io/docs.rs badges, installs from crates.io (`--version` while
  1.0 is a release candidate) and has a "Use Renox with Claude Code" section. Its docs links
  stay repository files (they work on GitHub and crates.io, and the site rewrites them to its
  own pages); the top and "Documentation" point readers at https://docs.renox.rs (#141),
  which is also the crates' `homepage`.
- `CHEATSHEET.md` is compiled: every ```rust block must build on its own (visible `use` lines, no
  `# ` hidden lines since GitHub shows them; define items only, no top-level statements, so the
  doctest's `main` does nothing). Check with `cargo test --doc -p renox`.
- There are two examples (#351, the owner's choice): examples/bikeshop, the one complete use
  case, and examples/hello, the smallest app (the quick start and the README's GIF). Both are
  workspace members with their own tests and a README.md; module docs name the `rnx make:*`
  commands that made them. A feature worth showing goes into the bike shop, as a page with its
  "About this page" entry (`/about/fields` and `/about/htmx` are references of that kind),
  not into a new example. Keep `llms.txt` and the two READMEs in step when adding or changing
  example files; a docs link to an example must point at a file that exists.
- A route group's index is `.get("/", …)` (not `""`, which panics).
- Examples use `renox.workspace = true`, so their `make:deploy` Dockerfile builds only in a copy
  made by `rnx new` (the CI docker job covers that).
- Examples without a `.env` run with `APP_DEBUG` off, i.e. with the views embedded at build time:
  restart after editing templates.
- UI changes get a browser test in tests/browser (it replaced checking by hand; `run.sh <file>`
  runs one file). Found by hand earlier in the crud example (removed in #351): `hx-boost` on a whole section also
  boosts its edit links and delete forms (scope it to the page links), and boosted requests get
  full pages (by design, `Htmx::wants_fragment`), so pair them with `hx-select`.
- Under `CSP=strict`, Alpine's CSP build rejects statements in attributes (examples/hello's
  `@htmx:after-request="if ($event.detail.successful) $el.reset()"` threw "CSP Parser Error:
  Unexpected token: $el" when the form was sent, found by tests/browser). That's why relaxed is
  the default; strict apps move logic into `Alpine.data` in a nonce'd script, as hello now does
  (`x-data="guestbookForm" @htmx:after-request="clearOnSuccess"`). The handlers only run on
  their events, so a page that loads cleanly can still break: tests/browser/examples.test.mjs
  sends hello's and crud's forms under `CSP=strict`.
- `docs/assets/demo.gif` was made by driving examples/hello (`APP_LOCALE=en`, a fresh database)
  in headless Chrome over CDP (a 760 × 752 viewport with `Emulation.setScrollbarsHidden`,
  `Page.captureScreenshot` per typed character, `Input.insertText`), then composing the frames
  with Pillow (a browser bar on top, a caption bar under the page, one 128-colour palette for
  every frame, ~73 KB). Last recorded in M31 on the warm theme. Re-record it when the guestbook's
  look changes.

### 4.11 Git, PRs, CI (how the owner works)
- **Work starts from a GitHub issue** (since 1.0.0-rc.3; CONTRIBUTING.md "How work is
  tracked"): a bug, a user story or a task, with labels for type, area and priority, in a
  milestone, on the project board. Found something while working? Open an issue for it
  rather than a note here or a ROADMAP checkbox. Branches are `fix/issue-N-…` (bugs) or
  `feat/issue-N-…`; the PR says `Closes #N` and follows .github/pull_request_template.md.
  ROADMAP.md keeps principles, milestone records and decisions, not open work.
- `gh issue edit`/`view` and `gh pr edit` fail on the deprecated Projects (classic) GraphQL
  field: use REST (`gh api repos/arif-rachim/renox/issues/N/labels`, `…/comments`, `-X PATCH
  …/pulls/N`). Projects v2 needs the `project` scope (`gh auth refresh -s project`).
- One branch and one PR per milestone or fix (branch names like `m17b-examples`, `fix-…`). The
  owner reviews and **merges PRs themselves**, then says so ("Done"). Don't merge unless asked.
- Before starting, check open PRs (`gh pr list -R arif-rachim/renox`) and base new branches on an
  up-to-date `main`. **Don't stack PRs** on unmerged branches (§6.3): push the next branch, but open
  its PR only after the previous one is merged.
- **Never edit `CHANGELOG.md` or the root `README.md`** (the owner's rule, 2026-10-05): every
  PR touched the same "Unreleased" lines and the same guide list, so every merge left the other
  open PRs in conflict. Leave both files exactly as they are on `main`, even when a feature or a
  breaking change would belong there; say it in the PR description instead (the owner keeps
  those two files). This overrides every other line in this file that says to update them.
  Example READMEs (`examples/*/README.md`, a crate's own README) are not covered and are still
  kept in step.
- Each milestone PR also updates: ROADMAP checkboxes and notes (tick only what is in the
  code, with the API names the code uses), §7 of this file, `llms.txt`, and the example READMEs
  when examples change. Also check, and update when the milestone touches them:
  `docs/operations.md` (commands, tables to prune, what
  `APP_KEY` protects, failure table), the other guides in `docs/`, the counts in §4.5/§7,
  `crates/renox-cli/stubs/AGENTS.md.stub` (traps for app agents) and `env.stub`, and the
  examples: an example that works around something the milestone adds should use the new API,
  and a major new feature should be shown by at least one example. M18–M20 skipped these and
  needed a catch-up PR (#53).
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
  `tests/cli/run.sh`), **tutorial** (`tests/tutorial/run.sh`), **docker** (`make:deploy` image answers `/health`), **s3** (SeaweedFS; renox's `it/s3.rs` and bikeshop's tests/fields.rs),
  **cargo-deny**, **coverage** (informational); a separate workflow, **Release build (site)**
  (release-site.yml: on pushes to main that touch the docs, builds site/ for the owner's server,
  which pulls it),
  **semver checks** (pull requests:
  `cargo semver-checks -p renox-core -p renox --baseline-rev origin/<base> --release-type
  minor`, informational until the first release; install it with `cargo install --locked
  cargo-semver-checks` to run it locally).
- MSRV is `rust-version` in the workspace `Cargo.toml` (1.94, set by sqlx 0.9); the `msrv` job
  uses the same number, so raise both together and note it in the PR description.
- After pushing, watch CI (`gh run watch <id> -R arif-rachim/renox --exit-status`) and tick the
  "CI green" box in the PR body.

## 5. Configuration (env vars)
Parsed in `crates/renox-core/src/config.rs`; defaults in parentheses.
- **App:** `APP_NAME` (Renox), `APP_ENV` (local; accepts local|dev|development,
  testing|test, production|prod; anything else fails at boot), `APP_DEBUG` (on in local),
  `APP_URL`, `APP_KEY` (required in production; `base64:…`, `rnx key:generate`), `APP_HOST`
  (127.0.0.1, an IP), `APP_PORT` (3000), `APP_LOCALE` (en; English is built in, other locales come from the app's `lang/*.json`),
  `APP_FALLBACK_LOCALE` (en), `APP_CURRENCY` (USD; the `money` filter, the kit's money entries/charts and the grid's money columns, all in the smallest unit), `APP_TIMEZONE` (`UTC`, an offset like `+07:00`, or an IANA name like
  `Asia/Jakarta`, with DST).
- **Paths:** `VIEWS_PATH` (resources/views), `LANG_PATH` (resources/lang), `PUBLIC_PATH`
  (public), `STORAGE_PATH` (storage; holds `framework/down` for maintenance mode and `app/` for the
  local disk). Paths are relative to the working directory: run apps from their own directory.
- **Sessions:** `SESSION_DRIVER` (cookie|database), `SESSION_LIFETIME` (minutes, 120),
  `SESSION_COOKIE` (renox_session),
  `REMEMBER_LIFETIME` (minutes, 43200).
- **Database:** `DATABASE_URL` (sqlite://storage/app.db, or `postgres://…` with the `postgres`
  feature; other schemes fail), `DATABASE_POOL_SIZE` (8, at least 1), `DATABASE_ACQUIRE_TIMEOUT`
  (seconds, 5), `DATABASE_STATEMENT_TIMEOUT` (seconds, 30, PostgreSQL; 0 = none).
- **Requests:** `REQUEST_TIMEOUT` (seconds, 60; 0 = none), `UPLOAD_MAX_SIZE` (MB, 10),
  `TRUSTED_PROXIES` (addresses, CIDR ranges or `*`), `TRUSTED_HOSTS` (host names, `*.example.com`;
  other hosts get a 400; empty: any), `CSP` (relaxed|strict|off, also `false`/`none`
  for off; relaxed is the owner's chosen default).
- **Logs:** `RUST_LOG` (default by command: `info,renox=debug` for `serve`/`queue:work`/
  `schedule:work` with debug on, `info` for them without, `warn` for every other command), `LOG_FORMAT` (text|json; anything else fails at boot),
  `LOG_FILE` (append there instead of stdout, no ANSI colors; falls back to stdout if it can't
  be opened).
- **Mail:** `MAIL_MAILER` (log; smtp|log|memory), `MAIL_HOST`, `MAIL_PORT`, `MAIL_ENCRYPTION`
  (starttls; tls|starttls|none), `MAIL_USERNAME`, `MAIL_PASSWORD`, `MAIL_FROM_ADDRESS`,
  `MAIL_FROM_NAME`, `MAIL_TIMEOUT` (seconds, 10, the whole send), `MAIL_FAILOVER` (names of
  `App::mailer` mailers tried in order when the default fails; an unknown name fails at boot).
- **Background:** `QUEUE_WORKERS` (2; 0 = none in serve), `SCHEDULER` (true), `CACHE_STORE`
  (memory|database; database also shares throttles and the login lock between servers).
- **Storage:** `STORAGE_DISK` (local|s3), `S3_BUCKET`, `S3_REGION`, `S3_ENDPOINT`,
  `S3_ACCESS_KEY_ID`, `S3_SECRET_ACCESS_KEY`, `STORAGE_URL`.
- **SEO/analytics** (used in production only): `GOOGLE_SITE_VERIFICATION`, `GA4_MEASUREMENT_ID`,
  `GA4_API_SECRET`, `GTM_CONTAINER_ID`.
- **App-specific:** anything else through `config.var(name)` (`config.vars` first, then the
  environment; empty counts as missing).
- **Set by others:** `LISTEN_FDS`/`LISTEN_PID` (systemd socket activation; `serve` uses that
  socket). For `rnx` itself: `TAILWIND_BIN` (a Tailwind binary to use instead of the pinned
  download) and `RNX_CACHE_DIR` (where downloads are kept).
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
- **Browser testing** is tests/browser (headless Chrome over the DevTools protocol, Node 24's
  `node:test`, no npm packages; lib/cdp.mjs launches Chrome, drives pages and collects console
  errors, exceptions and CSP violations). Write a test there rather than a one-off script; it
  found bugs unit tests missed (§6.4). A sandboxed `<iframe>` (mail preview) can't be read from
  the parent: check it via screenshot.

- **Other agent sessions use this checkout too** (2026-10-04: a session working on another
  project ran `git checkout origin/main` here, and a commit landed on a detached HEAD; it
  also builds into the same `target/`, so a test binary looked fresh but lacked the new
  module). Work on an issue in its own worktree with its own target directory:
  `git worktree add -b feat/issue-N-… <scratchpad>/wt-N origin/main`, and
  `CARGO_TARGET_DIR=/home/developer/workspace/renox-target-wt` for its builds. Check
  `git branch --show-current` before committing.

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
- A translated validation message named the field differently from the translated page label
  → localised labels in auth forms.
- The reset-password mail button reused the page's save-button label → uses the subject.
- A scripted edit meant to add `.image()` to the guestbook's photo rule silently didn't apply, so a
  text file named `.png` was accepted. All framework tests passed; only the headless-Chrome upload
  check showed it. Keep browser checks for UI features.
- renox-grid.js declared two functions named `save` (column preferences, inline-edit cells); the
  second replaced the first, so every column-menu action threw and saved nothing. Found by
  tests/browser/grid.test.mjs (#267); tests/browser/assets.test.mjs now refuses a top-level
  function declared twice in any shipped JS.
- Live validation of a list field (`tags`) answered with no errors while its items failed
  (`each(…)` reports on `tags.0`): the answer looked up the exact key only. Found by
  tests/browser/renoxjs.test.mjs; it includes the items' errors now.
- Writing browser tests (tests/browser): headless Chrome has no hovering mouse (`(hover:
  hover)` is false, so tooltips never show) unless started with `--blink-settings=…HoverType…`;
  each open tab keeps its live-reload stream and Chrome allows six connections per host (close
  every page, `browser.with` does); the browser's own checks (`required`, `type=email`) stop a
  form before the server sees it (give test forms `novalidate`); `hx-confirm`'s native dialog
  blocks the page until `Page.handleJavaScriptDialog` answers it.

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
The workspace's dev profile has `debug = "line-tables-only"` (as `rnx new` apps do): full
debug info made the `it` binary 415 MB and its link peak at 1.7 GB, and a workspace build
with 8 link jobs ran the 15 GB machine out of memory (systemd-oomd killed the terminal and
the agent in it, twice). Now 150 MB and 0.76 GB. Run heavy builds with `-j 4`, one at a
time, ideally in their own scope (`systemd-run --user --scope -p MemoryHigh=9G …`) so oomd
picks the build, not the terminal.

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
- Every test run leaves its `renox_test_…` schemas behind. After four days of runs (8,355
  schemas) the container's default 64 MB `/dev/shm` filled up ("could not resize shared memory
  segment … No space left on device"), a backend segfaulted and the server went into recovery,
  so 273 tests failed at once with "not yet accepting connections". Recreate the container
  (`docker stop renox-pg`, then the `docker run` above, which sets `--shm-size=512m`) when a
  PostgreSQL run fails everywhere at once.
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

- **All milestones M0–M34 are merged to `main`** (M34: #114); the owner's B/C/D before
  1.0 were M23–M25. History:
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
  keys and `Encrypted<T>` were deferred there and done in M22/M23.
- **M20a** (scheduler: cron/weekly/monthly, filters, IANA zones with DST, on_failure/on_success,
  `schedule:run`; cache add/pull/increment, locks, prune): merged (#51). A cron
  time skipped by DST runs right after the jump; intervals follow the current offset. Schedule
  methods return `ScheduledTask` (DerefMut to `Schedule`) so add-chains still compile.
- **M20b** (queue: priority, unique, encrypted, middleware, failed hook, chains, batches,
  dispatch_sync, forget/prune): merged (#52). Adds framework migration
  `00010101000110`; tests that count framework migrations must follow it.
- **M20c** (`renox::http` + fake + schedule pings, queue dashboard module, localized mail and
  notifications, mail components, storage list/copy/rename): merged (#54). The
  dashboard page was browser-checked (desktop, 390 px, dark). M20 is done.
- **M21a** (rough edges from #53/#55: `User::has_role` via context grants, `users_with_role`,
  confirm-password return for forms, `Db::retrying`, batch callbacks' `batch_id`
  (`callback_of` migration), `run_all_jobs`, `capture_queries`, `Morph::count_many`,
  `Current<T>`, seeders in context, old input on hook errors, `renox::Path` 404s): merged (#56).
- **M21b** (components that see the request via `RequestGlobal` + a thread-local of the page's
  globals, the HIG-style kit `views/ui.html` + `assets/renox-ui.{css,js}`, `toast.rs`,
  `View::also`, Hx headers, live validation via `X-Renox-Validate`, `ui:publish`,
  `make:component`): merged (#57), browser-checked on the crud example. Keep new kit
  components `rx-`-prefixed, keyboard-usable, and at WCAG AA contrast (docs/ui.md rules).
- **M21c** (`Routes::resource`, `make:module --resource` in renox-cli/src/scaffold.rs, new
  generators, `rnx new` layout on the kit, `clock.rs` + `TestApp::travel`, event and
  notification fakes in `AppState::fakes`, test assertions, `TestApp::serve`): merged (#58). Time must be read through `clock` (`db::now`, `queue::unix_now`), not
  `SystemTime::now`, so travel reaches it.
- **M21d** (request id outside `TraceLayer`, `LOG_FORMAT`/`LOG_FILE`, `App::report`, error pages
  in the app layout, `route()` query strings, named limiters, `/_renox/debug`): merged (#59).
  A context key that is `None` hides a global of the same name (`merge_maps` takes the first
  map's value, even `None`): the error page's debug key became `request_line` for that.
- **M21e** (Tailwind's standalone CLI pinned in renox-cli/src/tailwind.rs with SHA-256 sums,
  `push`/`stack` markers filled after the render, `AppCommand` + `typed_command`,
  `renox::prompt`): merged (#60). Stacks are dropped in fragments and mails (no scope there).
- **M21f** (`Validate` hooks `prepare`/`authorize`/`after` + `FormContext`, new rules, Renox's
  auth pages on the kit): merged (#61).
- **M21g** (`SESSION_DRIVER=database`: `Stored::Handle` cookies, rows keyed by sha256(id), id
  rotation at login/logout, `AppState::session_mirror` under `APP_ENV=testing`; systemd socket
  activation via `listenfd` + `deploy/<app>.socket`): merged (#62). A session given a new id must
  always be INSERTed, whatever its content.
- **Docs and examples audit** (#63): three read-only audits compared every doc, example and stub
  with M21; fixed the socket recipe's order, the shop admin delete that asked nothing, crud's
  missing flash, `make:mail`'s hint, and many pre-M21 statements.
- **M21h** (shop, teams and htmx-recipes on the kit; htmx-recipes adds `.also()`,
  `HxRetarget`/`HxReswap` and toasts): merged (#64). Three framework fixes: `add_trigger`
  `\u`-escapes non-ASCII (a raw UTF-8 `HX-Trigger` failed and the toast was silently dropped),
  toasts with `HxRefresh` wait in the session like `HxRedirect`, and error pages get `App::share`
  values (a layout using one failed under strict undefined). Toasts must survive any response
  htmx turns into a new page: they go to the session there.
- **M21i** (the examples' tests on `travel`, fakes, `assert_view`/`assert_json_path`;
  `App::report` in the jobs example; hello's typed `entries:prune`; fields with `each` +
  `one_of` + `distinct`; the stubs): merged (#66). Two framework fixes found by
  travelling: in-memory rate limits and the login lock used `Instant` (now `clock::Stamp`), and
  `TestApp`'s session helpers read the cookie on the real clock (now the travelled one, via
  `clock::with_offset_sync`). Anything timed in memory must use `clock`, not `Instant`.
  The offset is a task-local, which `tokio::spawn` doesn't inherit: wrap spawned app code in
  `clock::carry(fut)` (the worker's jobs and hooks, webhook handlers, scheduled runs), or
  travel doesn't reach it (found by the tutorial e2e, 2026-10-04).
- **After M21, small fixes:** `/favicon.ico` answers 204 unless the app ships one (#67; every
  example logged a 404 console error when run in a browser); the workspace dev profile uses
  `debug = "line-tables-only"` (#68, after two OOM kills during workspace builds).
- **M22** (model keys: `Model::Key` from the `id` field's type, `Ulid`, `Model::insert`,
  relations generic over keys, `Pivot<L, R>`, `make:model --key`, the fields example on `Uuid`):
  merged (#69). Generic code over models that needs an integer id says
  `M: Model<Key = i64>`.
- **M23** (B of the owner's B/C/D before 1.0: `Transaction::savepoint`, `db::Encrypted<T>`
  with the key carried by `Db`/`Transaction`/`Row`, the teams example on it): merged (#70).
- **M24** (C: `Routes::domain`/`fallback`, `route_is`/`CurrentRoute`, `Redirect::route`/
  `intended`, session `push`/`increment`, `Factory::factory()` states and sequences, plural
  ranges, `loop_controls`, `class_names`): merged (#71). A domain's host
  gets only that domain's routes (no fall-through, unlike Laravel).
- **M25** (D: `#[derive(Validate)]` + `ValidateHooks`, `App::detect_locale` for
  `Accept-Language` with `Vary`; hello and `make:module --resource` use the derive): merged
  (#72).
- **M26** (completeness before 1.0, three PRs): M26a (key bugs in `insert_many`/`upsert` and
  `unique().ignore()`, the fallback status bug, examples for M22–M25): merged (#73). M26b (docs
  brought up to date, the 380 undocumented public items documented, `missing_docs` enforced,
  `RedirectExt` sealed, `InvalidUlid` non-exhaustive): merged (#74). M26c (tests for `renox-cli` and weak core files, `App::run_args`,
  `renox::Path` answers 500 for a parameter the route lacks): merged (#75). M26 is done.
- **M27** (a data grid, asked by the owner before v1.0; three PRs): M27a (`renox::grid` +
  `renox/grid.html`, Cally, `grid_preferences`, `sparkline`, the grid example): merged (#76). M27b
  (`audit`/`details`, `editable` + `edit_url`, `reorder` + `RowOrder`, `merge`, several
  default sort keys): merged (#77). M27c (`exports`/`export`: CSV, Excel behind
  the `xlsx` feature, a print page): merged (#78). M27d (move columns by their
  heading, resize by its edge, `GridPrefs::widths`): merged (#79).
- **M28** (the grid next to Filament's tables, five PRs, the owner asked for all): M28a
  (`prefix`, `searchable` + search box, filter chips, `row_url`, `empty_state`): merged (#80). M28b (`bulk_action`/`row_action`, `Action`, `Selection`, `selected`):
  merged (#81). M28c (`Column::summary`/`Summary`, `groups`/`group_by`): merged (#82). M28d (`cards_on_mobile`, image/color columns, badges, icons,
  description, tooltip, wrap, limit, link, copyable): merged (#83). M28e
  (`related`/`count_of`/`sum_of`, `advanced_filter`, `remember`, `poll`, NULLs last): merged
  (#84). M28 is done (#80–#84).
- **Docs audit after M28** (merged, #85): every doc checked against the code,
  `docs/grid.md` added (doctest `GridGuide`). Three fixes it found: grid date-time filters take
  `APP_TIMEZONE` days (`day_start` in grid/mod.rs), `delete_account` also deletes
  `grid_preferences` rows, and `notifications:prune` / `auth::prune_read_notifications`.
- **Guides for the remaining areas**: docs/routing.md, docs/validation.md, docs/mail.md and
  docs/scheduling.md, which before lived only in CHEATSHEET.md: merged (#86). The grid example's
  `/follow-up` page (two prefixed grids, the rest of docs/grid.md's column options): merged
  (#87).
- **UI kit form fields** (Filament's forms as the yardstick; stage 1 of 3): `radio`,
  `checkbox_list`, `form_grid`/`fieldset`, `span`, `prefix`/`suffix`, `datalist`,
  `disabled`/`readonly`, `has_old()` (a checkbox or radio missing from `old()` after a
  failed submit was sent empty); the fields example on the kit. Stage 2: `revealable`/`copyable`
  inputs (auth pages reveal passwords), `toggle_buttons`, `file`, `date_picker` (Cally in a
  popover; its `change` doesn't bubble, so listen in the capture phase), `show_when`/
  `hide_when` (hidden groups are disabled fieldsets); shop checkout, uploads and fields use
  them. Stage 3: nested form names (`validation/nested.rs`: a form with a `[` in a name is
  read as a tree by its own small deserializer; errors and `old()` use dotted keys; labels
  are the last part), `KeyValues` (stored as a list of pairs: JSONB reorders object keys),
  `tags_input`, searchable/multiple `select` (a combobox over the native select, kept
  visually hidden so `required` still works), `repeater` (`{% call(row, prefix) %}`,
  renumbered by rewriting attributes), `key_value`, `wizard`; teams, fields and shop use
  them. Merged (#88, one PR for the three stages). Then the kit's `tabs` in shop's admin
  dashboard and `datalist` in teams, the two kit parts no example used yet (#90;
  `hide_when` has none, as it mirrors `show_when`). Then options from the server:
  `select(…, options_url=…, editable=true)` with `renox::select` (`SelectOption`,
  `OptionQuery`), one URL for search/lookup/add/rename; the combobox adds fetched options to
  the native select as they're chosen; shop's admin categories use it. Merged (#92).
  Then infolists (Filament's as the yardstick): `infolist`/`entry`/`repeatable` in the kit,
  the filters `money` (`APP_CURRENCY`, default `IDR` then, `USD` since #335), `since`, `words`, `markdown`
  (pulldown-cmark, raw HTML shown as text); shop's order page and fields' product page use
  them. Merged (#94). Then notifications (Filament's as the yardstick): toasts with a body,
  actions, a duration, an id and a position (`Renox.toast`), `DatabaseMessage`,
  `Auth::new().notifications()` with the kit's `notification_bell`, `renox/notifications.html`
  and `/notifications/stream` (SSE woken by `auth::notifications::Hub` in-process, polling the
  table every 15 s for other processes, five-minute streams, stopped at shutdown); shop and
  jobs use them. Merged (#96). Then dashboards (Filament's widgets as the yardstick, B before
  A): `renox::chart` (`Period`, `Trend`, `Series`), the `chart(…)` template function (SVG +
  HTML, no library), the kit's `stat`/`stats`/`dashboard`/`widget`/`period_filter`,
  `query_with`; shop's admin dashboard uses them. Merged (#97). Then actions (Filament's
  as the yardstick, the "A" after the widgets): `action_sheet` (a form in a sheet sent with
  htmx; a 422 stays in the sheet, a success closes it), `slide_over`/`width`/`icon` on
  sheets, `icon_button`, `icon`/`badge`/`key`/`disabled_reason` on buttons, keyboard
  shortcuts (`data-rx-key`), tooltips (`data-rx-tip`, one element placed by renox-ui.js:
  a CSS `::before` was dimmed by disabled buttons and widened phone tables while hidden);
  shop's admin products use them. Merged (#99).
- **M29** (examples complete; three PRs after an audit of all 13 examples): M29a (example
  bugs, tests for README claims, seeders that run twice, `.env.example` everywhere,
  fields/webhooks/uploads on the kit, current patterns): merged (#101). M29b
  (the backoffice example: invoices with a stock ledger, Midtrans/Xendit payment pages and
  webhooks, CSV import, exports from a job with the grid's filters, roles, activity log,
  settings, branded sign-in): merged (#102). M29c (relations as a public blog on Tailwind
  with Markdown/SEO/RSS/sitemap/search, an api browser client + `GET /api/me`, uploads on S3
  tested in CI's `s3` job, mail cc/bcc/reply_to/attachments in jobs): merged (#104). M29 is
  done.
- **M30** (every example on the UI kit, asked by the owner: the kit's navigation
  (`navbar`, `sidebar` + `rx-shell`), `page_header`, `toolbar`, `row_actions`, `list`,
  `card_grid`/`media_card`, `link_tabs`, `thumbnail`, `progress`, `menu_button`,
  `rx-page--fill`, `hide_label`; `rnx new`'s layout on `navbar`; all fourteen examples and
  their mails on the kit): merged (#105). New pages in examples and stubs
  use kit components only; `public/app.css` holds brand tokens and what is truly the app's.
- **M31** (the warm default theme and the `--rx-type-*` type scale, chosen by the owner after
  screenshot comparisons; Inter and Poppins bundled in assets/fonts and served from
  `/_renox/fonts`; `data-rx-theme="classic"` for the old look): merged (#107).
- **M32** (English only, asked by the owner: no Indonesian anywhere in the repo, the built-in
  `id` locale dropped, Spanish as the examples' second language; framework tests use
  `crates/renox/tests/lang/es.json`): merged (#108).
- **Docs refresh after M32** (asked by the owner: the README, the guides and the Laravel parity
  review, which was still the M17 snapshot): the review rewritten for today
  (`docs/audit/2026-10-laravel-parity.md`), the Indonesian PDF replaced by an English one,
  every guide, CHEATSHEET, llms.txt and example README checked against the code: merged
  (#110).
- **M33** (the parity review's small adds, chosen by the owner before v1.0: 28
  validation rules with `Dimensions`, `Found<M>` route model binding, `Routes::view`/
  `redirect`, named disks, `Routes::etag`, `App::xsrf_cookie`, `TRUSTED_HOSTS`): merged
  (#112). Layers like `.etag()` cover only the routes added before them.
- **M34** (the rest of the small adds: `current_password`, `Password::uncompromised` (HIBP),
  `Validator::finish_for`, session `keep`/`flash_now`, named error bags, `App::mailer` +
  `MAIL_FAILOVER`, `has_many_through`): merged (#114). Async rule checks live in
  `Validator::checks` and run in `finish_with` after the database ones.
- **v1.0 started** (the owner, 2026-10-03, after M34). V1a (release readiness: lockstep `=`
  versions, path-only dev-deps, docs.rs metadata, `rnx new` from crates.io, the semver CI
  job, RELEASING.md): merged (#115). V1b (the API audit's must-fix items: settings as enums with
  `setting_enum!` in lib.rs, `Duration` lifetimes, `Notification::channels(to)` and
  `to_database(to, state) -> Result`, `notify(impl Into<Recipient>)`, `Job::failed(ctx, …)`,
  no `Locale`, private `Upload` fields, `Migration` builders): merged (#117). V1c
  (the should-fix items; the owner chose all but `gate` → `authorize_gate`: the state last in
  every closure, seeders get `AppState`, `disk_named`, builder-only `Factory`, `DateTime`
  timestamps, opaque `Zone`, sealed `Viewer`/`Executor`/`ForeignKey`, `DownOptions`,
  `retry`/`retry_all`, secrets hidden from `Debug`): merged (#118). V1d (the docs site `site/`, docs/tutorial.md,
  docs/laravel.md; the owner hosts it on their own server): merged (#120). V1e
  (`rnx new --starter`, the 1.x promise in docs/stability.md): merged (#121). Then the release
  candidate `1.0.0-rc.1`: merged (#122), published to crates.io on 2026-10-03 with the
  owner's `cargo login` token after the owner confirmed (tag `v1.0.0-rc.1`; docs.rs built,
  `cargo install renox-cli --version 1.0.0-rc.1` + `rnx new` checked). Then the README's
  start and coding-agent sections: merged (#123). GitHub Pages was tried and dropped
  (the owner's account serves project sites on a personal domain): the docs site runs on the
  owner's own server, built with Renox: https://docs.renox.rs (renox.renoxium.com from
  2026-10-04, the project's own domain since 2026-10-07, #331). The project's domain is
  renox.rs: the landing page at https://renox.rs (www too, its own app, planned), the docs
  at docs.renox.rs, the bikeshop demo at https://bikeshop.renox.rs (#141;
  the server pulls each build of `.github/workflows/release-site.yml`).
  Then #124 (new apps failed `cargo fmt --check`): `rnx` formats what it writes, merged
  (#125); GitHub Pages switched off (2026-10-04). Release candidate `1.0.0-rc.2` with that
  fix: published 2026-10-04 (#126, tag `v1.0.0-rc.2`). Then #127 → #129 (plural tables),
  #128 → #130 (another session), #131 (the tutorial followed in CI; `clock::carry`), and
  `1.0.0-rc.3` with them: published 2026-10-04 (#132). Then the work moved to GitHub
  issues and the project board (#134); fixes #133/#144/#160/#163/#178 and the plugin extension
  points #167/#168, released as `1.0.0-rc.4` (tag `v1.0.0-rc.4`). Then #219, #226, the
  plugins renox-oauth/-admin/-billing (#147/#148/#155) and roles per branch (#244), with the
  workspace at `1.0.0-rc.5`, published (tag `v1.0.0-rc.5`). Then the coverage epic (#246:
  #277, #280, #291, #293, #295) and its fixes, with the workspace set to `1.0.0-rc.6`
  (published when the owner runs `cargo publish`). Left: 1.0.0 when the owner is happy with
  the rc (ask before every `cargo publish`).
- **Earlier plan for v1.0:** v1.0 (API audit, `cargo-semver-checks`, real
  crates.io releases (the owner runs `cargo login`), a docs site with a tutorial and a
  Laravel guide, a starter kit). 
- **#147, `renox-oauth`** (social login: Google, GitHub, the `Provider` trait, PKCE + a
  single-use `state` in the session, linking only by a verified address and never to an
  account whose own address is unverified, unlink never removes the last way in): branch
  `ccr-f926b004-17j95n`. It added to renox-core `auth::sign_in`, `register_verified`,
  `registration_open`, `confirm_identity` (auth/external.rs; the `Auth` module's settings now
  ride in `AppState::auth`), `User::has_password` (users made by a social login have an empty
  `password`: the account page lets them set one and asks them for a recent confirmation
  instead of a password), and the login/register/confirm pages'
  `{% include "renox/auth/login_options.html" ignore missing %}`.
- **#148, `renox-admin`** (the admin panel: `Admin::new().authorize(…).resource(…)`,
  `impl AdminResource` with grid columns, form fields, filters, actions and the model's
  `Policy`; lists, forms, view pages, the trash, exports; the admin example): branch
  `ccr-f926b004-17j95n`. It added to renox-core `Column::label`/`options` and
  `renox::currency_decimals`, and fixed `GridRequest`'s path inside a `Routes::group`
  (axum's nesting strips the prefix from `uri.path()`; it reads `OriginalUri` now, so a
  grid's form, links and exports keep `/admin/products`). Other code that reads
  `uri().path()` in a nested route sees the shorter path too (`FormContext::path`; signed
  URLs checked inside a group are worth a test).
  Template context keys named like a request global (`can`, `auth`, `errors`…) are hidden
  by the global (`merge_maps`, the last map wins): the panel passes `allowed`, not `can`.
- **#155, `renox-billing`** (subscriptions: `Billing::new().plan(…).stripe().xendit()`,
  `Billing::of(&state, &user)` with `subscribed`/`checkout`/`swap`/`cancel`/`resume`, the
  `Gateway` trait, Stripe and Xendit, webhooks through `renox::webhook`, guards, pages;
  the billing example): branch `ccr-f926b004-17j95n`. It added `Registry::provide` to renox-core
  (a module's settings for code without a request: `Webhook::verify`/`handle` get only the
  state). `Webhook::event_id` gets no state either: a plugin whose webhooks depend on its
  settings computes the id in a route layer and passes it in a header (billing's webhook.rs).
  `FakeHttp` answers in turn but repeats the last one: queue every answer a test needs before
  the first call, or a single answer keeps being given.
- **#244, roles per branch** (a role given in one record and for a period: `Scope`,
  `assign_role_in(..).from(..).until(..)`, `remove_role_in`, `sync_roles_in`, `assignments`,
  `users_with_role_in`, `permissions::set_scope` in `renox::context`, `has_role_in`/
  `has_permission_in`, `scopes_with` + `Scopes::apply` for default scopes,
  `permissions:prune`, `route:list` marks role/permission guards with `*`): branch
  `ccr-f926b004-17j95n`. `Grants` (permissions.rs) holds every assignment not ended at load
  time and filters by the active scope and `db::now()` at each check; `AuthUser::role_names`
  is worked out the first time it's asked. The global `assign_role`/`remove_role`/`sync_roles`
  and `users_with_role` only touch global rows.
- **#246, test coverage** (epic with sub-issues #247–#270, one PR): the coverage job measures
  every crate on SQLite and PostgreSQL; tests for every item the issues list (macros' compile
  errors, `renox::testing`'s messages, validation, views, request helpers, auth, the data
  layer, grid and charts, background work, commands, the CLI, the plugins); browser tests in
  tests/browser (CI `browser`) and process tests in tests/process. Fixes it found: date limits
  given as text, a failing `App::share` showing a bare 500, REAL/SMALLINT extra columns on
  PostgreSQL, live validation of lists, the grid's column menu (two JS `save`s), examples/hello
  under `CSP=strict`. Small refactors for testability in renox-cli (`asset_for`,
  `cache_dir_with`, `check_sum`, `app_root_in`, `key_generate_in`). Merged (#277). #280 (another
  session) tested what #277 left in #250–#253/#256 (fixes #278, #279). The rest of every
  sub-issue in a third PR: PostgreSQL races/retries/connections/migrator, grid rules and
  exports read back (zip), charts, the commands' output (tests/process), the CLI, the plugins,
  browser tests for every remaining kit/grid/editor item and each example's main flow, process
  tests on PostgreSQL; the coverage job fails under 90 % of lines. Fixes it found: #281
  (PostgreSQL TIME extra columns), #282 (charts rounded fractions), #283 (a required combobox
  focused its hidden select), #284 (`rx-shell` on phones), #285–#287 (grid date range, two
  grids on a page, default parameters in the URL), #288 (`rnx serve` left the app running on a
  signal). What stays untested is listed with its reason in that PR.
- **#231, examples/bikeshop, the flagship example** (stories #232–#243, #245; one branch
  `feat/issue-231-bikeshop`, one PR, a commit per story): a bike shop with three stores on
  Pagila's shape and volume, every page explained ("About this page", `/about/pages`), RBAC +
  ABAC on #244's roles per store, the plugins, a JSON API, dashboards and reports; the UI kit
  plus the example's own "blocks" (the owner's decision on #231), Motion vendored. #243 wired
  it into CI (`-p bikeshop` in the PostgreSQL job, tests/browser/run.sh, tests/process) and
  added `tests/queries.rs` (the main pages on the large seed: same query count as the small
  one). What Renox lacked became issues #299–#319 (listed in its README), worked around in the
  example. Open in #243: the Spanish pass, the explanations' review, the owner's sign-off.
- **#343, the bike shop's design system in Renox** (stories #344–#349 and #351, one branch
  `feat/issue-343-design-system`, one PR; the owner chose where each part goes):
  - #344: the kit's default theme is the bike shop's "editorial" look. `data-rx-theme="warm"`
    is the look before 1.1, `classic` the first one. New tokens `--rx-accent-2`, `--rx-tint`,
    `--rx-accent-soft`, `--rx-ink`, `--rx-type-hero`, `--rx-type-section`. Pill buttons,
    except in `rx-shell`.
  - #345: `date_picker(disabled_dates=…, closed_weekdays=…)`.
  - #346: `navbar(tabs=…)` (a phone tab bar), `nav_search`, and the public `icon()` macro
    (Lucide, crates/renox-core/src/icons.rs; `sidebar_link`, `stat` and `empty` take `icon=`).
  - #347: crates/renox-blocks.
  - #348: the starter's page patterns (stubs/starter: patterns.html + patterns.css, owned by
    the app).
  - #349: the kit's rarely used parts load on demand.
  - The bike shop: "About this page" is docked beside every page from 1200 px, with code
    samples cut from the source at build time (`[explain:name]` markers, build.rs →
    `code::REGIONS`, checked by tests/about.rs). A visual pass added icons, Unsplash photos
    (CREDITS.md) and initials avatars. The grid, charts and the kit's basics stay in core
    (the owner's decision).
- **#351, two examples** (the owner's choice, after the design-system work #343): only
  examples/bikeshop and examples/hello remain. Before the other fifteen went, what they showed
  and the bike shop lacked moved in: each store's page on its own host (home/stores.rs,
  `Routes::domain`), `/about/fields` (the types reference docs/types.md points at; files public
  and private; tests/fields.rs on SQLite, PostgreSQL and S3) and `/about/htmx` (the htmx
  recipes). Their browser tests moved to bikeshop-*.test.mjs, and the grid example's pages to the
  fixture's `/grid`. CI's PostgreSQL job tests bikeshop, its s3 job bikeshop's
  tests/fields.rs. The entries above name removed examples: they are history. In headless
  Chrome the tab that sent the bike shop's login form gets no key presses afterwards; tests
  that type after logging in log in from a tab of their own.
- **Still open** (ROADMAP `- [ ]`): none of the plugins; `renox-2fa` (#146),
  `renox-oauth` (#147), `renox-admin` (#148) and `renox-billing` (#155) are done. A Laravel gap review after M25 (in the
  conversation that planned M26) ranked them: release and docs first, then 2FA and social
  login, then small adds (validation rules like `json`/`gt`/`decimal`/`dimensions`, several
  storage disks, route model binding), then admin, search, realtime (SSE) and billing.
- As of M32: ~65k lines of Rust in `crates/` (stubs excluded) and ~16k in the fourteen
  examples, ~770 `#[test]`/`#[renox::test]`/`#[tokio::test]` functions in `crates/` and
  `examples/` (plus doctests), and 44 direct dependencies in renox-core (6 optional;
  pulldown-cmark came with the `markdown` filter, #94).
  Keep dependencies lean and remove unused ones.
