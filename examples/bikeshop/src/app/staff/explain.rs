//! "About this page" entries for the staff area (see `crate::explain`).

use crate::explain::{Audience, Explanation, Feature, Flow, NotAPage};

/// The explanation of every page in this area.
pub fn entries() -> Vec<Explanation> {
    let mut entries = vec![Explanation {
        route: "staff.dashboard",

        path: "/staff",
        title: "Staff dashboard",
        purpose: "Where the staff side starts after logging in. People who see reports in \
                  the active store get its last 7 days here (revenue, orders, rentals, work \
                  done, from the reports area's cached numbers) and a way into the reports; \
                  everyone else gets the shortcuts in the menu.",
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
                api: "Cache::remember",
                why: "The 7-day figures are the reports dashboard's own numbers for the \
                      active store (`reports::dashboard::overview`), cached until income \
                      changes, so the home page costs a few queries.",
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
            "examples/bikeshop/resources/views/reports/_overview.html",
        ],
    }];
    entries.extend(two_factor());
    entries.extend(back_office());
    entries.extend(admin_pages());
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

/// Who runs the back office.
const MANAGERS: &[Audience] = &[Audience::Manager, Audience::Owner];
/// The owner alone (by default; the matrix can change it).
const OWNER: &[Audience] = &[Audience::Owner];

/// Every staff page is guarded the same way.
const STAFF_GUARD: Feature = Feature {
    api: "Routes::require_permission",
    why: "`access::staff_routes` adds a login, the active store and `staff.access`; the \
          page's own permission comes on top and is checked **in the active store** \
          (`permissions::set_scope`), so a manager of North has it in North only. The \
          guard names a permission, never a role.",
};

/// The shared audit helper.
const AUDITED: Feature = Feature {
    api: "audit::record",
    why: "Every change is written to the audit log through `staff::audit::record`, \
          which adds the store being worked in and the role that granted the \
          permission, so the owner can see who changed what, where, and with which \
          rights.",
};

/// The store switcher, explained on every staff page that depends on it.
const SWITCHER: Feature = Feature {
    api: "renox::context + permissions::set_scope",
    why: "The store switcher in the top bar picks the store this request works in; \
          rights change with it because roles are given per store (#244). A record of \
          a store the person has no rights in answers **404, not 403**: for them it \
          doesn't exist, so ids can't be probed.",
};

