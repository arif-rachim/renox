# Running a Renox app in production

This guide covers what happens once the app is deployed: its settings for slow or failing
dependencies, the proxy in front of it, health checks, backups, and how to recover jobs and
webhooks that failed. For setting up the server itself, see the `deploy/README.md` that
`rnx make:deploy` writes.

## Timeouts

When something the app depends on stops answering, requests fail quickly instead of piling up.
All timeouts are in seconds and are set in `.env`:

| Setting | Default | What it limits |
|---|---|---|
| `DATABASE_ACQUIRE_TIMEOUT` | 5 | How long a query waits for a database connection. Past it, the request answers 500. |
| `DATABASE_STATEMENT_TIMEOUT` | 30 | How long one PostgreSQL statement may run (`0` = no limit). For a long report, run `SET LOCAL statement_timeout = 0` inside its transaction. |
| `REQUEST_TIMEOUT` | 60 | How long a handler may take to answer (`0` = no limit). Past it, the request answers 500. Streaming a response and waiting for a slow client don't count. |
| `MAIL_TIMEOUT` | 10 | How long sending one mail over SMTP may take, from connecting to the last reply. |
| (fixed) | 2 | How long `/health` waits for the database. |
| (fixed) | 5 | How long SQLite waits for another writer to finish (`busy_timeout`). |
| (fixed) | 30 | How long `serve` waits for running jobs after `SIGTERM`, before stopping. |

A job's own limit is its `Job::TIMEOUT` (60 s by default). A job that goes over it is stopped
and retried.

## Behind a reverse proxy

Run the app on `127.0.0.1` (`APP_HOST=127.0.0.1`) behind Caddy or nginx, which handle TLS:

```text
# Caddyfile
shop.example.com {
    reverse_proxy 127.0.0.1:3000
}
```

Behind a proxy, every connection comes from the proxy's address. Set `TRUSTED_PROXIES` so the
app believes the visitor's address that the proxy sends in `X-Forwarded-For`:

```text
TRUSTED_PROXIES=127.0.0.1          # Caddy or nginx on the same machine
TRUSTED_PROXIES=10.0.0.0/8         # a load balancer on a private network
TRUSTED_PROXIES=*                  # whoever connects (a platform whose proxy IPs you can't list)
```

Without this setting:
- `Routes::throttle` counts every visitor as one.
- The login lock counts per email only.
- Logs show the proxy's address.

Don't set it when the app is reachable directly: anyone could then send a fake
`X-Forwarded-For`. `ClientIp` in a handler gives the same address the app uses.

Also set `APP_URL` to the public `https://` address. Cookies are then marked `Secure`, HSTS is
sent, and links in mails point to the right place.

## Several servers

Several app servers can share one PostgreSQL database:
- The queue hands each job to one worker (`FOR UPDATE SKIP LOCKED`).
- Each scheduled run is claimed once.
- Migrations take turns.

Set `CACHE_STORE=database` as well. Then the cache, `Routes::throttle` and `throttle_by` limits, the login lock
and cache locks (`state.cache.lock`) live in the `cache` table, so they hold across servers. With
the default `memory` store, each server counts on its own (N servers allow N times the limit),
and a lock only keeps out other tasks of the same process.

Sessions live in their encrypted cookie, so any server can answer any request (no sticky
sessions). Uploaded files must be on shared storage (`STORAGE_DISK=s3`) or on the one server that
has the disk.

## `/health`

`GET /health` is meant for load balancers, uptime monitors and container health checks.

- **200** with `"status": "ok"` when the database answered within 2 seconds.
- **503** with `"status": "error"` and the reason in `"database"` otherwise.

The JSON also contains:
- `queue.pending` and `queue.failed`: job counts. Alert when `failed` grows.
- `maintenance`: whether the app is in maintenance mode.

Maintenance mode doesn't change the status code, so a load balancer keeps the app in rotation
and visitors see the maintenance page.

## When a dependency fails

The table below is what the app does when something it depends on fails. The database and
panic rows are what the chaos test (`tests/chaos/run.sh`, run in CI on SQLite and PostgreSQL)
checks on every change; the SMTP and killed-process rows are covered by the integration tests
(`crates/renox/tests/it/background_resilience.rs`); the `SIGTERM` row is how `serve` shuts down
and has no automated test yet.

