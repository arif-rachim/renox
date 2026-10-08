//! "About this page" entries for the about area (see `crate::explain`).

use crate::explain::{Audience, Explanation, Feature, Flow, NotAPage};

/// The explanation of every page in this area.
pub fn entries() -> Vec<Explanation> {
    vec![
        Explanation {
            route: "about.pages",
            path: "/about/pages",
            title: "Every page and its features",
            purpose: "The index of the whole example: every page, what it is for and the Renox \
                  features it uses. Start from a feature (\"which pages use \
                  `renox::grid`?\") or from a role (\"what does a cashier see?\") and \
                  follow the links.",
            who: "Developers learning Renox from the example, and anyone evaluating it.",
            audience: &[Audience::Developer],
            flow: Flow::Learn,
            features: &[
                Feature {
                    api: "Query<T>",
                    why: "The two filters are a plain GET form; `Query<Filters>` reads \
                      `?feature=…&audience=…` into a struct, so the filtered list has its \
                      own URL to share and works without JavaScript. A feature or a role \
                      the app doesn't know is ignored rather than refused.",
                },
                Feature {
                    api: "Lang",
                    why: "The extractor gives the request's language; each explanation is \
                      read from `resources/lang/<locale>.json` when it has the text, and \
                      falls back to English otherwise.",
                },
                Feature {
                    api: "Routes::get",
                    why: "Every page is a named route, and the explanations are keyed by \
                      that name. The list links to the pages without parameters with \
                      `state.url(name, &[])`; for one like `/products/{slug}` that call \
                      fails, so the row is shown without a link instead of a broken one.",
                },
                Feature {
                    api: "UI kit: toolbar + select",
                    why: "The filters line up in the kit's `toolbar`, which wraps on phones \
                      and needs no layout CSS. The feature list is long (every `api` used \
                      on any page), so it is a `select(…, searchable=true)`: type a few \
                      letters to find one.",
                },
                Feature {
                    api: "UI kit: list + badge",
                    why: "Each page is a row of the kit's `list`, its purpose through the \
                      `markdown` filter, its roles and features as `badge`s. Each feature \
                      badge links to this page filtered by it, so you can hop from a page \
                      to every other page that uses the same API.",
                },
                Feature {
                    api: "App::templates",
                    why: "The \"About this page\" panel on every page comes from the \
                      `about_page(request.route, request.path)` template function the app \
                      registers, so no handler passes anything for it: the layouts ask \
                      for it themselves. The panel is the kit's `sheet` (`slide_over`) \
                      with an `infolist` inside.",
                },
                Feature {
                    api: "App::share",
                    why: "`explain_panels` (from `BIKESHOP_EXPLAIN`) is given to every view, \
                      so the layouts can hide the panels for a clean demo without any \
                      handler knowing about it. This page stays either way.",
                },
            ],
            under_hood: "No database: the explanations are Rust values in each area's \
                     `explain.rs`, collected by `crate::explain::all()`. The handler \
                     filters them, localizes them and sorts them by title. \
                     `tests/about.rs` walks every GET route of the app and fails when a \
                     page has no explanation, or when a docs link or a source path here \
                     doesn't exist.",
            docs: &[
                "docs/routing.md#what-a-handler-can-take",
                "docs/routing.md#route-urls-and-the-current-route",
                "docs/ui.md#navigation-and-page-structure",
                "docs/laravel.md#blade--minijinja",
                "docs/laravel.md#cache-storage-sessions-cookies-and-translations",
                "docs/testing.md#a-first-test",
            ],
            sources: &[
                "examples/bikeshop/src/app/about/mod.rs",
                "examples/bikeshop/src/explain.rs",
                "examples/bikeshop/resources/views/about/pages.html",
                "examples/bikeshop/resources/views/about/_panel.html",
                "examples/bikeshop/tests/about.rs",
                "tests/browser/bikeshop-about.test.mjs",
            ],
        },
        Explanation {
            route: "about.blocks",
            path: "/about/blocks",
            title: "Bike shop blocks",
            purpose: "Every UI block the bike shop uses beyond Renox's kit, working, with its \
                  macro's signature: a photo gallery, a two-handle range, a quantity \
                  stepper, a counter keypad, a kanban board, a month calendar, an \
                  availability timeline, a date and time range, variant chips, a history \
                  timeline and plan cards. The other pages \
                  use them; this one shows them side by side.",
            who: "Developers who want one of the blocks on their own page, and anyone \
              checking how they look and behave (keyboard, phone, dark mode).",
            audience: &[Audience::Developer],
            flow: Flow::Learn,
            features: &[
                Feature {
                    api: "renox-blocks",
                    why: "The kit has no gallery, kanban or calendar. They were built in \
                      this example first, then moved to the `renox-blocks` crate, a \
                      plugin like renox-editors: the app adds `Blocks::new()` and a page \
                      imports a block like a kit component \
                      (`{% from \"renox-blocks/blocks.html\" import gallery %}`). The \
                      macros use `rx-` classes on the kit's `--rx-*` tokens and call the \
                      kit's own macros (`sheet`, `date_picker`); a page loads a block's \
                      script only when it has that block. The date picker with closed \
                      days became the kit's own `date_picker` (`disabled_dates`, \
                      `closed_weekdays`).",
                },
                Feature {
                    api: "UI kit: sheet",
                    why: "The gallery's enlarged photo opens in the kit's `sheet`: focus \
                      moves into it and back, Escape and the backdrop close it. The block \
                      reuses the kit's dialog rather than writing a second one.",
                },
                Feature {
                    api: "Valid<T>",
                    why: "Every form block sends plain fields (a number, the keypad's \
                      digits, one date-time per end, a radio's value), so \
                      `Valid<BlocksForm>` reads them like any form: `between(1, 5)`, \
                      `gt(\"starts_at\", …)` for the end, and `one_of` without the \
                      sold-out size. Its `after` hook refuses a rental starting in the \
                      past again on the server: anyone can send any date.",
                },
                Feature {
                    api: "Query<T>",
                    why: "The price filter is a GET form: the range's two handles become \
                      `?price_min=…&price_max=…`, the calendar's arrows `?month=…` and a \
                      free slot of the availability timeline `?slot=…`. Each state has \
                      its own URL, and a month that can't be shown falls back to today's.",
                },
                Feature {
                    api: "htmx fragments",
                    why: "A variant chip asks `GET /about/blocks/variant` for the price and \
                      stock (`hx-get` with `hx-include`) and swaps in \
                      `about/_variant.html`, not the whole page. The calendar's arrows are \
                      boosted links with `hx-select=\"#calendar\"`, so only the calendar \
                      changes. A kanban move is an htmx POST from a hidden form, which \
                      gets Renox's CSRF header like any htmx request.",
                },
                Feature {
                    api: "Web Animations",
                    why: "The gallery's slides and the kanban's cards glide with the \
                      browser's own Web Animations on `transform` (no library); under \
                      `prefers-reduced-motion` they jump instead.",
                },
            ],
            under_hood: "No database. `src/app/about/blocks.rs` builds the demo data (a \
                     month of visits on fixed days of whichever month is shown, today's \
                     bookings, a workshop board) and renders `about/blocks.html`. \
                     The crate's `blocks.js` module finds the blocks on the page and in \
                     whatever htmx swaps in (`htmx:load`) and imports each one's code \
                     (`gallery.js`, `kanban.js`…) from their `data-rx-*` attributes, with \
                     no inline handlers, so it runs under `CSP=strict`. \
                     Sending the form posts to `POST /about/blocks/form`: `Valid<T>` \
                     checks it, and a success redirects back to the form with a flash \
                     message (nothing is saved); a kanban move posts the card, the \
                     column and the position to `POST /about/blocks/kanban`, which \
                     answers `204` (or a `422` that puts the card back).",
            docs: &[
                "docs/ui.md#changing-the-kit-itself",
                "docs/ui.md#design-principles",
                "docs/ui.md#fragments-and-out-of-band-swaps",
                "docs/validation.md#hooks-prepare-authorize-after",
                "docs/validation.md#browser-values",
                "docs/testing.md#browser-tests",
            ],
            sources: &[
                "examples/bikeshop/src/app/about/blocks.rs",
                "examples/bikeshop/resources/views/about/blocks.html",
                "examples/bikeshop/resources/views/about/_variant.html",
                "crates/renox-blocks/views/blocks.html",
                "crates/renox-blocks/assets/blocks.js",
                "crates/renox-blocks/assets/parts/gallery.js",
                "crates/renox-blocks/assets/parts/kanban.js",
                "crates/renox-blocks/assets/blocks.css",
                "examples/bikeshop/tests/blocks.rs",
                "tests/browser/bikeshop-blocks.test.mjs",
                "tests/browser/blocks.test.mjs",
            ],
        },
        data_page(),
    ]
}

