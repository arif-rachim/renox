//! "About this page" entries for the staff area (see `crate::explain`).

use crate::explain::{Audience, Explanation, Feature, Flow, NotAPage};

/// The explanation of every page in this area.
pub fn entries() -> Vec<Explanation> {
    vec![Explanation {
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
    }]
}

/// GET routes of this area that aren't pages (JSON, files, streams).
pub fn not_pages() -> Vec<NotAPage> {
    vec![]
}
