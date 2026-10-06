//! "About this page" entries for the sales area's pages (see `crate::explain`).

use crate::explain::{Audience, Explanation, Feature, Flow, NotAPage};

/// The explanation of every page in this area.
pub fn entries() -> Vec<Explanation> {
    vec![Explanation {
        route: "cart.show",
        path: "/cart",
        title: "Cart",
        purpose: "What the shopper is about to buy, checked against the stock of one store: \
                  the store they pick up from (or that sends the delivery). Quantities can be \
                  changed or lines removed without a reload; then on to the checkout.",
        who: "Shoppers, logged in or not. A guest's cart follows them into their account \
              when they log in.",
        audience: &[Audience::Visitor, Audience::Customer, Audience::Developer],
        flow: Flow::Buy,
        features: &[
            Feature {
                api: "Sessions",
                why: "A guest's cart is a small value in the session (`session.put(\"cart\", …)`): \
                      no table rows for people who never buy. A logged-in customer's cart is a \
                      row in `carts`, so it is there on their phone and their laptop; the first \
                      request after logging in merges the session's cart into it \
                      (`Cart::load`).",
            },
            Feature {
                api: "View::also",
                why: "A change answers the cart's `lines` block plus the navbar's `mini` \
                      block (`.fragment(\"lines\").also(\"mini\")`); the second one has \
                      `hx-swap-oob`, so htmx puts it into the navbar's cart link while the \
                      first replaces the cart: one answer, two places updated.",
            },
            Feature {
                api: "Toast",
                why: "Every change says what happened in a toast (\"Only 2 left at North: \
                      your cart has 2\"), through the `HX-Trigger` header for htmx or the \
                      session after a plain form's redirect.",
            },
            Feature {
                api: "renox::db::relations",
                why: "The lines' variants, products, categories, photos and the store's \
                      stock are read in six queries however long the cart is \
                      (`find_many`, `has_many`).",
            },
            Feature {
                api: "Bike shop blocks",
                why: "The quantity of each line is the `quantity` block (− / number / +); \
                      each step sends `change`, which htmx turns into a `PATCH` after 300 ms.",
            },
            Feature {
                api: "Method spoofing",
                why: "Without JavaScript the quantity and remove forms are `POST`s carrying \
                      `_method=PATCH` / `DELETE`, routed like the htmx requests.",
            },
        ],
        under_hood: "Each request loads the cart (session or `carts`), then the variants and \
                     the chosen store's `stock_levels` (available = on hand − reserved, over \
                     every owner of the goods there). A line asking for more than the store \
                     has is lowered to what's left, and the page says so; a discontinued \
                     product's line goes. Changing the store checks every line against the \
                     new store at once. Nothing is reserved yet: that happens when the order \
                     is placed, in a transaction (the checkout).",
        docs: &[
            "docs/routing.md#sessions",
            "docs/ui.md#fragments-and-out-of-band-swaps",
            "docs/ui.md#toasts",
            "docs/routing.md#method-spoofing",
        ],
        sources: &[
            "examples/bikeshop/src/app/sales/cart.rs",
            "examples/bikeshop/resources/views/sales/cart/show.html",
            "examples/bikeshop/resources/views/sales/cart/_mini.html",
            "examples/bikeshop/resources/views/layouts/_nav_cart.html",
            "examples/bikeshop/migrations/20260102000700_create_carts_table.up.sql",
            "examples/bikeshop/tests/sales.rs",
        ],
    }]
}

/// GET routes of this area that aren't pages (JSON, files, streams).
pub fn not_pages() -> Vec<NotAPage> {
    vec![NotAPage {
        route: "cart.mini",
        reason: "an htmx fragment: the navbar's cart link with its count (explained on the cart page)",
    }]
}
