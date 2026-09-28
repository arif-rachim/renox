# Laravel parity review (September 2026, after M17)

**Question:** is Renox mature enough to compete with Laravel, and if not, what must be added?
**Method:** six read-only reviews, one per area (HTTP, data, auth and security, background work
and services, views, tooling and ecosystem). Each listed Laravel 11/12's features one by one,
found the Renox equivalent in the code (file:line), and rated it. The Renox side was checked
against the code at `c0df3d0` (M17b). The Laravel side comes mostly from the reviewers'
knowledge of Laravel 11/12, spot-checked against the docs where it mattered.

> **Status (after M20b):** this review is a snapshot at M17 and its tables are kept as they
> were. Most gaps marked M18, M19 and M20 are closed since (tenancy, roles and permissions,
> account pages, the query builder and model features, cron and time zones, locks, the queue);
> ROADMAP.md ticks what was built and lists what was deferred. A later gap report, in
> Indonesian, is [2026-09-laravel-gap-report.pdf](2026-09-laravel-gap-report.pdf).

Legend: ✅ equivalent · 🟡 partial · ❌ missing · ⛔ not planned (see ROADMAP "Not planned").
Impact, for a Laravel developer building a typical SaaS or business app: **B** blocker,
**Maj** major, **Min** minor. "Plan" is the milestone that addresses a gap (ROADMAP M18–M21).

## Verdict

Not yet. For its niche (HTML-over-the-wire apps, one server or a few, deployed as one binary)
Renox is solid, and in several areas ahead of Laravel. Head to head, two kinds of gaps remain:

- **Maturity, not code.** No release (crates.io has 0.0.1 placeholders, no tags), one maintainer,
  days of public history, no third-party packages, no docs site, tutorials or community. Only
  v1.0 and time fix these.
- **Features a typical SaaS needs:** multi-tenancy primitives, roles and permissions, account
  pages, scheduling beyond daily (and real time zones), locks and unique jobs, template components
  and a UI kit, JSON/session test assertions and error reporting. M18–M21 cover them.

### Maturity signals (at M17)

| Signal | Renox |
|---|---|
| Releases | none; apps are pinned to a git commit |
| Public history | first commits 2026-09-27; 1 contributor |
| Issues, stars, forks | 0 |
| Code | ~34k lines of Rust in `crates/`, ~5.3k in 11 examples |
| Tests | ~400 test functions, plus doctests (README, CHEATSHEET, guides) and `compile_fail` checks |
| CI | lint ×3 feature sets, tests on 3 OSes, PostgreSQL, chaos ×2, MSRV, feature matrix, generated app ×2, Docker, S3, cargo-deny, coverage |
| Docs | README, CHEATSHEET (compiled), 6 guides, 11 example READMEs, llms.txt, agent guides |

### Where Renox is already better

- **Deploy and operations:** one binary runs the web server, queue workers and scheduler; views,
  translations, assets and migrations are embedded. No Redis, Supervisor, cron entry or Octane.
  `rnx make:deploy` output is built and started in CI. A chaos test stops, pauses and locks the
  database under a running app.
- **Security by default:** CSP with nonces and security headers, uploads checked by content and
  served sandboxed, a three-way login lock shared across servers, constant-time login, verified
  and idempotent webhooks, typed forms (no mass assignment), same-site redirects only.
- **Data layer:** no N+1 by design (no lazy relations; loaders take a page of rows), typed columns
  checked by the compiler and identical on SQLite and PostgreSQL, `first_or_create` safe under
  races, large `IN` lists and bulk inserts that respect bind limits, transactional job dispatch
  (`dispatch_in`), a migrator with a lock, checksums and all-or-nothing rollback.
- **htmx:** 422 errors placed next to fields with no per-form code, `.fragment()`, htmx test
  helpers. SEO, sitemaps and GA4 built in. Live reload without Vite.
- **Tests:** every test gets its own migrated database, in parallel, with no `RefreshDatabase`.
- **Coding agents:** llms.txt, a compiled cheat-sheet, and an AGENTS.md in every new app linking
  the docs of the pinned Renox commit.

