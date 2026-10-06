# examples/bikeshop

Renox's flagship example (epic #231): a bike shop with three stores where customers **buy**
bikes and gear, **rent** a bike by the hour or the day, and have their bikes **serviced**,
once or on a plan. It keeps the shape of Sakila/Pagila (stock per store, rented for a period,
returned, maybe late, paid) and grows into a whole business, story by story (#232–#243, #245).

**Every page explains itself.** An "About this page" button opens a panel with what the page
is for, who uses it, which Renox features it uses and why, what happens under the hood, and
links to the guide and to the source files. `/about/pages` lists every page, filtered by
feature ("which pages use `renox::grid`?") or by role.

```bash
cd examples/bikeshop
cp .env.example .env    # then: rnx key:generate
cargo run -- migrate
cargo run -- db:seed    # a small shop in a second (or: demo:seed --size large)
cargo run               # http://127.0.0.1:3000
```

The seed prints the demo users (password `password`): `owner@bikeshop.test`,
`manager.north@bikeshop.test`, `cashier.south@bikeshop.test`, `mechanic.west@bikeshop.test`
(helping South this week), `floater@bikeshop.test` (roles in two stores),
`customer@bikeshop.test`, and the rest of each store's staff.

## What's here so far

The skeleton the stories build on (#232, part 1):

| What | Where |
|---|---|
| The app (`rnx new bikeshop`, lib + bin), its modules in one alphabetical list | [src/lib.rs](src/lib.rs) |
| One folder per area: `about`, `access`, `accounts`, `api`, `catalog`, `home`, `multistore`, `plans`, `rentals`, `reports`, `sales`, `staff`, `stock`, `workshop` (most are empty for now) | [src/app/](src/app/) |
| The "About this page" mechanism: `Explanation`, the registry, the panel's template function, `BIKESHOP_EXPLAIN` | [src/explain.rs](src/explain.rs), each area's `explain.rs` |
| The panel (the kit's `open_button` + `sheet(slide_over=true)` + `infolist`) | [resources/views/about/_panel.html](resources/views/about/_panel.html) |
| `/about/pages`: every page, filtered by feature and by role | [src/app/about/mod.rs](src/app/about/mod.rs), [resources/views/about/pages.html](resources/views/about/pages.html) |
| `/about/data`: the data model explained (tables, relations, Pagila → bike shop, store attributes, money, `Encrypted`, `DbEnum`, soft deletes, the seeders, roles and permissions), with live row counts | [src/app/about/data.rs](src/app/about/data.rs), [resources/views/about/data.html](resources/views/about/data.html) |
| Public layout: the kit's `navbar` (links, search and cart slots, "About this page", language and account menus) | [resources/views/layouts/app.html](resources/views/layouts/app.html) |
| Staff layout: the kit's `sidebar` in an `rx-shell` (a slot for the store switcher) | [resources/views/layouts/staff.html](resources/views/layouts/staff.html) |
| Renox's sign-in pages in the shop's look, with the panel | [resources/views/renox/auth/layout.html](resources/views/renox/auth/layout.html) |
| Brand colour and the page grids (only the kit's type and spacing tokens) | [public/app.css](public/app.css) |
| Motion ([motion.dev](https://motion.dev), MIT), vendored, no CDN; `prefers-reduced-motion` respected | [public/vendor/motion/](public/vendor/motion/) (see its `NOTICE`), [public/app.js](public/app.js) |
| English and Spanish | [resources/lang/](resources/lang/) |
| The walker test (every GET route has an explanation whose guide anchors and source files exist), the panel, the index, the language menu | [tests/about.rs](tests/about.rs) |

## The data model

Pagila's shape (stores, staff, customers and their places, inventory, rentals, payments)
grown into a bike shop that sells, rents and services bikes across three stores. Every
table has a model in the area that owns it (`src/app/<area>/model.rs`) and a `Factory`
with states (`src/app/<area>/factories.rs`). `/about/data` explains it all in the app,
with live row counts.

| What | Where |
|---|---|
| Ten migrations, SQLite (`*.up.sql`) and PostgreSQL (`*.postgres.up.sql`), plural table names, money as integers | [migrations/](migrations/) |
| Places and customers (the ID number an `Encrypted<String>`, soft deletes) | [src/app/accounts/model.rs](src/app/accounts/model.rs) |
| Stores (opening hours as JSON, the fee rate in basis points), staff (no role column), help between stores | [src/app/staff/model.rs](src/app/staff/model.rs) |
| Catalogue: categories, brands, products (searchable: `#[model(search = …)]`, the index in `migrations/20260101000410_search_products.*`), variants, photos, `part_fits` (a `Pivot` with a note) | [src/app/catalog/model.rs](src/app/catalog/model.rs) |
| Stock per variant × owner store × location store, the ledger (`Morph` to what caused it), suppliers, purchase orders, consignment shipments | [src/app/stock/model.rs](src/app/stock/model.rs) |
| The fleet (owner and location store), placements, rentals (owner, operating and return store; a `Ulid` reservation code) | [src/app/rentals/model.rs](src/app/rentals/model.rs) |
| Orders (operating store), lines (owner store of the goods), payments (`Morph` to an order, rental or work order) | [src/app/sales/model.rs](src/app/sales/model.rs) |
| Customers' bikes, service tasks, work orders (billed to the owner store for fleet repairs), `has_many_through` | [src/app/workshop/model.rs](src/app/workshop/model.rs) |
| Service plans, their tasks (`Pivot`), subscriptions | [src/app/plans/model.rs](src/app/plans/model.rs) |
| The books between stores (#245's rules in the module doc) and monthly settlements | [src/app/multistore/model.rs](src/app/multistore/model.rs) |
| `db:seed` (small) and `demo:seed --size large` (Pagila's volume: ~1,000 variants, 600 customers, ~15,000 rentals, 5,000 orders, 3,000 work orders over 18 months, about 4 s on SQLite), dates counted back from today, never twice | [src/seed/](src/seed/) |
| Building blocks for tests: a store, a person with roles per store, a dated role, a fleet bike | [src/seed/fixtures.rs](src/seed/fixtures.rs) |
| Tests: migrations up and down, the sealed ID number, relations in a fixed number of queries, search; the seeds, the ledger adding up, the books balancing | [tests/data.rs](tests/data.rs), [tests/seed.rs](tests/seed.rs) |

## Access: permissions for *what*, stores for *where*

Code checks **permissions**, never role names (`tests/access.rs` scans `src/` and
`resources/` and fails on `has_role`, `require_role`, `auth.roles` or a role's name).
Roles are given **in a store**, optionally between two dates (Renox's `Permissions`
module, #244); only the owner's role is global.

| What | Where |
|---|---|
| The permission catalogue (`rentals.checkout`, `stock.adjust`, `prices.change`…) and the roles: owner, manager, cashier, mechanic, staff | [src/app/access/catalogue.rs](src/app/access/catalogue.rs) |
| `access::staff_routes(routes)`: login, the active store (session → checked against today's roles → `permissions::set_scope`), `staff.access`; `POST /staff/store/{store}` switches | [src/app/access/active_store.rs](src/app/access/active_store.rs) |
| The store switcher in the staff shell (the kit's `menu`) | [resources/views/layouts/_store_switcher.html](resources/views/layouts/_store_switcher.html) |
| ABAC: `StoreAttr` (owner / location / operating), `StoreRecord`, `require` (404 when the record isn't visible, 403 when the action isn't allowed in the store that matters), `find`, `visible::<M>(permission)` ("mine or at my store") | [src/app/access/policy.rs](src/app/access/policy.rs) |

A staff page in a story looks like this:

```rust
use bikeshop::app::access::{self, StoreAttr, catalogue};
use bikeshop::app::rentals::model::RentalBike;
use renox::prelude::*;

fn routes() -> Routes {
    access::staff_routes(
        Routes::new()
            .post("/staff/fleet/{id}/rate", change_rate)
            .name("fleet.rate")
            .require_permission(catalogue::FLEET_VIEW),
    )
}

async fn change_rate(State(db): State<Db>, user: AuthUser, Path(id): Path<i64>) -> Result<&'static str> {
    let bike = access::find::<RentalBike>(&db, &user, id).await?; // 404 for another store's bike
    access::require(&user, catalogue::PRICES_CHANGE, StoreAttr::Owner, &bike)?; // the owner store decides
    Ok("changed")
}
```

## Adding a page

1. Add the route to the area's `routes()` in `src/app/<area>/mod.rs`, with a `.name(…)`.
2. Add its `Explanation` to `src/app/<area>/explain.rs`: the route's name and path, purpose,
   who uses it, the Renox features and why, what happens under the hood, `docs/*.md#anchor`
   links and the source files. Use the same `api` text as other pages for the same feature
   (`/about/pages` groups by it).
3. A GET route that isn't a page (JSON, a file, a stream) goes in `not_pages()` with the reason.
4. `cargo test -p bikeshop --test about` tells you what is missing.

Spanish texts for an explanation go in `resources/lang/es.json` under
`about_page.<route name>` (`title`, `purpose`, `who`, `under_hood`, `features.<n>`); whatever
is missing there is shown in English.

## Settings

`BIKESHOP_EXPLAIN=false` hides the panels (the index stays). The rest is Renox's, documented in
[.env.example](.env.example).
