//! "About this page" entries for the about area (see `crate::explain`).

use crate::explain::{Audience, Explanation, Feature, Flow, NotAPage};

/// The explanation of every page in this area.
pub fn entries() -> Vec<Explanation> {
    vec![Explanation {
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
    }]
}

/// GET routes of this area that aren't pages (JSON, files, streams).
pub fn not_pages() -> Vec<NotAPage> {
    vec![]
}
