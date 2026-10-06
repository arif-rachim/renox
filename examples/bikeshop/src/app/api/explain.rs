//! "About this page" entries for the api area's pages (see `crate::explain`).
//! The JSON endpoints aren't pages: each is listed in [`not_pages`], and
//! explained on `/about/api` ([`super::endpoints`]).

use crate::explain::{Audience, Explanation, Feature, Flow, NotAPage};

const MOD: &str = "examples/bikeshop/src/app/api/mod.rs";
const KIOSK: &str = "examples/bikeshop/src/app/api/kiosk.rs";
const CUSTOMER: &str = "examples/bikeshop/src/app/api/customer.rs";
const TOKENS: &str = "examples/bikeshop/src/app/api/tokens.rs";
const ENDPOINTS: &str = "examples/bikeshop/src/app/api/endpoints.rs";
const TESTS: &str = "examples/bikeshop/tests/api.rs";

/// The explanation of every page in this area.
pub fn entries() -> Vec<Explanation> {
    vec![
        Explanation {
            route: "api.about",
            path: "/about/api",
            title: "The JSON API",
            purpose: "Every endpoint of the shop's JSON API, for the kiosks next to the bike \
                      racks and for the shop's mobile app: what it is for, the token ability \
                      it needs, what to send, what comes back, and a `curl` example to try \
                      it. A test checks the list against the app's routes, as the walker \
                      checks pages.",
            who: "Developers of the kiosk and of the mobile app; anyone reading the example.",
            audience: &[Audience::Developer],
            flow: Flow::Learn,
            features: &[
                Feature {
                    api: "Authorization: Bearer (API tokens)",
                    why: "Programs don't log in with a password: they send a token made on a \
                          tokens page. Renox's auth middleware finds the user from it (only a \
                          SHA-256 hash is stored), and Bearer requests skip CSRF, since a \
                          browser never sends one on its own.",
                },
                Feature {
                    api: "Routes::require_ability",
                    why: "Each endpoint needs one ability (`rentals:checkout`, `rent`…): a \
                          token without it gets 403, no token 401, so a kiosk's token can't \
                          read customers' orders and a lost one can only do what it was \
                          given.",
                },
                Feature {
                    api: "Valid<T> (JSON)",
                    why: "The API reads the website's own forms (`ReserveForm`, the counter's \
                          `PickupForm` and `ReturnForm`) from JSON, or multipart for photos, \
                          and a refusal is Renox's `422 {\"message\", \"errors\"}`, the same \
                          rules as the pages.",
                },
                Feature {
                    api: "Routes::throttle_by",
                    why: "The `bikeshop-api` limiter (`App::rate_limiter` in `src/lib.rs`) \
                          counts per token (its id, before the `|`): 120 a minute, then 429 \
                          with `Retry-After`.",
                },
                Feature {
                    api: "Routes::cors",
                    why: "The app's web build on its own origin may call the API from a \
                          browser: Renox answers the preflight and adds the \
                          `Access-Control-Allow-*` headers for that origin only.",
                },
                Feature {
                    api: "Ulid",
                    why: "Rentals are found by their reservation code, a `Ulid`: it can't be \
                          guessed or counted the way 1, 2, 3 can.",
                },
                Feature {
                    api: "Query::paginate",
                    why: "Lists come a page at a time, with `meta` (total, last page) and \
                          `links` (first, last, prev, next) made from the paginator.",
                },
            ],
            under_hood: "No query: the page renders the endpoint list in \
                         `src/app/api/endpoints.rs`. The API itself shares the website's \
                         functions: `booking::book` and `reserve::cancel_rental` for \
                         customers, `counter::hand_over` and `counter::take_back` for \
                         kiosks.",
            docs: &[
                "docs/authorization.md#api-tokens-and-abilities",
                "docs/routing.md#rate-limits",
                "docs/routing.md#cors",
                "docs/validation.md#answer-1-htmx-and-json-requests-get-a-422",
            ],
            sources: &[
                MOD,
                ENDPOINTS,
                KIOSK,
                CUSTOMER,
                "examples/bikeshop/resources/views/api/about.html",
                TESTS,
            ],
        },
        Explanation {
            route: "api.tokens",
            path: "/account/api-tokens",
            title: "My API tokens",
            purpose: "A customer's personal tokens for the shop's mobile app (or their own \
                      scripts): make one with the abilities it needs (read, rent, order), \
                      see it once to copy it, see when each was last used, revoke one.",
            who: "Logged-in customers.",
            audience: &[Audience::Customer],
            flow: Flow::Account,
            features: &[
                Feature {
                    api: "User::create_token_with",
                    why: "A token limited to the abilities ticked and expiring in a year; \
                          `tokens()` lists them and `revoke_token` ends one at once.",
                },
                Feature {
                    api: "Session flash",
                    why: "Only the token's hash is stored, so the secret is shown once: \
                          flashed into the next page, gone after it.",
                },
                Feature {
                    api: "UI kit: checkbox_list + input (copyable) + table",
                    why: "The abilities as a checkbox list, the new token in a read-only \
                          field with the kit's copy button, the tokens in a table.",
                },
            ],
            under_hood: "One query for the tokens. Making one inserts a \
                         `personal_access_tokens` row (its SHA-256 hash, the abilities as \
                         JSON, the expiry); revoking deletes it.",
            docs: &["docs/authorization.md#api-tokens-and-abilities"],
            sources: &[
                TOKENS,
                "examples/bikeshop/resources/views/api/tokens.html",
                TESTS,
            ],
        },
        Explanation {
            route: "api.kiosks",
            path: "/staff/api-tokens",
            title: "Kiosk tokens",
            purpose: "The active store's self-service kiosks: a manager makes a kiosk's \
                      token with the rental abilities it needs (see bikes, check out, take \
                      back), copies it into the kiosk once, and revokes it when a kiosk is \
                      lost or replaced.",
            who: "Store managers (the permission `fleet.manage` in the store).",
            audience: &[Audience::Manager, Audience::Owner],
            flow: Flow::BackOffice,
            features: &[
                Feature {
                    api: "access::staff_routes + require_permission",
                    why: "The page works in the active store and needs `fleet.manage` there; \
                          a kiosk is made for that store and revoked only by someone who \
                          manages that store's fleet (404 elsewhere).",
                },
                Feature {
                    api: "User::create_token_with",
                    why: "Each kiosk acts as its own user (a random password nobody is \
                          told) with one token limited to `rentals:read`, \
                          `rentals:checkout` and `rentals:return` as ticked; the API then \
                          only shows that store's reservations.",
                },
                Feature {
                    api: "Session flash",
                    why: "The token is shown once, right after it is made.",
                },
            ],
            under_hood: "Two queries: the store's kiosks and their tokens' last use. Making a \
                         kiosk registers its user, makes the token and writes the `kiosks` \
                         row; revoking deletes the user's tokens and marks the row revoked.",
            docs: &[
                "docs/authorization.md#api-tokens-and-abilities",
                "docs/authorization.md#roles-per-branch-a-role-in-one-store-for-a-while",
            ],
            sources: &[
                TOKENS,
                KIOSK,
                "examples/bikeshop/resources/views/api/kiosks.html",
                "examples/bikeshop/migrations/20260103001100_create_kiosks_table.up.sql",
                TESTS,
            ],
        },
    ]
}

/// GET routes of this area that aren't pages (JSON, files, streams).
pub fn not_pages() -> Vec<NotAPage> {
    const JSON: &str = "a JSON API endpoint, explained on /about/api";
    [
        "api.kiosk.bikes",
        "api.kiosk.rental",
        "api.me",
        "api.products",
        "api.products.show",
        "api.me.bikes",
        "api.me.work_orders",
        "api.me.plan",
        "api.me.rentals",
        "api.me.orders",
    ]
    .into_iter()
    .map(|route| NotAPage {
        route,
        reason: JSON,
    })
    .collect()
}
