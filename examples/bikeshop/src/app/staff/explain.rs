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
                api: "Routes::require_auth",
                why: "Only logged-in people reach the staff side; a guest is sent to the \
                      login page and comes back here afterwards.",
            },
            Feature {
                api: "UI kit: page_header + empty",
                why: "The page's title row and the placeholder message, so even an empty \
                      page looks finished.",
            },
        ],
        under_hood: "The auth middleware has already loaded the user from the session; \
                     `require_auth` (a route layer) answers guests with a redirect to \
                     `/login`. The handler renders `staff/dashboard.html` in \
                     `layouts/staff.html`. No query runs yet.",
        docs: &[
            "docs/ui.md#navigation-and-page-structure",
            "docs/routing.md#guards",
        ],
        sources: &[
            "examples/bikeshop/src/app/staff/mod.rs",
            "examples/bikeshop/resources/views/staff/dashboard.html",
            "examples/bikeshop/resources/views/layouts/staff.html",
        ],
    }]
}

/// GET routes of this area that aren't pages (JSON, files, streams).
pub fn not_pages() -> Vec<NotAPage> {
    vec![]
}