| Fault | What the app does |
|---|---|
| PostgreSQL stops | Requests get 500 within 5 s and `/health` 503 within 2 s. After PostgreSQL is back, the app and its workers recover by themselves, with no restart. |
| PostgreSQL hangs (paused, network partition) | New requests get 500 within 5 s. A request already waiting on it gets 500 at `REQUEST_TIMEOUT`. |
| PostgreSQL restarts while a job runs | The job's result is written once the database is back, or the job is retried. |
| SQLite locked by another process (a backup, `sqlite3` shell) | Reads and `/health` keep working (WAL mode). Writes get 500 after 5 s. Workers retry writing a job's result, so nothing is stranded. |
| A handler panics | That request gets a 500 error page, and the app keeps serving. |
| A job panics | The attempt counts as failed and is retried, then the job goes to `failed_jobs`. The worker keeps running. |
| A scheduled task or event listener panics | It's logged. The task runs again on schedule, and the other listeners still run. |
| The SMTP server is down or silent | A direct `mailer.send` fails within `MAIL_TIMEOUT`. Queued mail (`queue_mail`) is retried five times, then goes to `failed_jobs`. |
| The process is killed (`SIGKILL`, out of memory) | Jobs it was running are retried after 15 minutes, or at `TIMEOUT` + 1 minute for longer jobs. If that was their last attempt, they go to `failed_jobs`. |
| The process gets `SIGTERM` (deploy, restart) | It stops taking requests and waits up to 30 s for running jobs. |

## Failed jobs

`/_renox/queue` (the `renox::queue::Dashboard` module, behind the `view-queue-dashboard` gate)
shows the queue and the failed jobs with retry and forget buttons; the commands below do the
same from a shell. `state.queue.stats()` has the numbers for monitoring (ready jobs, the oldest
wait, jobs done and failed in the last hour).

A job that fails every attempt (`Job::MAX_ATTEMPTS`, 3 by default) moves to `failed_jobs` with
its error. An error made with `Error::permanent` goes there after the first attempt, because
retrying can't fix it. So does a payload that no longer decodes.

```bash
my-app queue:failed          # list them, with their errors
my-app queue:retry 12        # put one back on the queue with fresh attempts
my-app queue:retry all
my-app queue:flush           # delete them all
my-app queue:forget 12       # delete one
my-app queue:prune-failed --hours 168    # delete those older than a week (the default)
my-app queue:prune-batches --hours 24    # delete finished batches
```

Fix the cause first, then retry. `queue:retry` runs the same payload again; a job of a chain
resumes the chain, and a job of a batch counts in its batch again. A job's `failed` hook runs
once it fails for good (not on `queue:retry`). Schedule the prune commands, e.g.
`s.daily_at("03:00", "prune-failed", |state| async move { state.queue.prune_failed(week).await.map(drop) })`.

## Failed webhook calls

Every accepted webhook call is stored in `webhook_calls` before it is processed. The provider
has its 200, so it won't send the call again; recovery is up to you.

```bash
my-app webhook:failed        # calls whose handler failed, with the error
my-app webhook:retry 7       # process one again, e.g. after a fix
```

A call is `received` until its handler succeeds (`processed`) or fails (`failed`). A handler
that panics also marks its call `failed`. The stored payload is the exact body the provider
sent, so signatures can be checked again.

## Backups

**SQLite.** The database is `storage/app.db`, plus its `-wal` and `-shm` files while the app runs.
- Continuous backup: use Litestream (see `deploy/README.md`).
- One-off copy while the app runs: `sqlite3 storage/app.db ".backup backup.db"`. Don't copy the
  file with `cp` while the app runs.

**PostgreSQL.** Use your provider's backups or `pg_dump`.

**Files.** Uploads live in `STORAGE_PATH/app` (or the S3 bucket). Back that directory up too.

**Keys.** Keep `.env`'s `APP_KEY` with the backups. Without it, sessions end and signed links
stop working. Values the app sealed with `state.encrypt`, and queued jobs with encrypted
payloads (`const ENCRYPTED`), can't be read any more: that data is lost unless you still have
the old key. Rotating the key has the same effect, so decrypt and
re-encrypt such values with the new key before switching.

## Deploys and migrations

- `migrate` runs pending migrations as one batch. Several servers can run it at the same deploy:
  they take turns, and the later ones find nothing left to do.
- Each migration runs in a transaction, so a failed one leaves nothing behind. Migrations with
  `CREATE INDEX CONCURRENTLY` or a `-- renox:no-transaction` line run without one; keep those to
  a single change.
- `migrate:status` shows each migration's batch, and flags applied migrations whose file was
  edited or deleted since they ran. To change the schema, add a new migration; never edit an
  applied one.
- Restart the app after `migrate` (the Dockerfile and `deploy/README.md` do: migrate, then
  start). Migrations run in-process (tests, `migrate:fresh`) are safe: connections opened before
  them are dropped. But a server that keeps running across a migration made by another process
  may keep connections that read the old schema until it restarts.
