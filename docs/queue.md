# The queue: jobs, retries, chains and batches

Work that doesn't belong in a request (sending mail, calling another service, generating a
report) goes on the queue. Renox's queue lives in the app's own database (tables `jobs`,
`failed_jobs`, `job_batches`, on SQLite or PostgreSQL), so there is no Redis to run, and
`serve` runs the workers itself (`QUEUE_WORKERS`, 2 by default). The short version of every
API is in the [cheat-sheet](../CHEATSHEET.md) ("Jobs, events, schedule, mail"); a complete app
is [examples/jobs](../examples/jobs). Running it in production (failed jobs, pruning) is in
[operations.md](operations.md).

## A job

A job is a serializable struct with a stable name. Keep it small: ids, not whole models (the row
may change before the job runs, and the payload is stored as JSON).

```rust
use renox::prelude::*;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Serialize, Deserialize)]
struct SendInvoice {
    order_id: i64,
}

impl Job for SendInvoice {
    const NAME: &'static str = "send-invoice"; // stored with each job: don't rename while queued
    const MAX_ATTEMPTS: u32 = 5;                // default 3
    const TIMEOUT: Duration = Duration::from_secs(30); // per attempt; default 60 s

    async fn handle(self, ctx: JobContext) -> Result {
        let mail = ctx.state.mail_view(
            "buyer@example.com",
            "Your invoice",
            "mail/invoice",
            context! { order_id => self.order_id },
        )?;
        ctx.state.mailer.send(mail).await
    }
}

fn app() -> App {
    App::new().job::<SendInvoice>() // every job type is registered once
}

async fn paid(State(state): State<AppState>) -> Result<String> {
    let id = state.dispatch(SendInvoice { order_id: 1 }).await?;
    state.queue.dispatch_after(SendInvoice { order_id: 2 }, Duration::from_secs(3600)).await?;
    Ok(format!("queued as job {id}"))
}
```

`rnx make:job SendInvoice --module orders` writes this and registers it.

## Failures and retries

- An error, a panic or a run past `TIMEOUT` is a failed attempt. The job waits `backoff(attempt)`
  (10 s × attempt by default) and runs again, up to `MAX_ATTEMPTS`; then it moves to
  `failed_jobs`, and its `failed` hook runs once.
- An error made with `Error::permanent(e)` skips the retries: retrying can't fix a bad card
  number.
- `queue:failed` lists them; `queue:retry ID|all` puts them back; `queue:forget ID`,
  `queue:flush` and `queue:prune-failed --hours 168` delete them.
- A worker that dies mid-job (a crash, `SIGKILL`) leaves the job reserved; another worker takes
  it again after 15 minutes (or `TIMEOUT` + 1 minute for longer jobs).

```rust
use renox::prelude::*;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Serialize, Deserialize)]
struct Charge {
    order_id: i64,
    card: String,
}

impl Job for Charge {
    const NAME: &'static str = "charge";

    fn backoff(attempt: u32) -> Duration {
        Duration::from_secs(60 * u64::from(attempt)) // 1, 2, 3 minutes
    }

    async fn handle(self, _ctx: JobContext) -> Result {
        let _card: u64 = self.card.parse().map_err(Error::permanent)?; // no retry for bad input
        Ok(())
    }

    async fn failed(self, state: AppState, error: String) {
        // Once, after the last attempt: tell someone.
        eprintln!("charge {} failed for good: {error}", self.order_id);
        let _ = state; // e.g. notify the customer through state.mailer
    }
}
```

## Queuing only if the data is saved

A job queued before the transaction that writes its data commits may run and find nothing
(or run for an order that was rolled back). `dispatch_in` writes the job in the same
transaction, so it exists only if the transaction commits:

```rust
use renox::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct SendInvoice { order_id: i64 }

impl Job for SendInvoice {
    const NAME: &'static str = "send-invoice";
    async fn handle(self, _ctx: JobContext) -> Result { Ok(()) }
}

async fn checkout(State(state): State<AppState>) -> Result {
    let mut tx = state.db.begin().await?;
    let order_id: i64 = renox::db::sql("INSERT INTO orders (total) VALUES (?) RETURNING id")
        .bind(75_000)
        .scalar(&mut tx)
        .await?;
    state.queue.dispatch_in(&mut tx, SendInvoice { order_id }).await?;
    tx.commit().await?; // no commit, no job
    Ok(())
}
```

## Queues and priority

A job's `QUEUE` (default `"default"`) or `dispatch_on(queue, job)` picks its queue. Workers
started with `queue:work --queue high,default` always take `high` first, then `default`:

```rust
use renox::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct ResetPasswordMail { user_id: i64 }

impl Job for ResetPasswordMail {
    const NAME: &'static str = "reset-password-mail";
    const QUEUE: &'static str = "high"; // someone is waiting for it
    async fn handle(self, _ctx: JobContext) -> Result { Ok(()) }
}

async fn resend(State(state): State<AppState>) -> Result {
    state.queue.dispatch_on("default", ResetPasswordMail { user_id: 1 }).await?; // not urgent this time
    Ok(())
}
```

## Unique jobs, rate limits and overlapping

- **Unique:** with `UNIQUE_FOR`, a second dispatch while one with the same `unique_id` is queued
  or running returns the queued job's id instead of adding another (a "remind me" button
  pressed twice).
- **Middleware**, checked before each attempt; a job that can't run yet is put back without
  using up an attempt:
  - `Middleware::without_overlapping(key)`: one job with that key at a time;
  - `Middleware::rate_limited(key, max, per)`: at most `max` attempts per window, e.g. an
    API's quota.
