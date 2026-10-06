//! "About this page" entries for the staff area (see `crate::explain`).

use crate::explain::{Audience, Explanation, Feature, Flow, NotAPage};

/// The explanation of every page in this area.
pub fn entries() -> Vec<Explanation> {
    let mut entries = vec![Explanation {
        route: "staff.dashboard",

        path: "/staff",
        title: "Staff dashboard",
        purpose: "Where the staff side starts after logging in. For now it holds the \
                  back office's layout; the dashboards with each store's numbers come \
                  with the reports.",
        who: "Everyone on a store's staff: cashiers, mechanics, store managers and the \
              owner, each seeing what their roles in the chosen store allow.",
        audience: &[
            Audience::Staff,
            Audience::Cashier,
            Audience::Mechanic,
            Audience::Manager,
            Audience::Owner,
        ],
        flow: Flow::BackOffice,
        features: &[
            Feature {
                api: "UI kit: sidebar + rx-shell",
                why: "The back office's frame: navigation down the side, the account menu \
                      and the store switcher on top, and a bar on phones, all from the \
                      kit (`sidebar`, `sidebar_link`, `rx-shell`).",
            },
            Feature {
                api: "Routes::require_permission",
                why: "The staff side needs `staff.access`, a permission every staff role \
                      grants, in the store the person works in today: a customer with a \
                      login gets a 403, a guest goes to the login page first \
                      (`access::staff_routes` adds both guards).",
            },
            Feature {
                api: "permissions::set_scope",
                why: "The active store: a middleware picks the store this request works in \
                      (the session's choice, checked against the person's roles today) and \
                      makes it the request's scope, so every permission check on the page \
                      counts the roles given in that store (#244).",
            },
            Feature {
                api: "UI kit: page_header + empty",
                why: "The page's title row and the placeholder message, so even an empty \
                      page looks finished.",
            },
        ],
        under_hood: "The auth middleware has already loaded the user and every role \
                     they hold (with its store and dates). `require_auth` sends guests to \
                     `/login`; the active-store middleware reads the chosen store from \
                     the session, keeps it if a role grants `staff.access` there today \
                     (else the home store), and calls `permissions::set_scope`; then \
                     `require_permission(\"staff.access\")` checks it. The layout's store \
                     switcher lists the stores where the person may work now (one query \
                     for their names). The handler renders `staff/dashboard.html` in \
                     `layouts/staff.html`.",
        docs: &[
            "docs/ui.md#navigation-and-page-structure",
            "docs/routing.md#guards",
            "docs/authorization.md#roles-per-branch-a-role-in-one-store-for-a-while",
        ],
        sources: &[
            "examples/bikeshop/src/app/staff/mod.rs",
            "examples/bikeshop/resources/views/staff/dashboard.html",
            "examples/bikeshop/resources/views/layouts/staff.html",
            "examples/bikeshop/src/app/access/active_store.rs",
            "examples/bikeshop/resources/views/layouts/_store_switcher.html",
        ],
    }];
    entries.extend(two_factor());
    entries
}

/// renox-2fa's pages: required for staff, optional for customers.
const TWO_FACTOR: Feature = Feature {
    api: "renox-2fa (Registry::second_factor)",
    why: "One module adds the whole second step: the `two_factor` table, these pages, \
          the card on the account page and the check after the password \
          (`Registry::second_factor`), so a stolen password alone opens nothing.",
};

/// Who must use it.
const STAFF_MUST: Feature = Feature {
    api: "Events (LoggedIn) + App::layer",
    why: "Two-factor login is optional for customers and required for staff: a \
          `LoggedIn` listener notes a member of staff who logged in without it, and a \
          layer on `/staff` and `/admin` sends them to set it up before anything else \
          opens (`src/app/staff/two_factor.rs`). The check asks for a permission \
          (`staff.access`), never a role's name.",
};