### Bugs and traps found

| Finding | Where | Plan |
|---|---|---|
| `User::set_password` also logs out the current session (the fingerprint changes); the handler must `auth::login` again. CHEATSHEET said "other sessions end" (fixed with this review). | `auth/user.rs:137`, `auth/mod.rs:347` | M18: `auth::change_password` |
| `auth::logout` ends every session of the user, on every device (deliberate since M13a); there's no "log out this device" | `auth/mod.rs:283` | M18 |
| `APP_TIMEZONE` accepts only UTC or a fixed offset (`+07:00`); `Asia/Jakarta` is refused and DST is impossible | `schedule.rs:283` | M20 (fixed in M20a) |
| A bad `Path<i64>` value gets axum's plain-text 400, not a 404 or Renox's error page (read, not run) | no rejection mapping found | M21 |
| Expired rows of the database cache store (and counters) are never pruned | `cache.rs`, `counters.rs` | M20 |

## 1. HTTP layer: routing, requests, responses, validation

**Equivalent (✅):** verbs and `{id}`/`{*rest}` parameters, named routes (duplicates fail at boot),
prefixed groups, `route:list` with guards, method spoofing, `App::layer` / `route_layer`
middleware, `auth`/`verified`/`guest`/`throttle` guards, extractors instead of DI, typed input
(`Form`, `Query`, `Json`, `Valid<T>`), uploads (`Upload`, `Vec<Upload>`), `ClientIp` +
`TRUSTED_PROXIES`, old input, plain and encrypted cookies, views/JSON/status, downloads and
streams, redirects and `Back`, htmx helpers, cookie sessions with flash/pull/reflash, CSRF
(field, header, per-route exclusion), `route()`/`url`/`absolute_url`, signed URLs, `asset()` with
content hash, `abort*`, custom error pages, rate limits shared across servers, CORS per route,
maintenance mode. Validation: presence, size, string, date, comparison, file, `unique`/`exists`,
arrays and nested, custom `Rule`, localized messages, every field's errors at once.

| Gap | Laravel | Renox today | St | Imp | Plan |
|---|---|---|---|---|---|
| Scoped `unique`/`exists` | `Rule::unique(...)->where('team_id', …)->ignore($id)` | `unique(table, col).ignore(id)` only | 🟡 | **B** (multi-tenant) | M18 |
| Resource routes and scaffolding | `Route::resource`, `make:controller --resource` | none; `make:module` writes an index route | ❌ | Maj | M21 |
| Error reporting hook | `report()`, Sentry/Flare | `tracing::error!` only | ❌ | Maj | M21 |
| Named, dynamic rate limiters | `RateLimiter::for('api', fn)` | fixed window per user or IP; `Limiter` is crate-private | ❌ | Maj | M21 |
| Query parameters in `route()` | extra params become the query string | extra params are an error | ❌ | Maj | M21 |
| Validation rules | ~100 rules | missing: `required_without*`, `prohibited*`, `alpha*`, `uuid`, `ip`, `json`, `starts_with`, `size`, `gt/lt`, `decimal`, `distinct`, `dimensions`, `Password`, `current_password` | 🟡 | Maj | M18 (password), M21 (rest) |
| Form Request hooks | `authorize`, `prepareForValidation`, `after`, async rules | `Validate::rules` only; sync `Rule` | 🟡 | Maj | M21 |
| Log channels, JSON logs, request id | channels, formatters, `withContext` | stdout `tracing` + `RUST_LOG` | 🟡 | Maj | M21 |
| Server-side sessions | file/db/redis drivers | encrypted cookie (~4 KB), last write wins across parallel requests | ⛔/🟡 | Maj (large sessions) | M21 (opt-in `SESSION_DRIVER=database`) |
| Subdomain routing, fallback route | `Route::domain`, `Route::fallback` | none; fallback taken by `public/` then 404 | ❌ | Maj (tenant subdomains) | M21 |
| Route model binding | implicit, custom keys, scoped | `Path(id)` + `find_or_404` | ❌ | Min | – |
| Optional params, `where` constraints, `any`, `redirect`/`view` routes | yes | none (bad `Path<i64>` → 400) | ❌ | Min | M21 (404 mapping) |
| `can:` middleware | `->can('update', 'post')` | handler or extractor | ❌ | Maj | M18 (`require_gate`) |
| Terminable middleware, ETag/cache headers, trusted hosts | yes | none | ❌ | Min | – |
| `routeIs`, current route name in views | yes | `request.path` only | ❌ | Min | M21 |
| `redirect()->route()`, public `intended()` | yes | build the URL; `intended` crate-private | 🟡 | Min | M21 |
| Session `push/increment/keep/now` | yes | none | ❌ | Min | M21 |
| Named error bags | yes | one bag per page | ❌ | Min | – |
| `XSRF-TOKEN` cookie for SPAs | yes | none | ❌ | Min | – |

