//! "About this page" entries for the home area (see `crate::explain`).

use crate::explain::{Audience, Explanation, Feature, Flow, NotAPage};

/// The explanation of every page in this area.
pub fn entries() -> Vec<Explanation> {
    vec![Explanation {
        route: "home",

        path: "/",
        title: "Home",
        purpose: "The shop's front door: what the three stores offer (buy, rent, \
                  service), the best-selling bikes, the categories, and where to start. \
                  It is also the first page of the public layout every customer-facing \
                  page shares.",
        who: "Anyone who opens the site: visitors and customers. Developers reading the \
              example start here too, with the \"About this page\" button.",
        audience: &[Audience::Visitor, Audience::Customer, Audience::Developer],
        flow: Flow::Buy,
        features: &[
            Feature {
                api: "Routes::get",
                why: "One `GET /` route named `home`, so every link to it is \
                      `route('home')` and the name keys this explanation. Links never \
                      spell the path out, so moving the page later is a one-line change.",
            },
            Feature {
                api: "UI kit: navbar",
                why: "The public layout's bar (`navbar`, `nav_links`, `menu`) comes from \
                      the kit: keyboard use, the phone layout and dark mode are already \
                      done, and the shop only sets its accent colour in `public/app.css`. \
                      Each link is highlighted with \
                      `nav_link(…, active=route_is('catalog.*'))`, by route name rather \
                      than by comparing paths.",
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
                      the catalog area, which shares them with every view as `storefront` \
                      (`Registry::share` in its `register`). The value is computed only \
                      when the path is `/`, so other pages run no query for it, and the \
                      home module stays ignorant of the catalogue: the page includes \
                      `catalog/_home.html` with `ignore missing`.",
            },
            Feature {
                api: "UI kit: card_grid + media_card",
                why: "The featured bikes are the same `media_card`s as the catalogue's, in a \
                      `card_grid`, so a bike looks the same everywhere. The categories are \
                      kit cards (`rx-card`) on a small grid of the catalogue's \
                      (`public/catalog/catalog.css`), each with its sub-categories.",
            },
            Feature {
                api: "Routes::etag",
                why: "`.etag()` hashes the page and answers `304 Not Modified` to a \
                      browser that already has it. The cart's count is fetched after the \
                      page loads (`cart.mini`), so it doesn't make the page differ per \
                      visitor. The layout's per-request CSP nonce still does, so for now \
                      the home page is always sent whole (a Renox gap, issue #306).",
            },
            Feature {
                api: "seo()",
                why: "`seo(title=…, description=…)` in the page's `seo` block writes the \
                      shop's name and lead as the title and description, with the \
                      canonical address and the Open Graph tags for shared links, without \
                      the page writing any `<meta>` by hand.",
            },
            Feature {
                api: "App::detect_locale",
                why: "A visitor whose browser asks for Spanish gets the Spanish texts \
                      straight away, without a choice to make first. The language menu \
                      then remembers a choice with `renox::i18n::remember_locale`, which \
                      wins over the browser from then on.",
            },
            Feature {
                api: "App::embed",
                why: "Views, translations and `public/` (motion.dev included) are compiled \
                      into the binary for production, so the shop deploys as one file and \
                      a page can't go out with a template from another version.",
            },
            Feature {
                api: "motion.dev (vendored)",
                why: "The page's sections (`data-bs-reveal`) slide in with `transform` and \
                      `opacity` only (`public/app.js`), from a copy of Motion served by the \
                      app itself: no CDN to allow in the Content-Security-Policy, and \
                      `prefers-reduced-motion` turns it off.",
            },
        ],
        under_hood: "The handler returns `view(\"home/index.html\")` with no data of its \
                     own. The catalog area's `storefront` share runs six queries: one for \
                     the category tree, one for the ten best-selling bikes (by units sold \
                     in `order_items`), and four for their cards (photos, brands, \
                     categories, variants, loaded for all ten at once). The view \
                     middleware renders the page inside `layouts/app.html`, with the \
                     shared `explain_panels` value and this explanation (the \
                     `about_page(…)` template function). After the page loads, the cart \
                     button asks `cart.mini` for its count. The language menu posts to \
                     `locale.update`, which stores the choice in the session and goes \
                     `Back`; for someone logged in, the accounts area's middleware also \
                     saves it on the account.",
        docs: &[
            "docs/routing.md#apps-modules-and-routes",
            "docs/routing.md#etags",
            "docs/ui.md#navigation-and-page-structure",
            "docs/ui.md#the-current-route-conditional-classes-loops",
            "docs/ui.md#components-see-the-request",
            "docs/laravel.md#blade--minijinja",
            "docs/laravel.md#cache-storage-sessions-cookies-and-translations",
            "docs/laravel.md#one-file-to-deploy",
        ],
        sources: &[
            "examples/bikeshop/src/app/home/mod.rs",
            "examples/bikeshop/resources/views/home/index.html",
            "examples/bikeshop/resources/views/catalog/_home.html",
            "examples/bikeshop/src/app/catalog/mod.rs",
            "examples/bikeshop/resources/views/layouts/app.html",
            "examples/bikeshop/resources/views/layouts/_nav_cart.html",
            "examples/bikeshop/src/app/accounts/locale.rs",
            "examples/bikeshop/public/app.js",
            "examples/bikeshop/tests/about.rs",
            "examples/bikeshop/tests/catalog.rs",
            "tests/browser/bikeshop-about.test.mjs",
        ],
    }]
}

/// GET routes of this area that aren't pages (JSON, files, streams).
pub fn not_pages() -> Vec<NotAPage> {
    vec![]
}
