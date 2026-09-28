# examples/jobs

Background work: an event, a listener, a queued job, a notification, scheduled tasks and a cache
lock.
Placing an order emits `OrderPlaced`; its listener queues the customer's receipt and notifies
every user (the admins) by mail and in the database. Read it before adding anything slow to a
request.

```bash
cd examples/jobs
cargo run -- migrate
cargo run -- db:seed             # admin@example.com / password123
cargo run                        # http://127.0.0.1:3000
```

Queue workers and the scheduler run inside `cargo run`. Mails go to the log (`MAIL_MAILER=log`
is the default), and while `APP_DEBUG` is on they are listed at `/_renox/mail`.
`cargo run -- schedule:list` shows the scheduled tasks and when they run next;
`cargo run -- schedule:run daily-sales` runs one now.

## What's where

| Feature | Where |
|---|---|
| Wiring: the `Auth` module, the orders module, the seeder | [src/lib.rs](src/lib.rs) |
| The model, the `OrderPlaced` event and its listener, the order form, the daily and weekly sales reports | [src/app/orders/mod.rs](src/app/orders/mod.rs) |
| `SendReceipt`: a queued job with retries | [src/app/orders/receipt.rs](src/app/orders/receipt.rs) |
| `NewOrder`: a notification sent by mail and stored in the database | [src/app/orders/new_order.rs](src/app/orders/new_order.rs) |
| Mail templates: the receipt (HTML and text) and the sales report | [resources/views/mail](resources/views/mail) |

## Things worth copying

- **The handler only emits.** `store` saves the order and calls `state.emit(OrderPlaced { .. })`;
  what follows lives in the listener registered in `Module::register`.
- **Jobs keep ids, not rows.** `SendReceipt` holds `order_id` and loads the order when it runs,
  because jobs are stored as JSON.
- **Retries for flaky mail servers.** `SendReceipt` sets `MAX_ATTEMPTS = 5`.
- **One notification, two channels.** `NewOrder` returns `Channel::Mail` and
  `Channel::Database`; the database row shows up in `user.unread_notifications(&db)`.
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
- **Admins are every user** here, since customers don't log in. With customer accounts, pick
  staff by role with the `Permissions` module.

## Tests

```bash
cargo test -p jobs
```

[tests/orders.rs](tests/orders.rs) checks the queued job with `app.queued_jobs()`, runs it with
`app.run_jobs()`, and reads the sent mail. It also checks the reports' counts and sums (old
orders left out), that a held lock skips a run, and that a failing report sends the alert.