## 2. Data layer: query builder, models, relations, migrations

**Equivalent (✅):** where/or-groups/in/null/between/like (case-insensitive on both databases),
`when`, sub-query `where_in_query`, aggregates, pluck, limit/offset, create/save/`insert_many`/
`upsert`, bulk `update`/`increment`/`delete`, `chunk` (by id), find/first/`*_or_404`,
`first_or_create`, soft deletes, typed casts (`bool`, chrono, `Json<T>`, `DbEnum`, `Uuid`),
serialization through serde, factories and seeders, length-aware pagination that keeps filters,
belongs-to/has-many/many-to-many loaders, `attach`/`detach`/`sync`, SQL joins read with
`fetch_as`, migrations with batches, rollback, fresh, status and checksums, error helpers,
validation `unique`/`exists`.

| Gap | Laravel | Renox today | St | Imp | Plan |
|---|---|---|---|---|---|
| Global scopes (tenancy) | `addGlobalScope`, tenancy packages | soft deletes only | ❌ | **B** (multi-tenant) | M18 |
| Raw fragments in the builder | `whereRaw`, `selectRaw`, `orderByRaw`, `groupBy`, `having` | whole statement with `sql()` only | ❌ | Maj | M19 |
| Pessimistic locking | `lockForUpdate`, `sharedLock` | none (`begin_immediate` crate-private) | ❌ | Maj | M19 |
| Aggregate loaders | `withCount`, `withSum`, `withExists` | raw `GROUP BY` | ❌ | Maj | M19 |
| Existence queries | `whereHas`, `whereDoesntHave`, `has('>', n)` | `where_in_query` (no NOT, no counts) | 🟡 | Maj | M19 |
| Non-integer keys | UUID/ULID/string keys, `HasUuids` | `id: i64` required | ❌ | Maj | M19 |
| Model events and observers | `creating`, `saved`, `deleted`, observers | none | ❌ | Maj | M19 |
| Dirty tracking, partial updates | `isDirty`, `getChanges`, UPDATE of changed columns | `save` writes every column | ❌ | Maj (lost updates) | M19 |
| Pivot data | `withPivot`, pivot timestamps, custom pivot models | two id columns only | ❌ | Maj | M19 |
| Polymorphic relations | morphTo/morphMany/morphToMany | none | ❌ | Maj | M19 |
| Cursor and simple pagination | `cursorPaginate`, `simplePaginate` | `paginate` only | ❌ | Min–Maj (APIs) | M19 |
| Transactions | `DB::transaction(fn, attempts)`, savepoints | `begin`/`commit`, no retry, no nesting | 🟡 | Min | M19 |
| hasOneOfMany, hasManyThrough | yes | none | ❌ | Min | M19 (docs pattern) |
| `firstOrNew`, `updateOrCreate`, `refresh`, persisted `touch` | yes | none | ❌ | Min | M19 |
| Factory states, sequences, `has()/for()` | yes | struct update syntax | 🟡 | Min | M21 |
| Query log / `DB::listen`, `toSql` | yes | none | ❌ | Min | M19 (`to_sql`), M21 (inspector) |
| Schema builder | `Blueprint` | plain SQL per database | ⛔ | Maj for some | – |
| MySQL, read/write split, several connections | yes | SQLite + PostgreSQL, one `Db` | 🟡 | Min | – |
| Redis | cache/queue/session | database | ⛔ | Min (Maj at scale) | – |

