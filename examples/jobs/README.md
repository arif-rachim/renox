# examples/jobs

Background work: an event, a listener, queued jobs (priority queues, a chain, a batch with
progress, a unique job, job middleware, a `failed` hook, an encrypted payload), a notification,
scheduled tasks and a cache lock.
Placing an order emits `OrderPlaced`; its listener notifies every user (the admins) by mail and
in the database. Paying charges the card, sends the receipt and tells the warehouse, one job
after another. Read it before adding anything slow to a request.

```bash
cd examples/jobs
cp .env.example .env    # optional: the settings this example reads
cargo run -- migrate
cargo run -- db:seed             # admin@example.com / password123
cargo run                        # http://127.0.0.1:3000
```

Queue workers and the scheduler run inside `cargo run`. To run workers on their own, highest
priority first: `cargo run -- queue:work --queue high,default` (a worker drains `high`, where
charges and receipts go, before `default`, where reports and statements go).
Registration is off (`Auth::new().without_registration()`): every user is an admin who gets
the order mails, so `db:seed` makes the only one. Log in as the admin (`/login`) for the staff buttons: "Remind customer" and "Email monthly
statements". On an unpaid order, pick a test card: one that works, one that is declined (the
order becomes `needs_attention` at once) and one whose gateway times out (three attempts, then
the same). `cargo run -- queue:failed` lists what failed for good. Mails go to the log (`MAIL_MAILER=log`
is the default), and while `APP_DEBUG` is on they are listed at `/_renox/mail`.
`cargo run -- schedule:list` shows the scheduled tasks and when they run next;
`cargo run -- schedule:run daily-sales` runs one now. Logged in as the admin, `/_renox/queue`
shows the queue: jobs waiting, throughput, failed jobs (retry or forget them) and batches.

## What's where

| Feature | Where |
|---|---|
| Wiring: the `Auth` module, the orders module, the seeder | [src/lib.rs](src/lib.rs) |
| The model, the `OrderPlaced` event and its listener, the order form, the daily and weekly sales reports | [src/app/orders/mod.rs](src/app/orders/mod.rs) |
| `SendReceipt`: a queued job with retries, on the `high` queue | [src/app/orders/receipt.rs](src/app/orders/receipt.rs) |
| Paying: the chain `ChargePayment` → `SendReceipt` → `NotifyWarehouse`; `ChargePayment`'s encrypted payload, middleware and `failed` hook; the gateway call over `state.http` and the sandbox gateway | [src/app/orders/payment.rs](src/app/orders/payment.rs) |
| `RemindUnpaid`: a unique job (one per order while queued) | [src/app/orders/remind.rs](src/app/orders/remind.rs) |
| Monthly statements: a batch with a `then` job, and its progress page | [src/app/orders/statements.rs](src/app/orders/statements.rs), [resources/views/orders/statements.html](resources/views/orders/statements.html) |
| The `status` column (`OrderStatus`, a `DbEnum`) | [migrations](migrations), [src/app/orders/mod.rs](src/app/orders/mod.rs) |
| `NewOrder`: a notification sent by mail and stored in the database | [src/app/orders/new_order.rs](src/app/orders/new_order.rs) |
| Mail templates: the receipt (HTML and text) and the sales report | [resources/views/mail](resources/views/mail) |
| The queue dashboard and the gate that lets the admin see it | [src/lib.rs](src/lib.rs) |
| `post_to_chat`: an `App::report` reporter that posts errors to a chat webhook | [src/lib.rs](src/lib.rs) |

## Things worth copying

- **The handler only emits.** `store` saves the order and calls `state.emit(OrderPlaced { .. })`;
  what follows lives in the listener registered in `Module::register`.
- **Jobs keep ids, not rows.** `SendReceipt` holds `order_id` and loads the order when it runs,
  because jobs are stored as JSON.
- **Retries for flaky mail servers.** `SendReceipt` sets `MAX_ATTEMPTS = 5`.
- **Priority queues.** `SendReceipt` and `ChargePayment` set `const QUEUE = "high"` (a customer
  is waiting); `queue:work --queue high,default` runs them before anything on `default`.
  `state.queue.dispatch_on("high", job)` picks the queue for a single dispatch instead.
- **A chain for steps that depend on each other.** `pay` dispatches
  `state.queue.chain().then(ChargePayment { .. }).then(SendReceipt { .. }).then(NotifyWarehouse { .. })`:
  each job is queued when the one before succeeds, so a declined card never gets a receipt.
  `pay` also moves the order from `unpaid` to `processing` with a conditional `update`, so a
  double click can't queue two charges.
- **Calling the gateway over HTTP.** `gateway::charge` posts to `PAYMENT_GATEWAY_URL/charges`
  with `state.http`: basic auth with the secret key (`PAYMENT_GATEWAY_KEY`), an
  `idempotency-key` of `order-{id}` so a retry can't charge twice, and a 15 s timeout. Unset,
  the URL is this app's own `/sandbox/gateway` route, which answers like a provider's test
  mode: `tok_declined` → 402, `tok_unreachable` → 503, any other token → 201. The tests don't
  use it: `app.fake_http()` answers instead, and one test checks the request itself.
- **Permanent vs. retryable errors.** A 402 (declined) becomes `Error::permanent(..)`
  (straight to `failed_jobs`); no answer, a 429 or a 5xx is a plain error (retried up to
  `MAX_ATTEMPTS`, waiting `Job::backoff` between attempts).