/// The stores, team, roles, audit and catalogue pages.
fn back_office() -> Vec<Explanation> {
    vec![
        Explanation {
            route: "staff.stores.index",
            path: "/staff/stores",
            title: "Stores",
            purpose: "The three stores at a glance: address, phone, opening hours, how many \
                      mechanic minutes the workshop has per day, and the fee rate a store \
                      earns for work done for another (renting out its bike, selling its \
                      goods on consignment).",
            who: "The owner (`stores.manage`, which only the owner's role grants by default).",
            audience: OWNER,
            flow: Flow::BackOffice,
            features: &[
                STAFF_GUARD,
                Feature {
                    api: "UI kit: card + infolist",
                    why: "A card per store on a CSS grid that fills the row, each with the \
                          kit's infolist: the days formatted from their keys (`labels`), \
                          the rate with a suffix.",
                },
                Feature {
                    api: "access::can_in",
                    why: "Only the stores where the person holds `stores.manage` are listed: \
                          a check in each store's own scope, with no query (the roles were \
                          loaded with the user).",
                },
            ],
            under_hood: "Two queries: the stores, then their addresses with city and \
                         country (`FullAddress::load`, three small queries for any number).",
            docs: &[
                "docs/authorization.md#checking-one-record-has_permission_in",
                "docs/ui.md#infolists-read-only-details",
            ],
            sources: &[
                "examples/bikeshop/src/app/staff/stores.rs",
                "examples/bikeshop/resources/views/staff/stores/index.html",
                "examples/bikeshop/src/app/staff/model.rs",
            ],
        },
        Explanation {
            route: "staff.stores.edit",
            path: "/staff/stores/{store}/edit",
            title: "Edit a store",
            purpose: "Change a store's details, its opening hours day by day, the workshop's \
                      daily minutes and, with its own permission, the fee rate.",
            who: "The owner; the fee rate needs `settings.fees` as well.",
            audience: OWNER,
            flow: Flow::BackOffice,
            features: &[
                STAFF_GUARD,
                Feature {
                    api: "UI kit: repeater",
                    why: "Opening hours are a list of rows (day, opens, closes) people add, \
                          remove and reorder; the kit's `repeater` names the fields \
                          `hours[0][day]`, Renox reads them as a `Vec`, and `v.nested` \
                          checks each row (a known day, times as HH:MM). Stored as JSON \
                          (`Json<Vec<OpeningHours>>`), a list so the order is kept on both \
                          databases.",
                },
                Feature {
                    api: "Valid<T> + impl Validate",
                    why: "Rules written by hand here, because the rows need `v.nested` and \
                          the fee rate `between(0, 100)`.",
                },
                Feature {
                    api: "audit::record",
                    why: "Every save is recorded (`store.updated`); a new fee rate gets its \
                          own entry with the old and new rate (`store.fee_rate_changed`), \
                          since it moves money between the stores' books.",
                },
            ],
            under_hood: "The store must be one the person manages (else a 404). On save: \
                         the address row and the store are updated; the fee rate only when \
                         `settings.fees` is held in that store (a value sent without it is \
                         ignored), converted from a percentage to basis points (integers, \
                         never floats, for money).",
            docs: &[
                "docs/ui.md#rows-of-fields",
                "docs/validation.md#rules-with-impl-validate",
                "docs/authorization.md#sensitive-actions-and-the-audit-trail",
            ],
            sources: &[
                "examples/bikeshop/src/app/staff/stores.rs",
                "examples/bikeshop/resources/views/staff/stores/edit.html",
                "examples/bikeshop/tests/staff.rs",
            ],
        },
        Explanation {
            route: "staff.team.index",
            path: "/staff/team",
            title: "Team",
            purpose: "Who works in the store being worked in: people whose home store it is, \
                      and people given a role here (a helper from another store this week \
                      shows up too).",
            who: "Store managers and the owner (`staff.manage`).",
            audience: MANAGERS,
            flow: Flow::BackOffice,
            features: &[
                STAFF_GUARD,
                SWITCHER,
                Feature {
                    api: "UI kit: table + badge",
                    why: "One row per person with their roles here as badges; on a phone the \
                          kit's table scrolls inside its card.",
                },
            ],
            under_hood: "A fixed number of queries: the users with a role in this store, the \
                         staff rows, their logins, their roles in force here or globally (one \
                         query with `IN`), and the stores' names.",
            docs: &[
                "docs/authorization.md#managing-assignments",
                "docs/authorization.md#roles-per-branch-a-role-in-one-store-for-a-while",
            ],
            sources: &[
                "examples/bikeshop/src/app/staff/team.rs",
                "examples/bikeshop/resources/views/staff/team/index.html",
            ],
        },
        Explanation {
            route: "staff.team.show",
            path: "/staff/team/{staff}",
            title: "A member of staff",
            purpose: "One person's roles in every store with their dates, giving a role in \
                      this store (optionally from and until a date), taking one away, and \
                      deactivating someone who leaves.",
            who: "Store managers and the owner (`staff.manage`).",
            audience: MANAGERS,
            flow: Flow::BackOffice,
            features: &[
                STAFF_GUARD,
                Feature {
                    api: "assign_role_in(…).from(…).until(…)",
                    why: "Roles are given in a store, with dates (#244): help for a week ends \
                          by itself, no one has to remember to take it back. Only roles \
                          whose every permission the giver holds here are offered, so a \
                          manager can't make an owner.",
                },
                Feature {
                    api: "User::revoke_sessions",
                    why: "Deactivating removes every role and ends every session of the \
                          person at once (`users.sessions_revoked_at`, checked on each \
                          request), so a login shared with or stolen by someone else stops \
                          working everywhere.",
                },
                Feature {
                    api: "UI kit: confirm + date_picker",
                    why: "Taking a role away and deactivating ask first, in the kit's \
                          confirmation sheet; the dates are the kit's date picker.",
                },
                AUDITED,
                SWITCHER,
            ],
            under_hood: "The person must belong to the active store (else a 404). Giving a \
                         role upserts a `role_user` row with the store's scope and the dates; \
                         removing deletes it; deactivating deletes all their rows, sets \
                         `staff.active = false` and bumps `sessions_revoked_at`, all audited \
                         with what was removed.",
            docs: &[
                "docs/authorization.md#managing-assignments",
                "docs/authorization.md#roles-per-branch-a-role-in-one-store-for-a-while",
                "docs/routing.md#sessions",
            ],
            sources: &[
                "examples/bikeshop/src/app/staff/team.rs",
                "examples/bikeshop/resources/views/staff/team/show.html",
                "examples/bikeshop/tests/staff.rs",
            ],
        },
        Explanation {
            route: "staff.invitations.create",
            path: "/staff/team/invite",
            title: "Invite to the team",
            purpose: "Invite someone by mail to work in this store with a role.",
            who: "Store managers and the owner (`staff.manage`).",
            audience: MANAGERS,
            flow: Flow::BackOffice,
            features: &[
                STAFF_GUARD,
                Feature {
                    api: "Signed URLs",
                    why: "The link is signed with `APP_KEY` over the store, the role and the \
                          address, and expires in seven days: no invitations table, and \
                          nobody can change the role in the link.",
                },
                Feature {
                    api: "queue_mail",
                    why: "The mail is queued and sent by a worker with retries.",
                },
                AUDITED,
            ],
            under_hood: "The role must be one the inviter may give here. The link is \
                         `state.signed_url(\"staff.invitations.accept\", [store, role, email])`; \
                         `mail/staff/invitation.html` is rendered and queued, and \
                         `staff.invited` recorded.",
            docs: &["docs/routing.md#signed-urls", "docs/mail.md#sending-a-mail"],
            sources: &[
                "examples/bikeshop/src/app/staff/team.rs",
                "examples/bikeshop/resources/views/staff/team/invite.html",
                "examples/bikeshop/resources/views/mail/staff/invitation.html",
            ],
        },
        Explanation {
            route: "staff.invitations.accept",
            path: "/staff/join/{store}/{role}/{email}",
            title: "Join the team",
            purpose: "The page an invitation opens: choose a name and a password (or log in \
                      with an existing account) and join the store's team with the role \
                      given.",
            who: "Someone who received a staff invitation.",
            audience: &[Audience::Visitor, Audience::Staff],
            flow: Flow::BackOffice,
            features: &[
                Feature {
                    api: "Signed URLs (ValidSignature)",
                    why: "A changed or expired link answers 403 before anything is read.",
                },
                Feature {
                    api: "View overrides (renox/auth/layout.html)",
                    why: "The page uses the shop's sign-in layout, like Renox's own pages.",
                },
                Feature {
                    api: "renox-2fa",
                    why: "After joining, the new member of staff logs in, and two-factor \
                          login is set up before the back office opens.",
                },
            ],
            under_hood: "On submit: the user is made (address marked verified, since the link \
                         reached it) or the logged-in one used; the `staff` row with this home \
                         store; the role in the store (`assign_role_in`); `staff.joined` \
                         recorded. Used once: a second time finds the role already given.",
            docs: &[
                "docs/routing.md#signed-urls",
                "docs/two-factor.md#what-your-users-see",
            ],
            sources: &[
                "examples/bikeshop/src/app/staff/team.rs",
                "examples/bikeshop/resources/views/staff/team/join.html",
                "examples/bikeshop/tests/staff.rs",
            ],
        },
        Explanation {
            route: "staff.roles.index",
            path: "/staff/roles",
            title: "Roles and permissions",
            purpose: "The role × permission matrix: what each role may do, switched on and \
                      off live. The shop's rights change without a developer.",
            who: "The owner (`roles.manage`).",
            audience: OWNER,
            flow: Flow::BackOffice,
            features: &[
                Feature {
                    api: "Permissions module (RBAC)",
                    why: "A role is a named set of permissions; **code only ever checks \
                          permissions** (`require_permission`, `can()`, `allows`), never a \
                          role's name (`tests/access.rs` fails on one). So one switch here \
                          changes what every cashier may do, from their next page.",
                },
                Feature {
                    api: "permissions::grant / revoke",
                    why: "Each switch posts on its own with htmx (`hx-post`, `hx-trigger: \
                          change`) and answers with a toast; Renox loads roles with the user \
                          on each request, so there is no cache to clear.",
                },
                Feature {
                    api: "UI kit: checkbox(switch=true)",
                    why: "The kit's switch, one per cell, with a hidden label (\"Cashier may \
                          rentals.checkout\") for screen readers; laid out by a CSS grid \
                          with table roles, the role headings sticky.",
                },
                AUDITED,
            ],
            under_hood: "Loading: two queries (`permissions::roles`). A switch: the role and \
                         the permission must exist (else a 404); a global role can't lose \
                         `roles.manage` or `staff.access` (else a 403), so the owner can't \
                         lock themselves out; then one insert or delete in \
                         `permission_role` and an audit entry.",
            docs: &[
                "docs/authorization.md#roles-and-permissions",
                "docs/ui.md#htmx-response-headers",
            ],
            sources: &[
                "examples/bikeshop/src/app/staff/roles.rs",
                "examples/bikeshop/resources/views/staff/roles/index.html",
                "examples/bikeshop/src/app/access/catalogue.rs",
                "examples/bikeshop/tests/staff.rs",
            ],
        },
        Explanation {
            route: "staff.audit.index",
            path: "/staff/audit",
            title: "Audit log",
            purpose: "Every sensitive change in the company: role and permission changes, \
                      roles given and taken, prices, stock adjustments, refunds, ID \
                      approvals, fee rates, logins; who, in which store, with which role, \
                      when.",
            who: "The owner (`audit.view`).",
            audience: OWNER,
            flow: Flow::BackOffice,
            features: &[
                Feature {
                    api: "Audit module",
                    why: "Renox's `Audit` module owns `audit_logs` and records the auth \
                          events itself (logins, lockouts, deleted accounts, two-factor and \
                          social login changes); the shop adds a store and a role column and \
                          records its own actions with `staff::audit::record`.",
                },
                Feature {
                    api: "renox::grid",
                    why: "The log is a data grid: newest first, each heading a filter by its \
                          kind (dates, the person and the store from their tables through \
                          `Column::related`), a search on the action, 50 rows a page, CSV \
                          and Excel exports.",
                },
                Feature {
                    api: "audit:prune",
                    why: "The table grows forever otherwise: `cargo run -- audit:prune --days \
                          365` (Renox's command) deletes older entries.",
                },
            ],
            under_hood: "One query for the page and one for the count; the related columns \
                         are subqueries, so there's no N+1.",
            docs: &[
                "docs/authorization.md#sensitive-actions-and-the-audit-trail",
                "docs/grid.md#filters-search-and-chips",
                "docs/operations.md#tables-that-keep-growing",
            ],
            sources: &[
                "examples/bikeshop/src/app/staff/audit.rs",
                "examples/bikeshop/resources/views/staff/audit/index.html",
                "examples/bikeshop/migrations/20260102000200_add_store_and_role_to_audit_logs.up.sql",
            ],
        },
        Explanation {
            route: "staff.catalog.move",
            path: "/staff/catalog/move/{token}",
            title: "Move products to a category",
            purpose: "The admin panel's \"Move to a category…\" bulk action leads here to \
                      pick the category for the selected products.",
            who: "Whoever manages the catalogue (`catalog.manage`).",
            audience: OWNER,
            flow: Flow::BackOffice,
            features: &[
                Feature {
                    api: "Cache",
                    why: "The selection waits in the cache for half an hour under a random \
                          token, for the same person only.",
                },
                Feature {
                    api: "Toast actions",
                    why: "The panel's action answers with a toast that links here \
                          (`Toast::link`), since `renox-admin` actions take no input yet.",
                },
                AUDITED,
            ],
            under_hood: "Loading reads the selection and the categories; moving is one \
                         `UPDATE … WHERE id IN (…)`, recorded as `catalog.category_moved`.",
            docs: &["docs/admin.md#actions", "docs/scheduling.md#cache"],
            sources: &[
                "examples/bikeshop/src/app/staff/catalog_tools.rs",
                "examples/bikeshop/resources/views/staff/catalog/move.html",
            ],
        },
        Explanation {
            route: "staff.catalog.fits",
            path: "/staff/catalog/fits/{product}",
            title: "What fits",
            purpose: "Which bike models a spare part fits (or, from a bike, which parts fit \
                      it), with a note: what the catalogue uses to find parts for a \
                      customer's bike and the workshop uses for repairs.",
            who: "Whoever manages the catalogue (`catalog.manage`).",
            audience: OWNER,
            flow: Flow::BackOffice,
            features: &[
                Feature {
                    api: "Pivot (part_fits)",
                    why: "A many-to-many between products with a `note` on the link \
                          (Pagila's `film_actor`): `load_with_pivot` reads the links with \
                          their notes, `attach_with` and `detach` change them, and \
                          `inverse()` gives the same table seen from the bike.",
                },
                Feature {
                    api: "UI kit: list + select(searchable)",
                    why: "The linked products as a list with a remove button each, and a \
                          searchable select to add one.",
                },
                AUDITED,
            ],
            under_hood: "Three queries: the product, the links with the products they point \
                         at, and the products that could be added.",
            docs: &[
                "docs/relations.md#pivot-columns",
                "docs/relations.md#changing-a-many-to-many",
            ],
            sources: &[
                "examples/bikeshop/src/app/staff/catalog_tools.rs",
                "examples/bikeshop/resources/views/staff/catalog/fits.html",
                "examples/bikeshop/src/app/catalog/model.rs",
            ],
        },
    ]
}

