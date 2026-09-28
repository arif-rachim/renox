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
`cargo run -- shop:make-admin you@example.com`.

## What's where

| Feature | Where |
|---|---|
| Wiring: modules, the `admin` gate, the `rupiah` filter, `cart_count` on every page, the `shop:make-admin` command, the seeder | [src/lib.rs](src/lib.rs) |
| Home page with a cached product list; list with search, category filter, sort and pagination (the search box swaps only the results with htmx); product page with SEO tags; `sitemap.xml`; language switch | [src/app/catalog/mod.rs](src/app/catalog/mod.rs) |
| Models, a factory, slugs, a "belongs to" method | [src/app/catalog/model.rs](src/app/catalog/model.rs) |
| The cart in the database, loaded with its products in one query (`relations::belongs_to`), an upsert in plain SQL | [src/app/cart/mod.rs](src/app/cart/mod.rs) |
| Checkout in one transaction that never oversells, and cancelling with the stock given back | [src/app/orders/checkout.rs](src/app/orders/checkout.rs) |
| `OrderPlaced` event, its listener, the daily task that cancels unpaid orders, the order policy | [src/app/orders/mod.rs](src/app/orders/mod.rs), [model.rs](src/app/orders/model.rs) |
| Notifications: a queued confirmation mail (HTML and text) and a database row for the customer, a database row for every admin, a "shipped" mail | [src/app/orders/notifications.rs](src/app/orders/notifications.rs), [resources/views/mail](resources/views/mail) |
| `/admin`: an `Admin` extractor that checks the gate, products with photo uploads and search/sort, orders moved pending → paid → shipped, a dashboard with low stock and notifications | [src/app/admin](src/app/admin) |
| English and Indonesian, with plurals (`0 products`, `One product`, `3 products`) and translated validation labels | [resources/lang](resources/lang) |
| Deploy: Dockerfile (cargo-chef), systemd unit, Litestream, from `rnx make:deploy` | [Dockerfile](Dockerfile), [deploy/](deploy) |

## Things worth copying

- **Stock is taken in the database, not in Rust.** `UPDATE products SET stock = stock - ?
  WHERE id = ? AND stock >= ?` checks and takes in one statement, so two customers can't buy the
  last item; a `CHECK (stock >= 0)` backs it up. If any line is short, the transaction is dropped
  and nothing changes ([checkout.rs](src/app/orders/checkout.rs)).
- **Orders keep what was bought.** `order_items` stores the name and price at the time of the
  order, so editing or deleting a product never changes past orders.
- **Slow work goes to the queue.** Checkout only emits `OrderPlaced`; the listener queues the
  mail with `notify_later`, so a slow mail server never slows down checkout.
- **Column names never come from the visitor.** Sorting maps `?sort=price_asc` to a known
  `order_by` call.
- **One query per relation.** The product list and the cart load their categories and products
  with `relations::belongs_to`, not one query per row.
- **Admin routes are checked in one place.** Every admin handler takes `Admin`, an extractor
  that sends guests to the login page and answers 403 to customers.

## Deploying

`Dockerfile` and `deploy/` are exactly what `rnx make:deploy` writes. In an app made with
`rnx new`, `docker build .` works as is (CI checks that on every change). This example is part
of the Renox workspace, so build it from a copy made with `rnx new` rather than from this folder.

Money is in rupiah as `i64`, and the payment is a bank transfer the admin confirms by hand. To
take card or e-wallet payments, see [examples/webhooks](../webhooks).