- Both middlewares use the cache: with `CACHE_STORE=memory` they hold within one process, so use
  `database` when several servers run workers. Unique claims always use the database.

```rust
use renox::prelude::*;
use renox::queue::Middleware;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Serialize, Deserialize)]
struct SyncStock { shop_id: i64 }

impl Job for SyncStock {
    const NAME: &'static str = "sync-stock";
    const UNIQUE_FOR: Option<Duration> = Some(Duration::from_secs(600));

    fn unique_id(&self) -> String {
        self.shop_id.to_string() // one queued sync per shop
    }

    fn middleware(&self) -> Vec<Middleware> {
        vec![
            Middleware::without_overlapping(format!("shop:{}", self.shop_id))
                .release_after(Duration::from_secs(10)),
            Middleware::rate_limited("supplier-api", 60, Duration::from_secs(60)),
        ]
    }

    async fn handle(self, _ctx: JobContext) -> Result { Ok(()) }
}
```

## Encrypted payloads

`const ENCRYPTED: bool = true` stores the payload sealed with `APP_KEY` (in `jobs` and
`failed_jobs` alike), for jobs that carry personal data or secrets. A payload sealed with
another key fails for good, so keep `APP_KEY` stable (see operations.md, "Keys").

## Chains and batches

A **chain** runs jobs one after another; if one fails for good, the rest wait in
`failed_jobs` and `queue:retry` resumes the chain. A **batch** runs jobs side by side and tracks
them: `then` runs if all succeeded, `catch` on the first failure (which cancels the rest unless
`allow_failures()`), `finally` either way. Callbacks are jobs, because closures can't be
stored.

```rust
use renox::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct Step { name: String }

impl Job for Step {
    const NAME: &'static str = "step";
    async fn handle(self, ctx: JobContext) -> Result {
        eprintln!("{} (batch {:?})", self.name, ctx.batch_id);
        Ok(())
    }
}

fn step(name: &str) -> Step {
    Step { name: name.to_owned() }
}

async fn fulfil(State(state): State<AppState>) -> Result {
    state.queue.chain().then(step("charge")).then(step("receipt")).then(step("ship")).dispatch().await?;
    Ok(())
}

async fn statements(State(state): State<AppState>) -> Result<String> {
    let mut batch = state.queue.batch("monthly-statements");
    for customer in 1..=100 {
        batch = batch.push(step(&format!("statement {customer}")));
    }
    let id = batch
        .then(step("all sent"))
        .catch(step("tell the admin"))
        .allow_failures()
        .dispatch()
        .await?;
    Ok(format!("/batches/{id}"))
}

// Poll it from the page (`hx-get` with `hx-trigger="every 2s"`) for a progress bar.
async fn progress(State(state): State<AppState>, Path(id): Path<i64>) -> Result<String> {
    let status = state.queue.batch_status(id).await?.ok_or(Error::NotFound)?;
    Ok(format!("{}% ({} failed)", status.progress(), status.failed))
}
```

`queue.cancel_batch(id)` skips the jobs not started yet; `queue:prune-batches --hours 24`
deletes finished batches.

## Testing jobs

`TestApp` doesn't start workers. `app.queued_jobs()` lists what was queued (names, in order),
`app.run_jobs().await` runs every job that is due, until none is left (the next job of a
chain and a batch's callbacks too; a retry still waiting for its backoff is not due), and `state.dispatch_sync(job)` runs one job now, in the caller. Assert what the
job did: rows written, `app.sent_mail()`.

## The dashboard

`.module(renox::queue::Dashboard)` adds `/_renox/queue`: jobs ready, delayed and running per
queue, how long the oldest has waited, jobs done and failed in the last hour, the failed jobs
(retry one, retry all, forget), and recent batches with their progress. It refreshes itself
every 5 seconds. Only users who pass the `view-queue-dashboard` gate see it, in development
too, so define the gate:

```rust
# use renox::prelude::*;
# let _ =
App::new()
    .module(renox::queue::Dashboard)
    .gate(renox::queue::DASHBOARD_GATE, |user| user.email == "ops@example.com")
# ;
```

`state.queue.stats()` returns the same numbers (`QueueStats`) for your own monitoring, e.g. an
alert when `oldest_wait` grows. The throughput counts are per-minute rows in the `cache` table
(`renox:queue:done:*`, `renox:queue:failed:*`) that expire after two hours.

## Coming from Laravel

| Laravel | Renox |
|---|---|
| `dispatch($job)`, `->delay()`, `->onQueue()` | `state.dispatch(job)`, `dispatch_after`, `dispatch_on` / `const QUEUE` |
| `$tries`, `$timeout`, `backoff()` | `MAX_ATTEMPTS`, `TIMEOUT`, `fn backoff` |
| `failed()` | `async fn failed(self, state, error)` |
| `ShouldBeUnique`, `uniqueFor`, `uniqueId` | `UNIQUE_FOR`, `fn unique_id` |
| `ShouldBeEncrypted` | `const ENCRYPTED: bool = true` |
| `WithoutOverlapping`, `RateLimited` middleware | `Middleware::without_overlapping`, `Middleware::rate_limited` |
| `Bus::chain`, `Bus::batch`, `then/catch/finally` | `queue.chain()`, `queue.batch(name)`, jobs as callbacks |
| `afterCommit` | `dispatch_in(&mut tx, job)` |
| `dispatchSync` | `state.dispatch_sync(job)` |
| Horizon | `.module(renox::queue::Dashboard)` (see "The dashboard") |