## 3. Authentication, authorization, security

**Equivalent (✅):** login/register/logout pages (overridable, en/id), extra registration fields
and hook, password reset (hashed tokens, throttled, same answer for unknown emails), email
verification, `attempt`/`login`/`logout`, `AuthUser` extractors, guards, login throttling,
Argon2id, encrypted cookies, personal access tokens (hashed, expiry, last used, `token_id`),
gates (sync and async), policies, CSRF, CORS, CSP and security headers, session revocation,
signed URLs, trusted proxies, upload checks, webhook verification, `acting_as` in tests.

| Gap | Laravel | Renox today | St | Imp | Plan |
|---|---|---|---|---|---|
| Roles and permissions | spatie/laravel-permission | a `role` column + gates by hand | ❌ | **Maj** | M18 |
| Account pages | Breeze/Jetstream profile: name, email (re-verify), password, delete account | `set_password`/`set` only | ❌ | **Maj** | M18 |
| Route-level gate, `Gate::before` | `can:` middleware, super-admin `before` | none | ❌ | Maj | M18 |
| Password rules, password confirmation | `Password::min()->mixedCase()->uncompromised()`, `password.confirm` | `min(8)` hard-coded | 🟡 | Maj | M18 |
| Auth events, audit log | Login, Failed, Lockout, Registered, … ; activitylog | none | ❌ | Maj (B2B) | M18 |
| Token abilities | `tokenCan`, `abilities` middleware | none | ❌ | Maj | M18 |
| Log out this device / other devices | `logoutOtherDevices` | `logout` and `revoke_sessions` end all sessions | 🟡 | Min | M18 |
| Public encryption API, encrypted fields | `Crypt`, `encrypted` casts | session and cookies only | ❌ | Maj (PII, secrets) | M19 (`Encrypted<T>`) |
| Importing Laravel users | bcrypt hashes, `needsRehash` | Argon2id only | 🟡 | Maj (migrations) | M18 |
| 2FA (TOTP, recovery codes) | Fortify/Jetstream | none | ⛔ | **Maj** | plugin `renox-2fa` |
| Social login | Socialite | none | ⛔ | **Maj** | plugin `renox-oauth` |
| Several user types / guards | admin and customer tables | one `users` table | ❌ | Maj | document "one table + roles"; revisit |
| Teams | Jetstream | none | ❌ | Maj | after M18 (roles + tenancy make it an example) |
| Impersonation, HTTP Basic, OAuth2 server | packages / Passport | none | ❌/⛔ | Min | – |

## 4. Background work and services

**Equivalent (✅):** a database queue on SQLite or PostgreSQL (`SKIP LOCKED`), named queues,
delays, retries/backoff/timeout, permanent errors, `dispatch_in` (better than `afterCommit`),
failed jobs with retry/flush, graceful shutdown, crash recovery (chaos-tested), events and
listeners, `withoutOverlapping` and `onOneServer` by default, `schedule:list/work`, mail with
HTML and text, cc/bcc/reply-to/from, attachments, queued mail, a preview page, notifications on
mail/database/custom channels, on-demand recipients, queued notifications, an inbox API, cache
(memory or database) with single-flight `remember`, local/S3 storage with temporary URLs,
JSON translations with fallback, `number`/`date` filters.

