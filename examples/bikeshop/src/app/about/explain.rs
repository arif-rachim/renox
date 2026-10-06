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
        Explanation {
            route: "about.blocks",
            path: "/about/blocks",
            title: "Bike shop blocks",
            purpose: "Every UI block the bike shop adds to Renox's kit, working, with its \
                  macro's signature: a photo gallery, a two-handle range, a quantity \
                  stepper, a counter keypad, a kanban board, a month calendar, an \
                  availability timeline, a date and time range, a date picker with closed \
                  days, variant chips, a history timeline and plan cards. The other pages \
                  use them; this one shows them side by side.",
            who: "Developers who want one of the blocks on their own page, and anyone \
              checking how they look and behave (keyboard, phone, dark mode).",
            audience: &[Audience::Developer],
            flow: Flow::Learn,
            features: &[
                Feature {
                    api: "Bike shop blocks",
                    why: "The kit has no gallery, kanban or calendar, and the owner chose to \
                      build them in the example rather than in Renox: macros in \
                      `resources/views/blocks/`, written like the kit's (keyword \
                      arguments, `bs-` classes on the kit's `--rx-*` tokens), with one \
                      stylesheet and one script in `public/blocks/`, so they can move to \
                      a crate later. A page imports one like a kit component \
                      (`{% from \"blocks/gallery.html\" import gallery %}`); inside, the \
                      blocks call the kit's own macros (`sheet`, `date_picker`).",
                },
                Feature {
                    api: "UI kit: sheet",
                    why: "The gallery's enlarged photo opens in the kit's `sheet`: focus \
                      moves into it and back, Escape and the backdrop close it.",
                },
                Feature {
                    api: "Valid<T>",
                    why: "Every form block sends plain fields (two numbers, one date-time per \
                      end, a radio's value), so `Valid<BlocksForm>` reads them like any \
                      form. Its `after` hook refuses the closed days and past dates again \
                      on the server: the greyed-out calendar is only a help.",
                },
                Feature {
                    api: "Query<T>",
                    why: "The price filter is a GET form: the range's two handles become \
                      `?price_min=…&price_max=…`, and the calendar's arrows `?month=…`.",
                },
                Feature {
                    api: "htmx fragments",
                    why: "A variant chip asks `GET /about/blocks/variant` for the price and \
                      stock (`hx-get` with `hx-include`), and a kanban move is an htmx \
                      POST from a hidden form; Renox adds the CSRF header to both.",
                },
                Feature {
                    api: "motion.dev (vendored)",
                    why: "The gallery's slides and the kanban's cards glide with Motion on \
                      `transform`; under `prefers-reduced-motion` they jump instead.",
                },
            ],
            under_hood: "No database. `src/app/about/blocks.rs` builds the demo data (a \
                     month of visits on fixed days of whichever month is shown, today's \
                     bookings, a workshop board) and renders `about/blocks.html`. \
                     `public/blocks/blocks.js` sets every block up from its `data-bs-*` \
                     attributes, on the page and in whatever htmx swaps in \
                     (`htmx:load`), with no inline handlers, so it runs under `CSP=strict`. \
                     Sending the form posts to `POST /about/blocks/form`: `Valid<T>` \
                     checks it, and a success comes back with a flash message; a kanban \
                     move posts the card, the column and the position to \
                     `POST /about/blocks/kanban`, which answers `204` (or a `422` that \
                     puts the card back).",
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
                "examples/bikeshop/resources/views/blocks/gallery.html",
                "examples/bikeshop/resources/views/blocks/kanban.html",
                "examples/bikeshop/public/blocks/blocks.js",
                "examples/bikeshop/public/blocks/blocks.css",
                "examples/bikeshop/tests/blocks.rs",
                "tests/browser/bikeshop-blocks.test.mjs",
            ],
        },
    ]
}

/// GET routes of this area that aren't pages (JSON, files, streams).
pub fn not_pages() -> Vec<NotAPage> {
    super::blocks::not_pages()
}