/// The admin panel's resources: (slug, plural, singular, permission, what it holds).
const RESOURCES: &[(&str, &str, &str, &str, &str)] = &[
    (
        "categories",
        "Categories",
        "category",
        "catalog.manage",
        "the catalogue's tree of categories, each of a kind (bikes, gear, parts)",
    ),
    (
        "brands",
        "Brands",
        "brand",
        "catalog.manage",
        "the brands the shop sells",
    ),
    (
        "products",
        "Products",
        "product",
        "catalog.manage",
        "bike models, gear and spare parts, with a Markdown description written in \
      `renox-editors`' `markdown_editor`; discontinued ones go to the trash",
    ),
    (
        "product_variants",
        "Variants",
        "variant",
        "catalog.manage",
        "the SKUs that are sold and stocked: size, colour, price and cost (integers)",
    ),
    (
        "product-photos",
        "Photos",
        "photo",
        "catalog.manage",
        "the products' photos, with the alternative text screen readers read",
    ),
    (
        "service-tasks",
        "Service tasks",
        "service task",
        "plans.manage",
        "what the workshop does, with its minutes and price",
    ),
    (
        "service-plans",
        "Service plans",
        "service plan",
        "plans.manage",
        "the plans customers subscribe a bike to (weekly, monthly…)",
    ),
    (
        "suppliers",
        "Suppliers",
        "supplier",
        "purchasing.manage",
        "who the stores buy from, with their lead time",
    ),
    (
        "stores",
        "Stores",
        "store",
        "stores.manage",
        "the three stores' names and contacts (edit only: hours and fees are on the stores page)",
    ),
];