| Gap | Laravel | Renox today | St | Imp | Plan |
|---|---|---|---|---|---|
| Schedules beyond daily, time zones | `cron()`, weekly, monthly, weekdays, `between`, `timezone()` | every/minute/hourly/daily; fixed offset only | 🟡 | **Maj** | M20 |
| Atomic locks | `Cache::lock()->get/block` | none | ❌ | Maj (payments, double submit) | M20 |
| Unique jobs | `ShouldBeUnique`, `uniqueFor` | none | ❌ | Maj | M20 |
| Job middleware | `RateLimited`, `WithoutOverlapping`, `ThrottlesExceptions` | none | ❌ | Maj | M20 |
| Chains and batches | `Bus::chain`, `Bus::batch` with progress | by hand | ❌ | Maj | M20 |
| Localized mail and notifications | `->locale()`, `HasLocalePreference` | `t()` not available in mail templates | ❌ | Maj (en/id apps) | M20 |
| Per-recipient channels | `via($notifiable)` | `channels()` doesn't see the recipient | 🟡 | Maj | M20 |
| HTTP client with fakes | `Http::retry()->timeout()`, `Http::fake` | none for apps | ❌ | Maj | M20 |
| Queue dashboard and metrics | Horizon, Pulse, `queue:monitor` | `/health` counts only | ❌ | Maj | M20 |
| Schedule hooks and pings | `onFailure`, `pingBefore`/`thenPing` | log line on failure | ❌ | Maj (monitoring) | M20 |
| Queue priority | `--queue high,default` drains in order | list is only a filter | 🟡 | Maj | M20 |
| Cache `add`/`pull`/`increment`, tags, pruning | yes | put/get/forget/remember; expired DB rows kept | 🟡 | Min | M20 |
| `dispatch_sync`, encrypted payloads, `failed()` hook, `queue:forget/prune-failed` | yes | none | ❌ | Min | M20 |
| Plural ranges (`{0}`, `[2,*]`), locale from `Accept-Language` | yes | `one\|many`; session/`APP_LOCALE` | 🟡 | Min | M21 |
| Multiple mailers/failover, API mail drivers | SES, Postmark, Resend | SMTP (they all offer SMTP) | 🟡 | Min | – |
| Several named disks, directory ops, streams | yes | one disk; put/get/exists/delete | 🟡 | Min | M20 |
| Broadcasting (Reverb/Pusher) | yes | none | ⛔ | Min–Maj | – |

## 5. Views and frontend

**Equivalent (✅):** inheritance, blocks and `super()`, include/import/macros, `for … else` and
`loop.*`, autoescaping, raw blocks, `tojson`, custom functions and filters (`App::templates`),
data for every view (`App::share`, async), `csrf_field`/`method_field`/`old`/`error`, redirect
back with errors, flash, `.fragment()`, htmx and Alpine bundled (with a CSP build), hashed
assets, embedded templates, live reload, `t()`, `number`/`date`, `can()`, overridable built-in
pages, SEO and sitemaps, semi-strict templates while debugging.

| Gap | Laravel | Renox today | St | Imp | Plan |
|---|---|---|---|---|---|
| Components that see the request | `<x-input>` with `old()`, `$errors`, `__()` | imported macros can't see `old`/`error`/`t`/`csrf`/`auth` | 🟡 | **B/Maj** | M21 |
| UI kit and `make:component` | Flux, Filament, Breeze components | none | ❌ | Maj | M21 |
| Admin / CRUD scaffolding | Filament, Nova | hand-written (examples/shop) | ❌ | Maj | M21 (`make:module --resource`), plugin `renox-admin` |
| Toasts over htmx | Livewire/Filament notifications | flash on full pages only | ❌ | Maj | M21 |
| `@push`/`@stack`/`@once` | yes | layout blocks only | ❌ | Maj | M21 |
| Error pages in the app layout | full context | only status/reason/detail | 🟡 | Maj | M21 |
| Live validation | Precognition | none | ❌ | Maj | M21 |
| Tailwind | Vite + Tailwind | bring your own CLI | ⛔/❌ | Maj for Tailwind users | M21 (standalone CLI in `rnx serve`) |
| Several fragments, out-of-band swaps, more htmx headers | `fragments([...])` | one block; `HxRedirect/Refresh/Trigger` | 🟡 | Maj | M21 |
| `@class`, `@checked`, `break`/`continue`, `truncate` | yes | `if` only; `loop_controls` and minijinja-contrib off | ❌ | Min | M21 |
| Livewire / Inertia | stateful components / SPA | htmx + Alpine recipes | 🟡/⛔ | Maj for SPA teams | – |
| Markdown mail components | `x-mail::button/panel/table` | a layout and a button | 🟡 | Min | M20 |