/// `/about/data`: the data model explained.
fn data_page() -> Explanation {
    Explanation {
        route: "about.data",
        path: "/about/data",
        title: "The data model",
        purpose: "Every table of the bike shop by the area that owns it, how they relate \
                  and how each relation is loaded, Pagila's tables and what they became, the \
                  owner / location / operating store attributes, how money, secrets, \
                  statuses and deletions are stored, the factories and seeders, and who may \
                  do what. The row counts are live: after `demo:seed --size large` the page \
                  shows Pagila's volume.",
        who: "Developers reading the example to see how a real business is modelled in \
              Renox, before opening the models.",
        audience: &[Audience::Developer, Audience::Owner],
        flow: Flow::Learn,
        features: &[
            Feature {
                api: "#[derive(Model)]",
                why: "One model per table, in the area that owns it: \
                      `src/app/<area>/model.rs`. The derive writes the column list, the \
                      reads and the writes, so no SQL is repeated by hand; the table names \
                      are written out (`#[model(table = \"rentals\")]`), plural as Renox \
                      recommends.",
            },
            Feature {
                api: "renox::db::relations",
                why: "`belongs_to`, `has_many`, `Pivot` (`part_fits` with a note), `Morph` \
                      (payments, stock movements and intercompany entries point at several \
                      tables) and `has_many_through` (a customer's work orders through \
                      their bikes): each loads a whole page's relations in a fixed number \
                      of queries instead of one per row, which `tests/data.rs` checks with \
                      `capture_queries`.",
            },
            Feature {
                api: "DbEnum",
                why: "Every status and kind is an enum stored as a word, readable in the \
                      table and checked when it is read back; the page lists the values \
                      from `ALL`, so it can't drift from the code.",
            },
            Feature {
                api: "Encrypted<T>",
                why: "The customer's ID number is sealed with `APP_KEY` in the table, so \
                      a copied database file doesn't give it away, and never serialized \
                      (`#[serde(skip_serializing)]`), so it can't leak into a view or JSON.",
            },
            Feature {
                api: "#[model(soft_deletes)]",
                why: "Customers who leave and discontinued products are hidden, not \
                      removed: their rentals, orders and payments still point at them.",
            },
            Feature {
                api: "renox::db::search",
                why: "Products are searchable by name, brand, SKU and description \
                      (`#[model(search = …)]`, the brand and SKUs copied into a \
                      `keywords` column): an FTS5 table on SQLite, a generated `tsvector` \
                      on PostgreSQL, from one migration Renox writes, instead of a slow \
                      `LIKE` over every row.",
            },
            Feature {
                api: "Factory",
                why: "The main models have factories with states named after the \
                      business (`rentals().overdue()`, `rental_bikes().placed_at(store)`), \
                      used by the seeders and the tests, so a test says what it needs in \
                      the shop's words instead of filling every column.",
            },
            Feature {
                api: "AppCommand",
                why: "`demo:seed --size large` is a typed clap command next to the \
                      built-in `db:seed`, so the option is parsed and listed in `--help`; \
                      it refuses a seeded database and says how to start again.",
            },
            Feature {
                api: "Permissions module",
                why: "The roles and what they grant are read from the database \
                      (`permissions::roles`), so the page shows what the owner set, with \
                      the labels and the permission catalogue from `access::catalogue`.",
            },
            Feature {
                api: "renox::db::sql",
                why: "The live row counts are one `UNION ALL` query over every table \
                      listed in `data.rs`, read into `(String, i64)` pairs with \
                      `fetch_as`: one round trip instead of one per table.",
            },
            Feature {
                api: "UI kit: card + table",
                why: "Each part of the page is a kit `card`; tables, relations and the \
                      mappings are kit `table`s (scrolling sideways on phones), the money, \
                      encryption, enum and soft-delete notes sit in a kit `columns(2)` \
                      grid. No layout CSS of the app's own.",
            },
            Feature {
                api: "UI kit: infolist + entry",
                why: "The numbers at the top, the enum values and the permissions are \
                      kit `infolist`s; `entry(…, format=\"money\")` shows how an \
                      integer amount is displayed.",
            },
            Feature {
                api: "Model::insert_many",
                why: "The seeders write each table with `insert_many` inside one transaction: a \
                      few multi-row `INSERT`s instead of one statement per row, which is what \
                      lets `demo:seed --size large` build 16,000 rentals and 5,000 orders in \
                      seconds.",
            },
        ],
        under_hood: "Two queries: one `SELECT 'table', COUNT(*) FROM table UNION ALL …` \
                     for the row counts, and `permissions::roles` for the roles and their \
                     permissions. Everything else is Rust data in \
                     `src/app/about/data.rs` (the tables, relations and mappings) or read \
                     from the code (`DbEnum::ALL`, `access::catalogue`). The texts are \
                     Markdown in `resources/lang/<locale>.json` under `data_page`.",
        docs: &[
            "docs/types.md#money",
            "docs/types.md#enums",
            "docs/types.md#soft-deletes",
            "docs/types.md#the-table",
            "docs/relations.md#a-page-of-rows-with-their-relations-no-n1",
            "docs/relations.md#polymorphic-relations",
            "docs/relations.md#through-a-middle-model-has_many_through",
            "docs/search.md#2-the-index",
            "docs/testing.md#factories",
            "docs/scheduling.md#a-typed-command-clap",
            "docs/authorization.md#roles-per-branch-a-role-in-one-store-for-a-while",
            "docs/authorization.md#lists-scopes_with",
            "docs/ui.md#infolists-read-only-details",
        ],
        sources: &[
            "examples/bikeshop/src/app/about/data.rs",
            "examples/bikeshop/resources/views/about/data.html",
            "examples/bikeshop/migrations/20260101000600_create_fleet_and_rentals_tables.up.sql",
            "examples/bikeshop/src/app/rentals/model.rs",
            "examples/bikeshop/src/app/rentals/factories.rs",
            "examples/bikeshop/src/app/access/catalogue.rs",
            "examples/bikeshop/src/app/access/policy.rs",
            "examples/bikeshop/src/seed/mod.rs",
            "examples/bikeshop/src/seed/history.rs",
            "examples/bikeshop/tests/data.rs",
            "examples/bikeshop/tests/seed.rs",
            "tests/browser/bikeshop-data.test.mjs",
        ],
    }
}

/// GET routes of this area that aren't pages (JSON, files, streams).
pub fn not_pages() -> Vec<NotAPage> {
    super::blocks::not_pages()
}