/// The panel's shared features.
const ADMIN: Feature = Feature {
    api: "renox-admin (AdminResource)",
    why: "The model is declared once (its grid columns, form fields, rules, filters and \
          actions) and the panel makes the list, the forms, the view page, exports and \
          the trash, on the kit and the data grid. No page of it is written by hand.",
};

const ADMIN_POLICY: Feature = Feature {
    api: "Policy (by permission)",
    why: "Every page and button asks the model's `Policy`, which answers with a \
          **permission** in the active store (`user.has_permission`), never a role: \
          `catalog.manage`, `plans.manage`, `purchasing.manage` or `stores.manage`; \
          prices need `prices.change` as well. A layer runs the active-store middleware on \
          `/admin` too, so a manager's rights count in the store they work in.",
};

const ADMIN_LAYOUT: Feature = Feature {
    api: "View overrides (renox-admin/layout.html)",
    why: "The shop replaces the panel's frame with its own (brand, store switcher, this \
          panel) and its field macro (`renox-admin/fields.html`, for the Markdown \
          editor), by files of the same name.",
};

const ADMIN_DOCS: &[&str] = &[
    "docs/admin.md#declaring-a-resource",
    "docs/admin.md#the-policy",
    "docs/admin.md#replacing-the-panels-pages",
];

