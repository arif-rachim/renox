# Scheduler, events, cache and commands

The pieces of an app that don't answer a request: tasks that run on a clock, events that let
modules react to each other, a cache for expensive results and locks, and the app's own
command-line commands. All of them are defined in code, compiled into the app binary, and run
by it: `serve` runs the scheduler next to the web server and the queue workers, and
`my-app <command>` runs a command. The short version is in the [cheat-sheet](../CHEATSHEET.md)
("Jobs, events, schedule, mail" and "Cache, session, uploads, translations"); the queue has its
own guide, [queue.md](queue.md). Complete apps: [examples/jobs](../examples/jobs) (scheduled
reports with a lock and a failure alert, an event, `App::report`) and
[examples/hello](../examples/hello) (a typed `entries:prune` command, a scheduled task).

## Scheduled tasks

`App::schedule` (or `Registry::schedule()` in a module's `register`) takes the tasks. Each
has a name, unique in the app, and an async function of `AppState`:

```rust
use renox::prelude::*;
use renox::chrono::Weekday;
use std::time::Duration;

async fn sync_stock(_state: AppState) -> Result { Ok(()) }
async fn report(_state: AppState) -> Result { Ok(()) }

fn app() -> App {
    App::new().schedule(|s| {
        s.every_minute("heartbeat", |_state| async move { Ok(()) });
        s.every_minutes(5, "sync-stock", sync_stock) // :00, :05, :10 … on the clock
            .weekdays()
            .between("08:00", "17:00");
        s.every(Duration::from_secs(30 * 60), "refresh-rates", report);
        s.hourly("hourly-report", report);
        s.daily_at("02:00", "cleanup", |state| async move {
            renox::db::sql("DELETE FROM carts WHERE updated_at < ?")
                .bind(renox::db::now() - renox::chrono::TimeDelta::days(30))
                .execute(&state.db)
                .await?;
            Ok(())
        });
        s.weekly_on(Weekday::Mon, "07:00", "weekly-report", report);
        s.monthly_on(1, "00:05", "invoices", report); // day 29–31 skips shorter months
        s.cron("30 9 * * 1-5", "standup", report) // minute hour day month weekday
            .timezone("Europe/Amsterdam");
    })
}
```

| Method | Runs |
|---|---|
| `every_minute(name, task)` | every minute, on the minute |
| `every_minutes(n, name, task)` | every `n` minutes, aligned to the clock (every 15: :00, :15, …) |
| `every(duration, name, task)` | every `duration`, aligned to the clock |
| `hourly(name, task)` | every hour, on the hour |
| `daily_at("HH:MM", name, task)` | every day at that time |
| `weekly_on(Weekday, "HH:MM", name, task)` | once a week (`renox::chrono::Weekday`) |
| `monthly_on(day, "HH:MM", name, task)` | on that day of each month (use 28 or lower for every month) |
| `cron(expr, name, task)` | when the wall clock matches a five-field cron expression |

Cron expressions take `*`, ranges `a-b`, lists `a,b`, steps `*/15`, names (`MON`, `JAN`;
Sunday is `0`, `7` or `SUN`), and `@hourly`, `@daily`, `@weekly`, `@monthly`, `@yearly`.

Each method returns a `ScheduledTask` to narrow or hook into:

- **Filters:** `weekdays()`, `weekends()`, `days(&[Weekday::Sat])`, and
  `between("08:00", "17:00")` (both ends included; `"22:00"` to `"06:00"` spans midnight).
  Filters only drop runs: `every_minutes(5, …).between(…)` still runs on the 5-minute marks.
- **Time zone:** `timezone("Asia/Jakarta")` for this task instead of `APP_TIMEZONE` (also
  `+07:00` or `UTC`).
- **Hooks:** `on_failure(|state, err| async move { … })` after a run that failed or panicked,
  `on_success(|state| async move { … })` after one that succeeded.
- **Pings** (health checks such as Healthchecks.io, Cronitor or Better Stack):
  `ping_before(url)`, `then_ping(url)` (after every run), `ping_on_success(url)`,
  `ping_on_failure(url)`. They are GET requests through `state.http` (10 s timeout, one retry);
  a failing ping is logged and never stops the task.

```rust
use renox::prelude::*;

async fn backup(_state: AppState) -> Result { Ok(()) }

async fn alert(state: AppState, err: Error) {
    let to = state.config.var("ALERT_EMAIL").unwrap_or_else(|| "ops@example.com".into());
    let mail = renox::mail::Mail::new(to, "The backup failed", format!("{err:?}"));
    if let Err(err) = state.queue_mail(mail).await {
        eprintln!("could not queue the alert: {err:?}");
    }
}

fn app() -> App {
    App::new().schedule(|s| {
        s.daily_at("03:00", "backup", backup)
            .ping_before("https://hc-ping.com/your-uuid/start")
            .ping_on_success("https://hc-ping.com/your-uuid")
            .ping_on_failure("https://hc-ping.com/your-uuid/fail")
            .on_failure(alert);
    })
}
```

Mistakes are caught when the app boots, before anything runs: a bad time (`"25:00"`), cron
expression or zone name, and a name used twice, fail, e.g. with ``invalid schedule: task `x` is
scheduled twice``.

### Time zones and daylight saving time

Times are in `APP_TIMEZONE`: `UTC` (the default), a fixed offset such as `+07:00`, or an IANA
name such as `Asia/Jakarta` or `Europe/Amsterdam`, with its daylight saving rules. A task's
own `timezone(…)` wins over it.

- A run due in the hour that clocks skip in spring (02:30 in Amsterdam) runs right after the
  jump, at 03:00.
- A run due in the hour that repeats in autumn runs once, the first time.
- Interval tasks (`every…`, `hourly`) are aligned to the local clock with the offset in force,
  so `every(Duration::from_secs(2 * 3600), …)` in `+07:00` runs at even local hours.

### What runs them

| Command | What it does |
|---|---|
| `my-app serve` (`rnx serve`) | runs the scheduler beside the web server, unless `SCHEDULER=false` |
| `my-app schedule:work` | only the scheduler, until stopped (for `SCHEDULER=false` deploys) |
| `my-app schedule:list` | each task's next run, its zone and name (`never` if filters exclude every run) |
| `my-app schedule:run NAME` | runs one task now, with its hooks and pings, whatever its schedule |

- **Overlaps:** a task whose previous run is still going in the same process is skipped (and
  logged), not run twice. Guard longer work, or a task also started by hand with
  `schedule:run`, with a cache lock (see "Locks" below; examples/jobs does).
- **Several instances:** servers sharing one database may all run the scheduler. Before each
  run, the scheduler claims it with a `renox:schedule:<task>:<slot>` row in the `cache` table
  (an `INSERT … ON CONFLICT DO NOTHING`, whatever `CACHE_STORE` is), so exactly one process runs
  each slot. If the database can't be reached, the task runs anyway, as a single process would.
  `schedule:run` doesn't claim: it always runs.
- **Failures:** an error or a panic is logged, `on_failure` runs, and the error goes to every
  reporter added with `App::report` (an `ErrorReport` with `kind: ReportKind::ScheduledTask`
  and the task's name in `source`), e.g. to post it to a chat:

```rust
use renox::prelude::*;
use renox::report::{ErrorReport, ReportKind};

fn app() -> App {
    App::new().report(|report: ErrorReport, _state: AppState| async move {
        if report.kind == ReportKind::ScheduledTask {
            eprintln!("task {:?} failed: {}", report.source, report.message);
        }
    })
}
```

Tasks run in their own `renox::context` scope, so `context::app()` works inside them, and the
clock they read is `renox::db::now()` (moved by `TestApp::travel` in tests).

### Housekeeping

Several tables grow until something prunes them. Each has a command (`cache:prune`,
`session:prune`, `queue:prune-failed`, `queue:prune-batches`, and with the modules
`tokens:prune`, `notifications:prune`, `audit:prune`), and a function to call from one daily
task:

```rust
use renox::prelude::*;
use std::time::Duration;

const DAY: Duration = Duration::from_secs(24 * 60 * 60);

async fn housekeeping(state: AppState) -> Result {
    let db = &state.db;
    let cache = state.cache.prune().await?; // expired rows (CACHE_STORE=database)
    let sessions = Session::prune_expired(db).await?; // SESSION_DRIVER=database
    let tokens = renox::auth::prune_expired_tokens(db, DAY).await?; // expired over a day ago
    let read = renox::auth::prune_read_notifications(db, 30 * DAY).await?; // read over 30 days ago
    let audit = renox::audit::prune(db, 365 * DAY).await?; // the Audit module's log
    let failed = state.queue.prune_failed(7 * DAY).await?; // failed_jobs
    let batches = state.queue.prune_batches(DAY).await?; // finished job_batches
    println!("pruned {cache} cache, {sessions} sessions, {tokens} tokens, {read} notifications, \
              {audit} audit entries, {failed} failed jobs, {batches} batches");
    Ok(())
}

fn app() -> App {
    App::new()
        .module(Auth::new())
        .module(renox::audit::Audit)
        .schedule(|s| {
            s.daily_at("04:00", "housekeeping", housekeeping);
        })
}
```

Each returns the number of rows deleted (`Result<u64>`). Call only those whose tables the app
has: the token and notification tables come with `Auth`, `audit_logs` with `Audit`. The
`sessions` and `cache` tables are in every app.

### Testing a task

`TestApp` doesn't start the scheduler. Run a task by name with `kernel().run_scheduled(name)`
(what `schedule:run` does), then assert what it did:

```rust
use renox::prelude::*;
use renox::testing::TestApp;

fn app() -> App {
    App::new().schedule(|s| {
        s.daily_at("02:00", "count", |state| async move {
            state.cache.increment("runs", 1).await?;
            Ok(())
        });
    })
}

async fn the_task_counts() -> Result {
    let app = TestApp::new(app()).await;
    app.kernel().run_scheduled("count").await?;
    assert_eq!(app.state().cache.get::<i64>("runs").await?, Some(1));
    Ok(())
}
```

## Events

An event says that something happened; listeners decide what follows. The module that places
orders emits `OrderPlaced`, and the stock and mail modules react to it without the order
module knowing them.

```rust
use renox::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug)]
struct OrderPlaced {
    order_id: i64,
}

impl Event for OrderPlaced {} // Clone + Send + Sync + 'static

#[derive(Serialize, Deserialize)]
struct SendReceipt { order_id: i64 }

impl Job for SendReceipt {
    const NAME: &'static str = "send-receipt";
    async fn handle(self, _ctx: JobContext) -> Result { Ok(()) }
}

struct Stock;

impl Module for Stock {
    fn name(&self) -> &'static str { "stock" }

    fn register(&self, app: &mut Registry) {
        app.listen(|event: OrderPlaced, state| async move {
            renox::db::sql("UPDATE products SET stock = stock - 1 WHERE id IN \
                            (SELECT product_id FROM order_items WHERE order_id = ?)")
                .bind(event.order_id)
                .execute(&state.db)
                .await?;
            Ok(())
        });
    }
}

fn app() -> App {
    App::new()
        .job::<SendReceipt>()
        .listen(|event: OrderPlaced, state| async move {
            state.dispatch(SendReceipt { order_id: event.order_id }).await?; // slow work: queue it
            Ok(())
        })
        .module(Stock)
}

async fn place(State(state): State<AppState>) -> Result<Redirect> {
    state.emit(OrderPlaced { order_id: 1 }).await?;
    Ok(Redirect::to("/orders"))
}
```

- `state.emit(event)` runs every listener of that type **now, in order, in the caller**:
  those added with `App::listen` first, then the modules' (registered at boot), each module's
  in its own order. Each listener gets a clone of the event.
- If one fails (or panics), the others still run; `emit` returns the first error, so a
  handler's `?` turns it into a 500. Every failure is logged.
- There are no queued listeners: a listener that does slow work (mail, another service)
  dispatches a job, as above, and returns.
- Built-in events (`LoggedIn`, `LoginFailed`, `Registered`, …) are in `renox::auth::events`;
  see the cheat-sheet's "Auth events and the audit log".

`rnx make:event OrderPlaced --module orders` writes the event in the module and registers a
listener for it in the module's `register`.

### Testing events

`app.fake_events()` records events instead of running their listeners, from then on:

```rust
use renox::prelude::*;
use renox::testing::TestApp;

#[derive(Clone, Debug)]
struct OrderPlaced { order_id: i64 }
impl Event for OrderPlaced {}

async fn placing_an_order_emits_the_event() -> Result {
    let app = TestApp::new(App::new()).await;
    app.fake_events();
    app.state().emit(OrderPlaced { order_id: 7 }).await?; // usually: app.post("/orders", …)
    app.assert_emitted::<OrderPlaced>(|e| e.order_id == 7);
    assert_eq!(app.emitted::<OrderPlaced>().len(), 1);
    Ok(())
}
```

`assert_not_emitted::<E>()` checks that none was. To test a listener itself, leave events real
and assert what it did (rows, `app.queued_jobs()`, `app.sent_mail()`).

## Cache

`state.cache` keeps values for a while, as JSON. `CACHE_STORE` picks the store:

| `CACHE_STORE` | Where | Shared with |
|---|---|---|
| `memory` (default) | this process | nothing: lost on restart |
| `database` | the `cache` table | `queue:work` processes and other servers on the same database |

With `database`, rate limits, the login lock and the queue's job middleware are shared between
servers too.

```rust
use renox::prelude::*;
use std::time::Duration;

async fn demo(state: AppState) -> Result {
    let cache = &state.cache;
    let count: i64 = cache
        .remember("products.count", Duration::from_secs(60), || async {
            renox::db::sql("SELECT COUNT(*) FROM products").scalar(&state.db).await.map_err(Into::into)
        })
        .await?; // computed once a minute; errors are not cached

    cache.put("banner", &"Sale today", Some(Duration::from_secs(3600))).await?; // None: no expiry
    let banner: Option<String> = cache.get("banner").await?;
    let present = cache.has("banner").await?;
    let first = cache.add("welcome-sent:7", &true, None).await?; // false if already there
    let code: Option<String> = cache.pull("otp:7").await?; // read and remove
    let views = cache.increment("views:home", 1).await?; // atomic; 1 the first time
    cache.decrement("stock:42", 1).await?;
    cache.forget("banner").await?;
    cache.flush().await?; // everything the app cached; keeps the framework's `renox:` rows
    let _ = (count, banner, present, first, code, views);
    Ok(())
}
```

- `get` errors when the stored value is another type; `remember` computes again instead (e.g.
  after a deploy changed the type). Concurrent `remember` calls for one key in a process compute
  once.
- `add` and `increment` are atomic, so of several callers exactly one `add` returns `true`, and
  no count is lost. A missing key counts as 0 and never expires.
- Keys starting with `renox:` belong to the framework (scheduler claims, locks, counters,
  unique jobs, queue throughput): don't use that prefix.
- Expired rows of the database store are deleted now and then as values are written, and by
  `cache.prune()` / `my-app cache:prune`.

### Locks

A lock lets one process at a time do something: send a report, work on an order. With
`CACHE_STORE=database` it holds across processes and servers; with `memory`, within one
process.

```rust
use renox::prelude::*;
use std::time::Duration;

async fn sales_report(state: AppState) -> Result {
    // Held for 5 minutes at most, so a crashed holder doesn't block others forever.
    let lock = state.cache.lock("sales-report", Duration::from_secs(300));
    let Some(guard) = lock.try_acquire().await? else {
        return Ok(()); // someone else is on it
    };
    // … build and send the report …
    guard.release().await?; // or let it drop (released in the background)
    Ok(())
}

async fn work_on_order(state: AppState, order_id: i64) -> Result {
    let lock = state.cache.lock(&format!("order:{order_id}"), Duration::from_secs(30));
    let guard = lock.block(Duration::from_secs(5)).await?; // waits up to 5 s, then 423 Locked
    // … one process at a time …
    drop(guard);
    Ok(())
}
```

`lock.is_held()` says whether anyone holds it; `lock.force_release()` frees it whoever holds it
(an admin command). `guard.release()` returns `false` when the `ttl` had already run out and
someone else took the lock.

## Commands

The app binary is its own command line, like artisan: `my-app migrate`, `my-app queue:work`,
`my-app help`. During development, `rnx <command>` runs the same through `cargo run`. Apps add
their own.

### A plain command

`App::command(name, about, run)` (or `Registry::command` in a module) takes the words after the
name as `Args`:

```rust
use renox::prelude::*;
use renox::command::Args;

async fn create_admin(state: AppState, args: Args) -> Result {
    let (Some(email), Some(password)) = (args.value("--email"), args.value("--password")) else {
        return Err(Error::BadRequest("usage: admin:create --email E --password P".into()));
    };
    User::register(&state.db, "Admin", email, password).await?;
    println!("Created {email}.");
    Ok(())
}

fn app() -> App {
    App::new()
        .module(Auth::new())
        .command("admin:create", "Create an admin user (--email, --password)", create_admin)
}
```

`Args` has `value("--flag")` (`--flag v` or `--flag=v`), `has("--flag")`, `positional()` (the
words that aren't flags or their values) and `all()`.

### A typed command (clap)

An `AppCommand` declares its arguments with clap (re-exported as `renox::clap`), so they are
parsed and checked for you and `my-app entries:prune --help` prints the usage. The doc comment
is its line in `my-app help`:

```rust
use renox::prelude::*;
use renox::clap;
use renox::command::AppCommand;

/// Delete entries older than --days (default 30).
#[derive(clap::Parser)]
#[command(name = "entries:prune")]
struct PruneEntries {
    /// Keep entries younger than this many days.
    #[arg(long, default_value_t = 30)]
    days: i64,
    /// Don't ask first.
    #[arg(long)]
    force: bool,
}

impl AppCommand for PruneEntries {
    async fn run(self, state: AppState) -> Result {
        let question = format!("Delete entries older than {} days?", self.days);
        if !self.force && !renox::prompt::confirm(&question, true).await? {
            return Ok(());
        }
        let cutoff = renox::db::now() - renox::chrono::TimeDelta::days(self.days);
        let deleted = renox::db::sql("DELETE FROM entries WHERE created_at < ?")
            .bind(cutoff)
            .execute(&state.db)
            .await?;
        println!("Deleted {deleted} entries.");
        Ok(())
    }
}

fn app() -> App {
    App::new().typed_command::<PruneEntries>()
}
```

`rnx make:command entries:prune --module guestbook` writes a typed command and registers it in
the module's `register`.

### How commands run

- After the app boots: the database is connected, but migrations are not run for you.
  Commands run in a `renox::context` scope, like jobs and requests.
- An error ends the command with `Error: …` and a non-zero exit code; a clap error prints what's
  wrong and the usage.
- Names can't clash: a name that is built in (`migrate`, `queue:work`, `serve`, `help`, …) or
  registered twice fails at boot, as does an empty name or one with spaces.
- Modules add their own: `tokens:prune` and `notifications:prune --days N` with `Auth`,
  `audit:prune --days N` with `Audit`.

### Asking questions

`renox::prompt` asks in a terminal, like Laravel's `ask`, `secret`, `confirm` and `choice`:

```rust
use renox::prelude::*;
use renox::prompt;

async fn create_user(state: AppState, args: renox::command::Args) -> Result {
    let email = match args.value("--email") {
        Some(email) => email.to_owned(),
        None => prompt::ask("Email").await?, // asks until something is typed
    };
    let name = prompt::ask_or("Name", "Admin").await?; // empty answer: the default
    let password = prompt::secret("Password").await?; // not echoed
    let role = prompt::choice("Role", &["admin", "staff"], Some("staff")).await?; // text or number
    if prompt::confirm(&format!("Create {email} as {role}?"), true).await? {
        User::register(&state.db, &name, &email, &password).await?;
    }
    Ok(())
}
```

Questions go to stderr. Without a terminal (a pipe, cron, CI), answers are read from stdin
line by line; when there is none, a question with a default takes it and one without fails,
naming the missing answer. So give commands that run unattended flags for everything (a
`--force`, as above).

### Testing commands

`app.kernel().call(name, args)` runs a command as the binary would; `renox::prompt::answering`
gives the answers, one per question, in order:

```rust
use renox::prelude::*;
use renox::testing::TestApp;

async fn the_command_asks_first(app: TestApp) -> Result {
    renox::prompt::answering(["no"], app.kernel().call("entries:prune", ["--days", "7"])).await?;
    app.assert_database_count("entries", 2).await; // nothing deleted
    renox::prompt::answering(["yes"], app.kernel().call("entries:prune", ["--days", "7"])).await?;
    Ok(())
}

async fn the_binary_runs_a_built_in() -> Result {
    App::new().run_args(["migrate:status"]).await // any command, output on stdout
}
```

Wrap the call in `app.at_travelled_time(…)` when the test moved the clock with `travel`
(examples/hello's `the_prune_command_deletes_old_entries` does).

## Coming from Laravel

| Laravel | Renox |
|---|---|
| `$schedule->command(…)->everyFiveMinutes()` | `s.every_minutes(5, name, task)` |
| `->hourly()`, `->dailyAt('02:00')`, `->weeklyOn(1, '7:00')`, `->monthlyOn(1, '00:05')` | `hourly`, `daily_at`, `weekly_on(Weekday::Mon, …)`, `monthly_on` |
| `->cron('30 9 * * 1-5')` | `s.cron("30 9 * * 1-5", name, task)` |
| `->weekdays()`, `->weekends()`, `->days([…])`, `->between()` | the same names on `ScheduledTask` |
| `->timezone()`, `schedule_timezone` | `.timezone(…)`, `APP_TIMEZONE` |
| `->onOneServer()`, `->withoutOverlapping()` | built in: each run is claimed in the database, and a task still running in the process is skipped (use a cache lock for more) |
| `->onFailure()`, `->onSuccess()` | `on_failure`, `on_success` |
| `->pingBefore()`, `->thenPing()`, `->pingOnSuccess()`, `->pingOnFailure()` | the same, snake_case |
| `schedule:work`, `schedule:list`, `schedule:test` / `schedule:run` | `schedule:work`, `schedule:list`, `schedule:run NAME` (no cron entry needed: `serve` runs it) |
| `Event::dispatch()`, `event(…)` | `state.emit(event).await?` |
| `Event::listen()`, `EventServiceProvider` | `App::listen`, `Registry::listen` in a module |
| `ShouldQueue` listeners | a listener that dispatches a job |
| `Event::fake()`, `Event::assertDispatched()` | `app.fake_events()`, `app.assert_emitted::<E>(…)` |
| `Cache::get/put/remember/add/pull/increment/forget/flush` | `state.cache.` the same names |
| `Cache::lock()->get()`, `->block(5)` | `cache.lock(name, ttl).try_acquire()`, `.block(Duration)` |
| `CACHE_DRIVER=database` | `CACHE_STORE=database` |
| `php artisan make:command`, `$signature` | `rnx make:command`, a clap `AppCommand` |
| `$this->ask/secret/confirm/choice` | `renox::prompt::ask/secret/confirm/choice` |
| `$this->artisan(…)->expectsQuestion()` | `prompt::answering([…], kernel.call(…))` |
| `Artisan::call()` | `kernel.call(name, args)`, `App::run_args` |
