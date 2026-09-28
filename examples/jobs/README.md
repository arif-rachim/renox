# examples/jobs

Background work: an event, a listener, a queued job, a notification and a scheduled task.
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
`cargo run -- schedule:list` shows the scheduled task.

## What's where

| Feature | Where |
|---|---|
| Wiring: the `Auth` module, the orders module, the seeder | [src/lib.rs](src/lib.rs) |
| The model, the `OrderPlaced` event and its listener, the order form, the daily task at 21:00 | [src/app/orders/mod.rs](src/app/orders/mod.rs) |
| `SendReceipt`: a queued job with retries | [src/app/orders/receipt.rs](src/app/orders/receipt.rs) |
| `NewOrder`: a notification sent by mail and stored in the database | [src/app/orders/new_order.rs](src/app/orders/new_order.rs) |
| Mail templates: the receipt (HTML and text) and the daily sales mail | [resources/views/mail](resources/views/mail) |

## Things worth copying

- **The handler only emits.** `store` saves the order and calls `state.emit(OrderPlaced { .. })`;
  what follows lives in the listener registered in `Module::register`.
- **Jobs keep ids, not rows.** `SendReceipt` holds `order_id` and loads the order when it runs,
  because jobs are stored as JSON.
- **Retries for flaky mail servers.** `SendReceipt` sets `MAX_ATTEMPTS = 5`.
- **One notification, two channels.** `NewOrder` returns `Channel::Mail` and
  `Channel::Database`; the database row shows up in `user.unread_notifications(&db)`.
- **Scheduled tasks are plain functions.** `daily_sales` is registered with
  `app.schedule().daily_at("21:00", ...)` (in `APP_TIMEZONE`) and the tests call it directly.
  It sums the orders of the last 24 hours and queues one mail per user with `queue_mail`.

## Tests

```bash
cargo test -p jobs
```

[tests/orders.rs](tests/orders.rs) checks the queued job with `app.queued_jobs()`, runs it with
`app.run_jobs()`, and reads the sent mail.