const ADMIN_SOURCES: &[&str] = &[
    "examples/bikeshop/src/app/staff/admin.rs",
    "examples/bikeshop/resources/views/renox-admin/layout.html",
    "examples/bikeshop/resources/views/renox-admin/fields.html",
    "examples/bikeshop/tests/staff.rs",
];

/// Leaks a string once (the explanations are `&'static`, made once per run).
fn leak(text: String) -> &'static str {
    Box::leak(text.into_boxed_str())
}

/// The admin panel's pages: the dashboard and each resource's list, form
/// and view page, made from [`RESOURCES`] once.
fn admin_pages() -> Vec<Explanation> {
    static PAGES: std::sync::OnceLock<Vec<Explanation>> = std::sync::OnceLock::new();
    PAGES
        .get_or_init(|| {
            let mut pages = vec![Explanation {
                route: "admin.dashboard",
                path: "/admin",
                title: "Admin panel",
                purpose: "The catalogue, the workshop's tasks and plans, the suppliers and the \
                          stores, each with how many records it has: the panel's home.",
                who: "The owner, and managers with a catalogue, plans, purchasing or stores \
                      permission in their store.",
                audience: MANAGERS,
                flow: Flow::BackOffice,
                features: &[
                    ADMIN,
                    Feature {
                        api: "Admin::authorize",
                        why: "The panel opens for `staff.access` plus any of its permissions \
                              (`staff::admin::PANEL_PERMISSIONS`); a resource you may not \
                              see is left out of the sidebar and the counts.",
                    },
                    ADMIN_LAYOUT,
                ],
                under_hood: "One `COUNT(*)` per resource the person may see.",
                docs: ADMIN_DOCS,
                sources: ADMIN_SOURCES,
            }];
            for (slug, plural, one, permission, holds) in RESOURCES {
                let pages_of = [
                    (
                        "index",
                        format!("/admin/{slug}"),
                        plural.to_string(),
                        format!(
                            "The list of {holds}: search, a filter on every heading, sorting, \
                              bulk actions and exports."
                        ),
                    ),
                    (
                        "create",
                        format!("/admin/{slug}/create"),
                        format!("New {one}"),
                        format!("A form for a new {one}."),
                    ),
                    (
                        "show",
                        format!("/admin/{slug}/{{id}}"),
                        format!("A {one}"),
                        format!("One {one}'s details, in the kit's infolist."),
                    ),
                    (
                        "edit",
                        format!("/admin/{slug}/{{id}}/edit"),
                        format!("Edit a {one}"),
                        format!("A {one}'s form."),
                    ),
                ];
                for (page, path, title, purpose) in pages_of {
                    let mut features = vec![ADMIN, ADMIN_POLICY];
                    if page == "index" {
                        features.push(Feature {
                            api: "renox::grid",
                            why: "The list is the data grid: each column filters by its kind, \
                                  `Column::related` shows a name from another table (one \
                                  subquery, no N+1), and the user's column choices are kept.",
                        });
                        if *slug == "products" {
                            features.push(Feature {
                                api: "AdminAction",
                                why: "Bulk actions: prices ±5 % or ±10 % on every variant \
                                      (`prices.change`, audited), move to a category (a page \
                                      to pick it), discontinue (to the trash). The \"Products\" \
                                      tabs are `Filter`s by kind; the trash restores.",
                            });
                        }
                    } else if page != "show" {
                        features.push(Feature {
                            api: "Valid<T> + #[derive(Validate)]",
                            why: "The form is a typed struct with its rules, plus `rules()` for \
                                  what needs the record (a unique slug that may be its own); \
                                  errors show under the fields without a reload (htmx), and \
                                  each field is checked as you leave it.",
                        });
                        if *slug == "products" || *slug == "service-plans" {
                            features.push(Feature {
                                api: "renox-editors (markdown_editor)",
                                why: "The description is Markdown with a toolbar and a preview; \
                                      the shop shows it with the `markdown` filter, which \
                                      prints any HTML as text.",
                            });
                        }
                    }
                    features.push(ADMIN_LAYOUT);
                    pages.push(Explanation {
                        route: leak(format!("admin.{slug}.{page}")),
                        path: leak(path),
                        title: leak(title),
                        purpose: leak(purpose),
                        who: leak(format!(
                            "Anyone with `{permission}` in the store they work in."
                        )),
                        audience: MANAGERS,
                        flow: Flow::BackOffice,
                        features: Box::leak(features.into_boxed_slice()),
                        under_hood: leak(match page {
                            "index" => "One query for the page and one for the count, with the \
                                        filters, search and sort from the address; the policy is \
                                        asked once for the list's buttons."
                                .to_owned(),
                            "show" => "The record is read (a 404 if missing), the policy asked \
                                       `view`, and the entries formatted by kind."
                                .to_owned(),
                            _ => format!(
                                "On save: the policy is asked, the form validated, \
                                          `fill` copies it into the {one} and the model is \
                                          saved (its hooks run); back to the list with a toast."
                            ),
                        }),
                        docs: ADMIN_DOCS,
                        sources: ADMIN_SOURCES,
                    });
                }
            }
            pages
        })
        .clone()
}
