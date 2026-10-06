//! "About this page" entries for the home area (see `crate::explain`).

use crate::explain::{Audience, Explanation, Feature, Flow, NotAPage};

/// The explanation of every page in this area.
pub fn entries() -> Vec<Explanation> {
    vec![Explanation {
        route: "home",

        path: "/",
        title: "Home",
        purpose: "The shop's front door: what the three stores offer (buy, rent, \
                  service) and where to start. It is also the first page of the public \
                  layout every customer-facing page shares.",
        who: "Anyone who opens the site: visitors and customers. Developers reading the \
              example start here too, with the \"About this page\" button.",
        audience: &[Audience::Visitor, Audience::Customer, Audience::Developer],
        flow: Flow::Buy,
        features: &[
            Feature {
                api: "Routes::get",
                why: "One `GET /` route named `home`, so every link to it is \
                      `route('home')` and the name keys this explanation.",
            },
            Feature {
                api: "UI kit: navbar",
                why: "The public layout's bar (`navbar`, `nav_links`, `menu`) comes from \
                      the kit: keyboard use, the phone layout and dark mode are already \
                      done, and the shop only sets its accent colour in `public/app.css`.",
            },
            Feature {
                api: "UI kit: columns + card",
                why: "The three ways to use the shop sit side by side on a CSS grid \
                      (`columns(3)`), one under the other on phones, with no CSS of the \
                      shop's own.",
            },
            Feature {
                api: "App::share",
                why: "The featured bikes (the best sellers) and the category tree come from \
                      the catalog area, which shares them with the view as `storefront` \
                      (`Registry::share`, computed only when the path is `/`, so other pages \
                      run no query for it). The home module stays ignorant of the catalogue.",
            },
            Feature {
                api: "UI kit: card_grid + media_card",
                why: "The featured bikes are the same `media_card`s as the catalogue's, in a \
                      `card_grid`; the categories are kit cards on a CSS grid.",
            },
            Feature {
                api: "Routes::etag",
                why: "`.etag()` hashes the page and answers `304 Not Modified` to a \
                      browser that already has it. The cart's count is fetched after the \
                      page loads, so it doesn't make the page differ per visitor; the \
                      layout's per-request CSP nonce still does, for now (a Renox gap, \
                      reported).",
            },
            Feature {
                api: "seo()",
                why: "The shop's name and lead as the title and description, with Open \
                      Graph tags for shared links.",
            },
            Feature {
                api: "App::detect_locale",
                why: "A visitor whose browser asks for Spanish gets the Spanish texts \
                      straight away; the language menu then remembers a choice with \
                      `renox::i18n::remember_locale`.",
            },
            Feature {
                api: "App::embed",
                why: "Views, translations and `public/` (motion.dev included) are compiled \
                      into the binary for production, so the shop deploys as one file.",
            },
            Feature {
                api: "motion.dev (vendored)",
                why: "The page's sections slide in with `transform` and `opacity` only \
                      (`public/app.js`), from a copy of Motion served by the app itself, \
                      so it works under the Content-Security-Policy and honours \
                      `prefers-reduced-motion`.",
            },
        ],
        under_hood: "The handler returns `view(\"home/index.html\")`; the catalog area's \
                     `storefront` share adds two queries for the featured bikes (by units \
                     sold) and four for their cards, plus one for the category tree. \
                     The view middleware renders it inside `layouts/app.html`, adding the \
                     shared `explain_panels` value and this explanation (the \
                     `about_page(…)` template function). The language menu \
                     posts to `locale.update`, which stores the choice in the session and \
                     goes `Back`.",
        docs: &[
            "docs/routing.md#apps-modules-and-routes",
            "docs/routing.md#etags",
            "docs/ui.md#navigation-and-page-structure",
            "docs/ui.md#the-current-route-conditional-classes-loops",
            "docs/laravel.md#one-file-to-deploy",
        ],
        sources: &[
            "examples/bikeshop/src/app/home/mod.rs",
            "examples/bikeshop/resources/views/home/index.html",
            "examples/bikeshop/resources/views/catalog/_home.html",
            "examples/bikeshop/src/app/catalog/mod.rs",
            "examples/bikeshop/resources/views/layouts/app.html",
            "examples/bikeshop/public/app.js",
            "examples/bikeshop/tests/about.rs",
        ],
    }]
}

/// GET routes of this area that aren't pages (JSON, files, streams).
pub fn not_pages() -> Vec<NotAPage> {
    vec![]
}