- **A `failed` hook.** `ChargePayment::failed` runs once, after the last attempt: it marks the
  order `needs_attention` and mails the admins.
- **Job middleware.** `ChargePayment::middleware` returns
  `Middleware::rate_limited("payment-gateway", 100, 60 s)` (the gateway's limit; jobs over it
  wait for the next window without using up an attempt) and
  `Middleware::without_overlapping("charge:{order_id}")` (never two charges for one order at
  once). Across several servers both need `CACHE_STORE=database`.
- **Copies, replies and attachments.** The monthly statement attaches the customer's orders
  as a CSV file (`Mail::attach(name, "text/csv", bytes)`) and sends the books a copy the
  customer doesn't see (`.bcc(ACCOUNTS_EMAIL)`); the receipt's replies go to support
  (`.reply_to(SUPPORT_EMAIL)`) rather than the no-reply sender; the warehouse's mail copies the
  manager (`.cc(MANAGER_EMAIL)`). Queued mail keeps its attachments (stored as base64).
- **Encrypt what's sensitive.** `ChargePayment` carries the card token, so it sets
  `const ENCRYPTED: bool = true`: the payload is sealed with `APP_KEY` in `jobs` and
  `failed_jobs`, and opened by the worker.
- **Unique jobs.** `RemindUnpaid` sets `UNIQUE_FOR` and `unique_id() = order_id`: pressing
  "Remind customer" twice queues one reminder. Once it has run, the next press queues again.
- **Batches with progress.** "Email monthly statements" pushes one `SendStatement` per customer
  into `state.queue.batch("monthly-statements")`, with `.allow_failures()` (one bad address
  doesn't cancel the rest) and `.then(StatementsSent { .. })` (queued once all went out; there
  are also `.catch` and `.finally`). The page reads `state.queue.batch_status(id)` and its
  `progress()`; htmx polls it with `hx-trigger="every 1s"` and gets only the `progress` block
  (`view(..).fragment("progress")`), which stops polling when the batch is finished.
- **One notification, two channels.** `NewOrder` returns `Channel::Mail` and
  `Channel::Database`; the database row, a `DatabaseMessage` (title, body and the order's
  keys), shows up in `user.unread_notifications(&db)` (and in the UI kit's
  `notification_bell` of an app with `Auth::new().notifications()`).
- **Scheduled tasks are plain functions.** `daily_sales` is registered with
  `daily_at("21:00", ...)` narrowed by `.weekdays()` and `.timezone("Asia/Jakarta")` (instead
  of `APP_TIMEZONE`, with DST handled for zones that have it); `weekly_sales` uses a cron
  expression, `cron("30 7 * * 1", ...)` (Mondays at 07:30). Tests call them directly or by
  name with `app.kernel().run_scheduled("weekly-sales")`, which is also what
  `schedule:run` does.
- **Count and sum in SQL.** The reports use `Order::query()...count(&db)` and
  `.sum::<i64, _>(&db, "total")` instead of loading every order.
- **A lock against double sends.** Scheduled runs are already claimed once per slot, even
  with several servers on one database. The report also takes
  `state.cache.lock("sales-report:1", ttl).try_acquire()` and skips when another run (say a
  `schedule:run` typed by hand) holds it. Across servers this needs `CACHE_STORE=database`.
- **Tell someone when it fails.** `.on_failure(report_failed)` queues an alert mail to
  `ALERT_EMAIL` (default `admin@example.com`); the failure is logged either way.
- **Errors that need a person reach the team.** `App::report(post_to_chat)` gets every 500,
  every job that failed for good (a declined card) and every failed scheduled task, and posts
  a line like `[production] charge-payment #1: the card was declined` to `ERROR_WEBHOOK_URL`
  (a Slack, Discord or Google Chat incoming webhook) with `state.http`. Reporters run in the
  background; errors are logged with or without them.
- **Admins are every user** here, since customers don't log in. With customer accounts, pick
  staff by role with the `Permissions` module.

## Tests

```bash
cargo test -p jobs
```

[tests/orders.rs](tests/orders.rs) checks queued jobs with `app.queued_jobs()`, runs them with
`app.run_jobs()` (or one at a time with `app.kernel().worker(..).run_next()`), and reads the sent
mail. It covers the chain's order, the encrypted payload (the token isn't in the `jobs` table), a
declined card, the `failed` hook after the third attempt, one reminder for two clicks, the batch
reaching 100 % with its `then` job, and a worker for `high,default` taking the receipt before an
earlier report. It also checks the reports' counts and sums, that a held lock skips a run, that a
failing report sends the alert, and that a failed job is posted to the chat (`fake_http`).

Time and side effects are faked, not waited for:
- `app.travel(Duration)` moves the clock for requests and `run_jobs`: past `Job::backoff`
  between attempts, past a unique job's `UNIQUE_FOR` hour, three days ahead for the daily
  report (code called directly runs inside `app.at_travelled_time(..)`).
- `app.fake_events()` records `OrderPlaced` without running the listener
  (`assert_emitted::<OrderPlaced>(|e| e.order_id == 1)`), so the handler is tested alone.
- `app.fake_notifications()` records `NewOrder` for each admin (`assert_notified(&user,
  "new-order")`) without mail or database rows.