## 6. Tooling, testing, operations, ecosystem

**Equivalent (✅):** generators for module, model (+migration), policy, job, command, mail,
migration and deploy (they register themselves), migrate/seed/status, `route:list`, `db:shell`,
`down`/`up`, `schedule:*`, `queue:*`, `key:generate`, `rnx serve` with reload, parallel tests
with isolated databases, HTTP test helpers and status assertions, `assert_invalid`, database
assertions, a memory mailer, `queued_jobs`/`run_jobs`, coverage in CI, rustfmt/clippy, typed
state instead of a container, `make:deploy` (Docker, systemd, Litestream).

| Gap | Laravel | Renox today | St | Imp | Plan |
|---|---|---|---|---|---|
| Release, support policy, community | yearly majors, LTS windows, Laracasts, a big hiring pool | pre-1.0, git pins, 1 maintainer | ❌ | **B** | v1.0 |
| Ecosystem packages | Cashier, Scout, Socialite, Pennant, Filament… | none | ❌ | **B** | plugins after M18–M21 |
| JSON assertions | `assertJson`, `assertJsonPath`, `assertJsonStructure` | `res.json::<T>()` + `assert_eq!` | ❌ | Maj | M21 |
| Session, auth, view assertions | `assertSessionHas`, `assertAuthenticated`, `assertViewHas` | none | ❌ | Maj | M21 |
| Time travel | `travelTo`, `freezeTime` | no clock abstraction | ❌ | Maj | M21 |
| Fakes | Event, Notification, Http, Storage, Bus fakes | mail and queue only (names) | 🟡 | Maj | M20 (Http), M21 |
| Generators | ~35 `make:*` | 9 | 🟡 | Maj | M21 (factory, seeder, test, notification, event, rule, middleware, component, resource) |
| Observability | Telescope, Debugbar, Pulse, Sentry | `/health`, mail preview | ❌ | Maj | M21 (`/_renox/debug`, report hook) |
| Typed commands, prompts, output helpers | signature DSL, Laravel Prompts | `Args` with strings | 🟡 | Maj | M21 |
| Browser tests | Dusk | none (manual CDP checks) | ❌ | Maj | M21 (recipe) |
| Docs site, API reference | laravel.com, docs for every package | markdown, not on docs.rs | 🟡 | Maj | v1.0 |
| Zero-downtime deploys, hosted platforms | Forge, Vapor, Cloud, Envoyer | systemd restart | 🟡 | Maj | M21 (recipes) |
| Tinker | REPL | `db:shell` + commands | ⛔ | Maj for some | – |

## Plan

See ROADMAP.md, milestones M18–M21, then v1.0:

1. **M18 · SaaS foundations:** tenancy (global scopes, scoped `unique`/`exists`), roles and
   permissions, `require_gate` and `gate_before`, account pages, password rules and confirmation,
   auth events and an audit log, token abilities, per-device logout, bcrypt import.
2. **M19 · Data layer 2:** raw fragments and `group_by`, locking, aggregate loaders, `where_has`,
   non-integer keys, model hooks, partial saves, pivot data, polymorphic relations, cursor and
   simple pagination, transaction retries, `Encrypted<T>`.
3. **M20 · Background 2:** cron and calendar schedules with IANA time zones, locks, unique jobs,
   job middleware, chains and batches, localized mail and notifications, an HTTP client with
   fakes, schedule hooks, a queue dashboard, priorities, pruning.
4. **M21 · Views and DX:** components with request context, a UI kit, toasts, error pages in the
   layout, resource scaffolding, Tailwind, more fragments and htmx headers, test assertions,
   time travel, fakes, generators, error reporting and JSON logs, named rate limiters, more
   validation rules, form-request hooks, `route()` query parameters.
5. **Plugins:** `renox-oauth`, `renox-2fa`, `renox-admin`; billing later.
6. **v1.0:** releases, the docs site and tutorial, a support policy.