- `migrate:rollback` undoes the last batch only if every migration in it has a `.down.sql`;
  otherwise it undoes nothing.

## Scheduled tasks and housekeeping

The scheduler runs inside `serve` (or alone with `schedule:work`). Its times follow
`APP_TIMEZONE`: `UTC`, a fixed offset like `+07:00`, or an IANA name like `Asia/Jakarta`, with
daylight saving handled; a task can set its own `timezone`.

```bash
my-app schedule:list          # each task's next run, in its time zone
my-app schedule:run backup    # run one task now, e.g. to check it after a deploy
```

Some tables grow until something prunes them. Schedule the commands (e.g. daily) or run them by
hand:

| Table | Grows with | Pruned by |
|---|---|---|
| `cache` (database store) | expired entries | itself, at most once an hour per process; `cache:prune` on demand |
| `personal_access_tokens` | expired API tokens | `tokens:prune` (Auth module): tokens expired more than a day ago |
| `audit_logs` | every audited action (Audit module) | `audit:prune --days 365` |
| `revoked_sessions` | logouts | itself, on each logout |
| `failed_jobs` | jobs that failed for good | `queue:prune-failed --hours 168`, `queue:flush` (see Failed jobs) |
| `job_batches` | every dispatched batch | `queue:prune-batches --hours 24` (finished batches) |
| `webhook_calls` | every received webhook | nothing yet: delete old `processed` rows yourself if it matters |

## Maintenance mode

```bash
my-app down --secret let-me-in --retry 60   # 503 page with Retry-After; /let-me-in lets you through
my-app up
```

The flag file lives in `STORAGE_PATH`, so every process that shares that directory sees it.
`/health` and webhook routes that allow it keep working.

## Logs

- Logs go to stdout. Their level is set with `RUST_LOG` (e.g. `RUST_LOG=info,sqlx=warn`).
- `LOG_FORMAT=json` writes one JSON object per line, for Loki, Datadog, CloudWatch or
  `jq`. The request's fields are in `span`:

  ```json
  {"timestamp":"…","level":"ERROR","fields":{"message":"request failed","error":"…"},
   "target":"renox_core::error","span":{"id":"k3J9x2mQpL0aB7cDe4Fg","ip":"203.0.113.9","method":"POST","uri":"/checkout","name":"request"}}
  ```

- `LOG_FILE=storage/logs/app.log` appends to that file instead (without colors). Rotate it
  with logrotate's `copytruncate`, or leave logs on stdout for journald or Docker.
- Each request has an id: the `X-Request-Id` a proxy sent (kept when it's 8–64 characters of
  `A-Z a-z 0-9 . _ -`), or a new one. It's in every log line of the request, in the response's
  `X-Request-Id`, in error reports, and in handlers as the `RequestId` extractor. A visitor
  who quotes it points you at the exact log lines.
- Errors are logged with their cause. With `APP_DEBUG=false`, visitors see only the error page.
- Job attempts log `job done`, `job failed, will retry` and `job failed for good`.

## Error reports

`App::report` gets every error a person should look at, in the background after the response
is sent:

- a request that answered 500 (with its method, path, request id, IP and user id);
- a job that failed for good (`source` is its name and id, `send-invoice #42`);
- a scheduled task that failed or panicked.

```rust
# use renox::prelude::*;
use renox::report::ErrorReport;

# let _ =
App::new().report(|report: ErrorReport, state: AppState| async move {
    // Sentry, Honeybadger, a Slack webhook… The report serializes to JSON.
    if let Some(url) = state.config.var("ERROR_WEBHOOK") {
        let _ = state.http.post(url).json(&report).send().await;
    }
})
# ;
```

Several reporters can be added; one that fails or panics doesn't stop the others. 4xx errors
(a 404, a failed validation) aren't reported.

## Error pages

`rnx new` writes `resources/views/errors/default.html`, which extends the app's layout, so an
error page keeps the navigation bar and the signed-in user's menu. It gets the same globals as
any page (`auth`, `request`, `t()`, `route()`…) plus `status`, `reason` and `detail` (the
message of an `abort(…)`; a 500's cause only with `APP_DEBUG`). `errors/404.html`, or any
status, wins for that status. If the app's page itself fails to render, Renox shows its own,
so an error in the layout can't hide the original error. JSON clients get JSON.

## Debug inspector

With `APP_DEBUG=true` and `APP_ENV=local`, `/_renox/debug` lists the last 50 requests (newest
first) with their status, time, the view they rendered and their SQL. A statement run three or
more times in one request is flagged as a likely N+1. The page is never mounted in
production or tests. Mail sent while developing is at `/_renox/mail`.

