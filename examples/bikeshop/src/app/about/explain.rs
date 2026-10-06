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
                      own URL to share.",
                },
                Feature {
                    api: "Lang",
                    why: "The extractor gives the request's language; each explanation is \
                      read from `resources/lang/<locale>.json` when it has the text, and \
                      falls back to English otherwise.",
                },
                Feature {
                    api: "Routes::get",
                    why: "Every page is a named route; the list links to the ones without \
                      parameters with `state.url(name, &[])`.",
                },
                Feature {
                    api: "UI kit: toolbar + select",
                    why: "The filters line up in the kit's `toolbar`, which wraps on phones \
                      and needs no layout CSS.",
                },
                Feature {
                    api: "UI kit: list + badge",
                    why: "Each page is a row of the kit's `list`, its features and roles as \
                      `badge`s.",
                },
                Feature {
                    api: "App::templates",
                    why: "The \"About this page\" panel on every page comes from the \
                      `about_page(…)` template function the app registers, so \
                      handlers don't pass anything for it.",
                },
                Feature {
                    api: "App::share",
                    why: "`explain_panels` (from `BIKESHOP_EXPLAIN`) is given to every view, \
                      so the layouts can hide the panels for a clean demo.",
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
                "docs/ui.md#navigation-and-page-structure",
                "docs/laravel.md#cache-storage-sessions-cookies-and-translations",
                "docs/testing.md#a-first-test",
            ],
            sources: &[
                "examples/bikeshop/src/app/about/mod.rs",
                "examples/bikeshop/src/explain.rs",
                "examples/bikeshop/resources/views/about/pages.html",
                "examples/bikeshop/resources/views/about/_panel.html",
                "examples/bikeshop/tests/about.rs",
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
                why: "One model per table (36 of them), in the area that owns it: \
                      `src/app/<area>/model.rs`. The derive writes the column list, the \
                      reads and the writes; the table names are written out \
                      (`#[model(table = \"rentals\")]`), plural as Renox recommends.",
            },
            Feature {
                api: "renox::db::relations",
                why: "`belongs_to`, `has_many`, `Pivot` (`part_fits` with a note), `Morph` \
                      (payments, stock movements and intercompany entries point at several \
                      tables) and `has_many_through` (a customer's work orders through \
                      their bikes): each loads a whole page's relations in a fixed number \
                      of queries, which `tests/data.rs` checks with `capture_queries`.",
            },
            Feature {
                api: "DbEnum",
                why: "Every status and kind is an enum stored as a word; the page lists \
                      them from `ALL`, so it can't drift from the code.",
            },
            Feature {
                api: "Encrypted<T>",
                why: "The customer's ID number is sealed with `APP_KEY` in the table and \
                      never serialized (`#[serde(skip_serializing)]`).",
            },
            Feature {
                api: "#[model(soft_deletes)]",
                why: "Customers who leave and discontinued products are hidden, not \
                      removed: their rentals, orders and payments still point at them.",
            },
            Feature {
                api: "renox::db::search",
                why: "Products are searchable by name, brand, SKU and description \
                      (`#[model(search = …)]`): an FTS5 table on SQLite, a generated \
                      `tsvector` on PostgreSQL, from one migration Renox writes.",
            },
            Feature {
                api: "Factory",
                why: "Every model has a factory with states named after the business \
                      (`rentals().overdue()`, `rental_bikes().placed_at(store)`), used by \
                      the seeders and by every test.",
            },
            Feature {
                api: "AppCommand",
                why: "`demo:seed --size large` is a typed clap command next to the \
                      built-in `db:seed`; it refuses a seeded database with how to start \
                      again.",
            },
            Feature {
                api: "Permissions module",
                why: "The roles and what they grant are read from the database \
                      (`permissions::roles`), so the page shows what the owner set, with \
                      the labels and the permission catalogue from `access::catalogue`.",
            },
            Feature {
                api: "renox::db::sql",
                why: "The live row counts are one `UNION ALL` query over every table, \
                      read into `(String, i64)` pairs with `fetch_as`.",
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
        ],
    }
}

/// GET routes of this area that aren't pages (JSON, files, streams).
pub fn not_pages() -> Vec<NotAPage> {
    vec![]
}
