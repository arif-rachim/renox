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
        under_hood: "No database query: the handler returns `view(\"home/index.html\")`. \
                     The view middleware renders it inside `layouts/app.html`, adding the \
                     shared `explain_panels` value and this explanation (the \
                     `about_page(…)` template function). The language menu \
                     posts to `locale.update`, which stores the choice in the session and \
                     goes `Back`.",
        docs: &[
            "docs/routing.md#apps-modules-and-routes",
            "docs/ui.md#navigation-and-page-structure",
            "docs/ui.md#the-current-route-conditional-classes-loops",
            "docs/laravel.md#one-file-to-deploy",
        ],
        sources: &[
            "examples/bikeshop/src/app/home/mod.rs",
            "examples/bikeshop/resources/views/home/index.html",
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