fn two_factor() -> Vec<Explanation> {
    let docs: &'static [&'static str] = &[
        "docs/two-factor.md#what-your-users-see",
        "docs/two-factor.md#how-it-keeps-accounts-safe",
        "docs/authorization.md#a-second-login-step-two-factor-authentication",
    ];
    vec![
        Explanation {
            route: "two-factor.challenge",
            path: "/two-factor/challenge",
            title: "Two-factor code",
            purpose: "After the right password, the login waits here for the six-digit \
                      code from the authenticator app (or a recovery code).",
            who: "Every member of staff at each login; customers who turned it on.",
            audience: &[Audience::Staff, Audience::Owner, Audience::Customer],
            flow: Flow::Account,
            features: &[
                TWO_FACTOR,
                Feature {
                    api: "Login throttle",
                    why: "Wrong codes count towards the same throttle as wrong passwords, \
                          so codes can't be guessed either.",
                },
                Feature {
                    api: "View overrides (renox/auth/layout.html)",
                    why: "The plugin's pages extend Renox's sign-in layout, which the shop \
                          replaced: they get the brand and this panel with no copy of them.",
                },
            ],
            under_hood: "The pending login waits in the session for ten minutes. The code \
                         is checked against the secret (decrypted with `APP_KEY`) for the \
                         current 30-second step and its neighbours; a step already used is \
                         refused. The right code finishes the login (`complete_login`) and \
                         emits `LoggedIn`; a recovery code is used up.",
            docs,
            sources: &[
                "crates/renox-2fa/src/handlers.rs",
                "crates/renox-2fa/views/challenge.html",
                "examples/bikeshop/src/app/staff/two_factor.rs",
            ],
        },
        Explanation {
            route: "two-factor.setup",
            path: "/two-factor/setup",
            title: "Set up two-factor login",
            purpose: "Scan the QR code with an authenticator app and type the first code \
                      to turn two-factor login on. Staff are sent here before the back \
                      office opens.",
            who: "Staff (required) and customers (optional), from their account page.",
            audience: &[Audience::Staff, Audience::Owner, Audience::Customer],
            flow: Flow::Account,
            features: &[
                TWO_FACTOR,
                STAFF_MUST,
                Feature {
                    api: "Routes::require_password_confirmed",
                    why: "Turning it on asks for the password again unless it was typed in \
                          the last three hours, so someone at an unlocked computer can't.",
                },
                Feature {
                    api: "db::Encrypted",
                    why: "The shared secret is sealed with `APP_KEY` in the table and shown \
                          only on this page, once.",
                },
            ],
            under_hood: "`two-factor.enable` made a new secret (not active yet); this page \
                         draws it as an SVG QR code and as text. The first right code turns \
                         it on, emits `TwoFactorEnabled` (written to the audit log) and \
                         lifts the staff side's block for this person.",
            docs,
            sources: &[
                "crates/renox-2fa/src/handlers.rs",
                "crates/renox-2fa/src/qr.rs",
                "crates/renox-2fa/views/setup.html",
                "examples/bikeshop/src/app/staff/two_factor.rs",
            ],
        },
        Explanation {
            route: "two-factor.recovery-codes",
            path: "/two-factor/recovery-codes",
            title: "Recovery codes",
            purpose: "Eight one-time codes for the day the phone is lost, shown once with \
                      a download button, right after two-factor login is turned on or new \
                      codes are made.",
            who: "Anyone who just turned two-factor login on.",
            audience: &[Audience::Staff, Audience::Owner, Audience::Customer],
            flow: Flow::Account,
            features: &[
                TWO_FACTOR,
                Feature {
                    api: "Hashed recovery codes",
                    why: "Only a SHA-256 of each code is kept, like a password, so the page \
                          can show them only once and a database leak gives nothing away.",
                },
            ],
            under_hood: "The codes come from the session, put there when they were made, \
                         and are removed from it once shown.",
            docs: &[
                "docs/two-factor.md#recovery-codes",
                "docs/two-factor.md#how-it-keeps-accounts-safe",
            ],
            sources: &[
                "crates/renox-2fa/src/recovery.rs",
                "crates/renox-2fa/views/recovery-codes.html",
            ],
        },
    ]
}

/// GET routes of this area that aren't pages (JSON, files, streams).
pub fn not_pages() -> Vec<NotAPage> {
    vec![]
}
