# examples/shop

A small online shop built the way a real Renox app is: customers browse, search and buy; admins
manage products and ship orders. It is tested end to end (`tests/shop.rs`) and is the example to
read when you want to see how the pieces fit together.

```bash
cd examples/shop
rnx key:generate                 # writes APP_KEY to .env
cargo run -- migrate
cargo run -- db:seed             # categories, fake products, admin@example.com / password123
cargo run                        # http://127.0.0.1:3000
```

Register a customer at `/register`, or promote any registered user with
`cargo run -- shop:make-admin you@example.com`. It gives them the `admin` role, creating the role
on a fresh install. It's a typed command (`impl AppCommand`, arguments declared with clap) and asks
for the email when it's left out.

## What's where

| Feature | Where |
|---|---|
| Wiring: modules (including `Permissions` and `Audit`), the `admin` role and who has it (`make_admin_of`, `admins`), the `rupiah` filter, `cart_count` on every page, the `shop:make-admin` command, the seeder | [src/lib.rs](src/lib.rs) |
| Home page with a cached product list; list with search, category filter, sort and pagination (the search box swaps only the results with htmx); product page with SEO tags; `sitemap.xml`; language switch | [src/app/catalog/mod.rs](src/app/catalog/mod.rs) |
| Models, a factory, slugs, a "belongs to" method | [src/app/catalog/model.rs](src/app/catalog/model.rs) |
| The cart in the database, loaded with its products in one query (`relations::belongs_to`), an upsert in plain SQL | [src/app/cart/mod.rs](src/app/cart/mod.rs) |
| Checkout in one transaction (`db.transaction_retrying`) that never oversells, and cancelling with the stock given back | [src/app/orders/checkout.rs](src/app/orders/checkout.rs) |
| `OrderPlaced` event, its listener, the daily task that cancels unpaid orders, the order policy (owners; admins pass by role) | [src/app/orders/mod.rs](src/app/orders/mod.rs), [model.rs](src/app/orders/model.rs) |
| Notifications: a queued confirmation mail (HTML and text) and a database row for the customer, a database row for every admin, a "shipped" mail | [src/app/orders/notifications.rs](src/app/orders/notifications.rs), [resources/views/mail](resources/views/mail) |
| `/admin`: a route group guarded by the `admin` role (`require_role`), products with photo uploads and search/sort (deleting one asks for the password again), orders moved pending → paid → shipped (each move written to the audit log), a dashboard with pending orders, low stock, notifications and recent activity | [src/app/admin](src/app/admin) |
| English and Indonesian, with plurals (`0 products`, `One product`, `3 products`) and translated validation labels | [resources/lang](resources/lang) |
| Deploy: Dockerfile (cargo-chef), systemd unit, Litestream, from `rnx make:deploy` | [Dockerfile](Dockerfile), [deploy/](deploy) |

## Things worth copying

- **Stock is taken in the database, not in Rust.** `UPDATE products SET stock = stock - ?
  WHERE id = ? AND stock >= ?` checks and takes in one statement, so two customers can't buy the
  last item; a `CHECK (stock >= 0)` backs it up ([checkout.rs](src/app/orders/checkout.rs)).
- **All or nothing, retried on conflicts.** Checkout runs in
  `db.transaction_retrying(3, |tx| Box::pin(async move { … }))`: committed when the closure
  returns `Ok`, rolled back on `Err`, and tried again when the database reports a conflict
  (SQLite busy, a PostgreSQL deadlock between two carts taking the same products). So a short
  line is an `Err` (a private `OutOfStock` error), not an early `Ok`, which would commit the
  stock already taken for the other lines; `place` turns it back into
  `Checkout::OutOfStock`. The closure's future can't borrow from the caller, so each attempt
  clones the cart lines it was given.
- **Orders keep what was bought.** `order_items` stores the name and price at the time of the
  order, so editing or deleting a product never changes past orders.
- **Slow work goes to the queue.** Checkout only emits `OrderPlaced`; the listener queues the
  mail with `notify_later`, so a slow mail server never slows down checkout.
- **Column names never come from the visitor.** Sorting maps `?sort=price_asc` to a known
  `order_by` call.
- **One query per relation.** The product list and the cart load their categories and products
  with `relations::belongs_to`, not one query per row.
- **Admins are a role, not a column.** The `Permissions` module keeps roles in its own tables;
  `shop:make-admin` and the seeder call `permissions::define_role` (idempotent) and
  `user.assign_role(db, "admin")`. The `/admin` group ends with `.require_role("admin")`, which
  sends guests to the login page and answers 403 to customers (`rnx route:list` shows
  `role:admin`); handlers ask `user.has_role("admin")` and templates `'admin' in auth.roles`.
  A `Policy` gets a plain `User` without roles, so `orders::show` checks the role before asking
  the order's policy.
- **Ask for the password before what can't be undone.** `DELETE /admin/products/{id}` is its
  own small route group ending in `.require_password_confirmed()`, merged into the admin group
  (a guard covers only the routes added before it). An admin who hasn't typed their password in
  the last three hours goes to the `Auth` module's `/confirm-password` first; the role is still
  checked before that, so customers get 403. Tests: `acting_as` doesn't count as typing the
  password, so post it to `/confirm-password` first (a DELETE isn't remembered for after the
  confirmation; only GET requests are).
- **Admin actions leave a trail.** With the `Audit` module, logins and account changes are
  recorded on their own; moving an order records `order.status_changed` with the admin, the
  order, `{from, to}` and the client IP (`audit::record`). `audit::for_subject(db, "orders", id,
  n)` reads an order's history; `cargo run -- audit:prune --days 365` trims old entries.
- **Save only what you changed.** A status change uses `order.save_only(db, &["status"])`, so it
  never writes back stale copies of the other columns.

## Deploying

`Dockerfile` and `deploy/` are exactly what `rnx make:deploy` writes. In an app made with
`rnx new`, `docker build .` works as is (CI checks that on every change). This example is part
of the Renox workspace, so build it from a copy made with `rnx new` rather than from this folder.

Money is in rupiah as `i64`, and the payment is a bank transfer the admin confirms by hand. To
take card or e-wallet payments, see [examples/webhooks](../webhooks).
