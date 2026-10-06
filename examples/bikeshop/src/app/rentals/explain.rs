//! "About this page" entries for the rentals area's pages (see `crate::explain`).

use crate::explain::{Audience, Explanation, Feature, Flow, NotAPage};

const MOD: &str = "examples/bikeshop/src/app/rentals/mod.rs";
const MODEL: &str = "examples/bikeshop/src/app/rentals/model.rs";
const PRICING: &str = "examples/bikeshop/src/app/rentals/pricing.rs";
const BOOKING: &str = "examples/bikeshop/src/app/rentals/booking.rs";
const RESERVE: &str = "examples/bikeshop/src/app/rentals/reserve.rs";
const IDENTITY: &str = "examples/bikeshop/src/app/rentals/identity.rs";
const COUNTER: &str = "examples/bikeshop/src/app/rentals/counter.rs";
const FLEET: &str = "examples/bikeshop/src/app/rentals/fleet.rs";
const TASKS: &str = "examples/bikeshop/src/app/rentals/tasks.rs";
const NOTIFY: &str = "examples/bikeshop/src/app/rentals/notify.rs";
const POLICY: &str = "examples/bikeshop/src/app/access/policy.rs";
const TESTS: &str = "examples/bikeshop/tests/rentals.rs";
const BROWSER: &str = "tests/browser/bikeshop-rentals.test.mjs";

/// The explanation of every page in this area.
pub fn entries() -> Vec<Explanation> {
    vec![
        Explanation {
            route: "rentals.create",
            path: "/rent",
            title: "Rent a bike",
            purpose: "Where a rental starts: pick a store, a start and an end (by the hour or \
                      the day), a bike type and a size, and see the bikes standing at that \
                      store and free for the whole period, each with its price and deposit, \
                      then reserve one. Under the list, the store's day as a timeline of \
                      bikes against hours; a free hour starts a search from there.",
            who: "Customers planning a ride (anyone can look; reserving needs an account).",
            audience: &[Audience::Visitor, Audience::Customer],
            flow: Flow::Rent,
            features: &[
                Feature {
                    api: "blocks: datetime_range",
                    why: "The kit's `date_picker` picks a day but not an hour; the example's \
                          `datetime_range` block puts two of them beside time selects and sends \
                          plain `starts_at` / `ends_at` fields (`YYYY-MM-DDTHH:MM`), read as \
                          `NaiveDateTime` in `APP_TIMEZONE` and turned into moments in Rust.",
                },
                Feature {
                    api: "blocks: availability",
                    why: "The store's day as a table of bikes × hours (booked, free); each free \
                          hour is a link, so the timeline works without JavaScript and reads \
                          well to a screen reader (\"Trail 5, 11:00, Book\").",
                },
                Feature {
                    api: "Htmx",
                    why: "The search form re-asks the same page as it changes (`hx-get`, \
                          `hx-select` of the results, `hx-push-url`), so only the bike list \
                          moves and the address can be shared; without JavaScript it is a \
                          plain GET form.",
                },
                Feature {
                    api: "Valid<T> + after hook",
                    why: "The reservation form's rules run first; its `after` hook then checks \
                          the period and that the bike is still free (the **first** of two \
                          overlap checks), so a clash shows next to the bike with the form \
                          kept.",
                },
                Feature {
                    api: "Db::begin_immediate + lock_for_update",
                    why: "The **second** overlap check, in the transaction that writes the \
                          rental: the bike's row is taken first (SQLite's write lock, \
                          PostgreSQL's row lock), so two customers booking the last bike at \
                          once run one after the other and the second is told it was just \
                          taken. Both checks are needed: the hook gives a friendly error, the \
                          transaction makes it true.",
                },
                Feature {
                    api: "Pricing in Rust",
                    why: "`pricing::quote` is a plain function (hours counted when started, \
                          capped at the daily rate; whole days at the daily rate; the deposit), \
                          unit-tested without a database and shared by the page, the booking \
                          and the counter.",
                },
                Feature {
                    api: "StoreAttr::Location",
                    why: "Availability is by **location**: the bikes listed are those standing \
                          at the store now, whoever owns them (a bike North placed at South is \
                          South's to rent out, #245). The owner store is copied onto the rental \
                          for the books.",
                },
            ],
            under_hood: "Loading: the stores, the bike categories, the variants at the store \
                         (for sizes and names), the free bikes (two queries: the bikes at the \
                         store, then the clashing rentals of all of them at once) and the day's \
                         timeline (two more). Reserving (`POST /rent`): `Valid<ReserveForm>` \
                         with its `after` hook, then `booking::book` in one transaction \
                         (`begin_immediate`, the bike's row with `lock_for_update`, the clash \
                         query again, the insert with a fresh `Ulid` code), then a redirect to \
                         the reservation, where the deposit is paid. A customer who never sent \
                         an ID is sent to the ID page first.",
            docs: &[
                "docs/validation.md#hooks-prepare-authorize-after",
                "docs/relations.md#more-of-the-query-builder",
                "docs/ui.md#what-htmx-sent-the-htmx-extractor",
                "docs/types.md#dates-and-times",
                "docs/authorization.md#checking-one-record-has_permission_in",
            ],
            sources: &[
                RESERVE,
                BOOKING,
                PRICING,
                MODEL,
                "examples/bikeshop/resources/views/rentals/search.html",
                "examples/bikeshop/resources/views/blocks/datetime_range.html",
                "examples/bikeshop/resources/views/blocks/availability.html",
                TESTS,
                BROWSER,
            ],
        },
        Explanation {
            route: "rentals.show",
            path: "/rentals/{code}",
            title: "A reservation",
            purpose: "The reservation after booking: its code (to show at the counter), the \
                      bike, the store, the period, the price and the deposit, the deadline \
                      to pay the deposit online and the time the bike is held until; \
                      \"Pay the deposit\" and \"Cancel\" while they apply.",
            who: "The customer who made it (anyone else gets a 404).",
            audience: &[Audience::Customer],
            flow: Flow::Rent,
            features: &[
                Feature {
                    api: "Ulid",
                    why: "The reservation code is a `Ulid`: unique without asking the \
                          database, sortable by time, short enough to read out at the counter, \
                          and it doesn't reveal how many rentals there are (an id would).",
                },
                Feature {
                    api: "renox::webhook",
                    why: "\"Pay the deposit\" starts an online payment through the shared \
                          payments contract (`payments::start`); the gateway's webhook marks it \
                          paid and emits `PaymentSucceeded`, which this area's listener turns \
                          into a held deposit and a confirmation mail with the code.",
                },
                Feature {
                    api: "Schedule::every_minutes",
                    why: "The page states the two deadlines the `rentals:watch` task enforces \
                          every 15 minutes: an unpaid reservation is called off after 30 \
                          minutes, and one not picked up 30 minutes after its start is a \
                          no-show (the bike is released, the deposit keeps the price at most, \
                          the customer is told).",
                },
                Feature {
                    api: "Events and listeners",
                    why: "`PaymentSucceeded` / `PaymentFailed` are the sales area's events; \
                          rentals only listens for `Payable::Rental`, so the payment code never \
                          knows about rentals.",
                },
                Feature {
                    api: "UI kit: infolist",
                    why: "The reservation's details as the kit's `infolist`: labels and values, \
                          money and dates formatted, the code `copyable`.",
                },
            ],
            under_hood: "The rental is looked up by code **and** customer (a 404 otherwise), \
                         loaded with its bike, model, customer and stores by \
                         `RentalRow::load` (a fixed number of queries). Cancelling (`POST \
                         /rentals/{code}/cancel`) is allowed until an hour before the start; a \
                         held deposit is given back whole, and a mail and an in-app \
                         notification say so.",
            docs: &[
                "docs/types.md#keys",
                "docs/routing.md#csrf",
                "docs/scheduling.md#scheduled-tasks",
                "docs/scheduling.md#events",
                "docs/ui.md#infolists-read-only-details",
            ],
            sources: &[
                RESERVE,
                MODEL,
                TASKS,
                "examples/bikeshop/src/app/sales/payments.rs",
                "examples/bikeshop/resources/views/rentals/show.html",
                TESTS,
            ],
        },
        Explanation {
            route: "rentals.mine",
            path: "/rentals",
            title: "My rentals",
            purpose: "The customer's rentals: the current ones (reserved, out, overdue) with \
                      their code and due time, and the past ones with the price, the late \
                      and damage fees and what came back of the deposit. Also whether the ID \
                      check is done.",
            who: "Customers.",
            audience: &[Audience::Customer],
            flow: Flow::Account,
            features: &[
                Feature {
                    api: "relations::belongs_to",
                    why: "`RentalRow::load` brings each rental's bike, model, customer and \
                          stores in one query per relation, however many rentals: no N+1 \
                          (`tests/rentals.rs` counts them).",
                },
                Feature {
                    api: "UI kit: list + badge",
                    why: "Each rental is a row of the kit's `list`, its status a `badge` whose \
                          colour and text agree (never colour alone).",
                },
                Feature {
                    api: "money filter",
                    why: "Money is stored as integers in the smallest unit and shown with \
                          `money` in the visitor's locale (`Rp 150,000` / `Rp 150.000`).",
                },
            ],
            under_hood: "Two queries for the customer and their latest 50 rentals, then the \
                         relations of all of them at once, then the latest ID document.",
            docs: &[
                "docs/relations.md#a-page-of-rows-with-their-relations-no-n1",
                "docs/types.md#money",
            ],
            sources: &[
                RESERVE,
                MODEL,
                "examples/bikeshop/resources/views/rentals/mine.html",
                TESTS,
            ],
        },
        Explanation {
            route: "rentals.identity",
            path: "/rentals/identity",
            title: "ID check",
            purpose: "Before the first rental, the customer sends a photo of their ID \
                      document and its number, and picks the store that checks it. Once \
                      staff approve it, they rent in every store without showing it again.",
            who: "Customers renting for the first time.",
            audience: &[Audience::Customer],
            flow: Flow::Rent,
            features: &[
                Feature {
                    api: "Upload",
                    why: "The photo is a form field (`Option<Upload>`) checked with `image()` \
                          (by its **content**, sniffed, not its name) and `max`, then stored \
                          with `Upload::store` under a random name **outside** `public/`: no \
                          address reaches it.",
                },
                Feature {
                    api: "Encrypted<String>",
                    why: "The ID number is the customer's `id_number`, sealed with `APP_KEY` \
                          when saved (`save_only` writes just that column), so a copy of the \
                          database leaks nothing; staff see it masked.",
                },
                Feature {
                    api: "Valid<T> + prepare hook",
                    why: "`prepare` tidies the number (trimmed, upper case) before the rules \
                          check its length and characters.",
                },
                Feature {
                    api: "notify",
                    why: "The chosen store's staff who may verify IDs get an in-app \
                          notification; the customer gets a mail and a notification when it is \
                          approved or refused.",
                },
            ],
            under_hood: "On submit: the photo goes to private storage, the number is sealed \
                         onto the customer, an earlier pending document is set aside, an \
                         `identity_documents` row is written, and `notify::staff` finds the \
                         users holding `rentals.verify_id` in that store (a query over the \
                         permission tables, never a role name).",
            docs: &[
                "docs/validation.md#uploads",
                "docs/validation.md#hooks-prepare-authorize-after",
                "docs/mail.md#database-notifications",
            ],
            sources: &[
                IDENTITY,
                NOTIFY,
                MODEL,
                "examples/bikeshop/resources/views/rentals/identity.html",
                TESTS,
            ],
        },
        Explanation {
            route: "rentals.identities",
            path: "/staff/identities",
            title: "ID checks waiting",
            purpose: "The ID documents customers sent to this store, oldest first: open the \
                      photo, compare it with the masked number, approve or refuse with a \
                      note.",
            who: "Cashiers and managers (`rentals.verify_id` in the store).",
            audience: &[Audience::Cashier, Audience::Manager],
            flow: Flow::Rent,
            features: &[
                Feature {
                    api: "Signed URLs",
                    why: "\"Open photo\" goes through a route that checks the permission in the \
                          document's store (or a store serving one of the customer's rentals) \
                          and then redirects to `storage.temporary_url`, a link signed with \
                          `APP_KEY` that works for five minutes. Other customers and staff \
                          without the permission never get a link.",
                },
                Feature {
                    api: "scopes_with",
                    why: "The list is `access::visible::<IdentityDocument>(rentals.verify_id)`: \
                          the documents of the stores where the person holds the permission \
                          now (all of them for the owner's global role).",
                },
                Feature {
                    api: "has_permission_in",
                    why: "Approving checks the permission in the document's store, not the \
                          active store: ABAC on the record's own attribute.",
                },
                Feature {
                    api: "Toast",
                    why: "Approve and refuse answer with a toast and go back to the list.",
                },
            ],
            under_hood: "Three queries: the waiting documents, their customers, their stores. \
                         Approving sets the document's status and the customer's \
                         `id_verified_at` (valid in every store) and notifies the customer.",
            docs: &[
                "docs/routing.md#signed-urls",
                "docs/authorization.md#lists-scopes_with",
                "docs/authorization.md#checking-one-record-has_permission_in",
                "docs/ui.md#toasts",
            ],
            sources: &[
                IDENTITY,
                POLICY,
                "examples/bikeshop/resources/views/rentals/identities.html",
                TESTS,
            ],
        },
        Explanation {
            route: "rentals.counter",
            path: "/staff/rentals",
            title: "Rental counter",
            purpose: "A cashier's day: the pick-ups due today at this store, the bikes out \
                      and the overdue ones, and a search by reservation code or customer. \
                      A rental out from another store shows up too when it is searched for: \
                      bikes can come back anywhere.",
            who: "Cashiers and staff at the counter (`rentals.view` in the active store).",
            audience: &[Audience::Cashier, Audience::Staff, Audience::Manager],
            flow: Flow::Rent,
            features: &[
                Feature {
                    api: "renox::context",
                    why: "The active store is the access area's middleware's choice, kept in \
                          `renox::context` for the request; every list is that store's.",
                },
                Feature {
                    api: "StoreAttr::Operating",
                    why: "Pick-ups belong to the **operating** store (where the bike stands). \
                          Returns are allowed in any store where the person may take bikes \
                          back, so the search also finds rentals out from another store.",
                },
                Feature {
                    api: "relations::belongs_to",
                    why: "All three lists and the search results are loaded together by \
                          `RentalRow::load`: one query per relation, not per row.",
                },
                Feature {
                    api: "UI kit: columns + list",
                    why: "Three columns on a desk, one under the other on a phone (the kit's \
                          `columns`, a CSS grid), each a `list` of rentals.",
                },
            ],
            under_hood: "One query for the store's rentals (out, overdue, or reserved to \
                         start before tomorrow), one for the search, then `RentalRow::load` \
                         once for all of them.",
            docs: &[
                "docs/authorization.md#roles-per-branch-a-role-in-one-store-for-a-while",
                "docs/relations.md#a-page-of-rows-with-their-relations-no-n1",
                "docs/ui.md#navigation-and-page-structure",
            ],
            sources: &[
                COUNTER,
                "examples/bikeshop/src/app/access/active_store.rs",
                "examples/bikeshop/resources/views/rentals/counter.html",
                TESTS,
                BROWSER,
            ],
        },
        Explanation {
            route: "rentals.walkin",
            path: "/staff/rentals/walk-in",
            title: "Walk-in rental",
            purpose: "A rental for someone standing at the counter without a reservation: \
                      a verified customer, a period starting now, and a bike standing here \
                      and free for it. It is booked like a reservation, then opened at the \
                      desk for the pick-up.",
            who: "Cashiers and staff (`rentals.checkout` in the active store).",
            audience: &[Audience::Cashier, Audience::Staff],
            flow: Flow::Rent,
            features: &[
                Feature {
                    api: "renox::select",
                    why: "The customer is a searchable select whose options come from the \
                          server as the cashier types (`options_url`, `OptionQuery`): only \
                          verified customers, never the whole table in the page.",
                },
                Feature {
                    api: "blocks: datetime_range",
                    why: "The period, starting now, by the hour.",
                },
                Feature {
                    api: "Valid<T> + after hook",
                    why: "The hook checks the customer is verified, the period, and that the \
                          bike stands here and is free, with the errors next to the fields; \
                          the booking transaction checks the overlap again.",
                },
            ],
            under_hood: "The bikes standing at the store and free for the next two hours \
                         (two queries) fill the bike select. On submit the same \
                         `booking::book` transaction as online reservations runs, with the \
                         active store as operating store and the cashier as `served_by`.",
            docs: &[
                "docs/ui.md#options-from-the-server",
                "docs/validation.md#hooks-prepare-authorize-after",
            ],
            sources: &[
                COUNTER,
                BOOKING,
                "examples/bikeshop/resources/views/rentals/walk_in.html",
                TESTS,
            ],
        },
        Explanation {
            route: "rentals.desk",
            path: "/staff/rentals/{rental}",
            title: "Pick-up and return",
            purpose: "One rental at the counter. Before the ride: the customer (verified, the \
                      ID masked), the condition checklist, and what is paid now (the price, \
                      and the deposit when it wasn't paid online). After it: the checklist \
                      again, the late fee so far, damage (photos, a note, a fee), and how the \
                      deposit settles against the fees.",
            who: "Cashiers and staff: the pick-up in the operating store, the return in any \
                  store.",
            audience: &[Audience::Cashier, Audience::Staff],
            flow: Flow::Rent,
            features: &[
                Feature {
                    api: "renox::context",
                    why: "The active store decides what the page offers: the pick-up only in \
                          the rental's operating store, the return wherever the person may take \
                          bikes back; the bike's **location** becomes the store that took it \
                          back, its **owner** never changes.",
                },
                Feature {
                    api: "UI kit: checkbox_list + toggle_buttons",
                    why: "The condition checklist is a `checkbox_list` (a `Vec<String>`, each \
                          item checked with `one_of`), cash or card a `toggle_buttons`.",
                },
                Feature {
                    api: "Upload",
                    why: "Damage photos are a `Vec<Upload>` (each `image()` by content, at most \
                          six), stored privately and shown to staff through signed links.",
                },
                Feature {
                    api: "Events and listeners",
                    why: "A damaged return emits `FleetRepairNeeded`; the workshop area listens \
                          and opens a work order at this store, billed to the bike's owner store \
                          when it is another. Every return emits `RentalClosed`, where the \
                          intercompany books (#245) book the revenue to the owner store and the \
                          operating store's fee.",
                },
                Feature {
                    api: "Transactions",
                    why: "The rental, the bike (status, location, ridden hours) and the photos \
                          are written in one transaction; the counter payments follow through \
                          the shared payments contract.",
                },
                Feature {
                    api: "Pricing in Rust",
                    why: "`pricing::late_fee` (per started hour after 15 minutes of grace) and \
                          `pricing::settle` (fees out of the deposit first, the rest given back, \
                          any excess paid now) are unit-tested functions; the page shows their \
                          answer for now.",
                },
            ],
            under_hood: "Pick-up: `access::require(rentals.checkout, Operating)`, the customer \
                         must be verified, one transaction (rental active, checklist, bike \
                         `rented`), then `payments::record_counter` for the price and, when \
                         due, the deposit. Return: the photos to private storage, one \
                         transaction (late and damage fees, the deposit settled, the return \
                         store, the ridden minutes; the bike's location, hours and status), \
                         the excess paid at the counter, the events, and a mail with the \
                         receipt to the customer.",
            docs: &[
                "docs/authorization.md#checking-one-record-has_permission_in",
                "docs/scheduling.md#events",
                "docs/validation.md#uploads",
                "docs/relations.md#more-of-the-query-builder",
                "docs/ui.md#form-fields",
            ],
            sources: &[
                COUNTER,
                PRICING,
                MOD,
                "examples/bikeshop/src/app/workshop/fleet.rs",
                "examples/bikeshop/resources/views/rentals/desk.html",
                TESTS,
                BROWSER,
            ],
        },
        Explanation {
            route: "rentals.receipt",
            path: "/staff/rentals/{rental}/receipt",
            title: "Rental receipt",
            purpose: "What the customer paid and got back, the payments recorded, how the \
                      rental is booked between the bike's owner store and the operating \
                      store (revenue and fees to the owner, the operating store's fee at its \
                      rate), and where the bike should go next when it came back elsewhere.",
            who: "Cashiers, after a return; managers checking a rental.",
            audience: &[Audience::Cashier, Audience::Manager],
            flow: Flow::Rent,
            features: &[
                Feature {
                    api: "Morph",
                    why: "The payments are `payments` rows pointing at the rental through a \
                          polymorphic reference (`payable_type = 'rentals'`), the same table \
                          sales and the workshop use.",
                },
                Feature {
                    api: "StoreAttr::Owner",
                    why: "The books follow the **owner** store copied onto the rental: the \
                          owner earns the revenue and fees, the operating store its fee \
                          (`fee_rate_bp`, 20 % by default). #245 writes those entries on \
                          `RentalClosed`; the receipt shows the same numbers.",
                },
                Feature {
                    api: "money filter",
                    why: "Integers in the smallest unit, formatted in the visitor's locale.",
                },
            ],
            under_hood: "The rental, its payments, the bike, the store it was placed at (the \
                         latest placement that moved it, else the owner), and the stores' \
                         names: a handful of queries.",
            docs: &[
                "docs/relations.md#polymorphic-relations",
                "docs/types.md#money",
            ],
            sources: &[
                COUNTER,
                "examples/bikeshop/src/app/multistore/model.rs",
                "examples/bikeshop/resources/views/rentals/receipt.html",
                TESTS,
            ],
        },
        Explanation {
            route: "rentals.fleet",
            path: "/staff/fleet",
            title: "Fleet board",
            purpose: "A store's rental bikes at a glance: available, reserved, out, overdue, \
                      in maintenance; who owns each and where it stands; the hours to its \
                      next service. Tabs show the bikes the store owns and has here, the ones \
                      other stores placed here, and its own bikes placed elsewhere.",
            who: "Store managers and staff (`fleet.view`).",
            audience: &[Audience::Manager, Audience::Staff, Audience::Cashier],
            flow: Flow::BackOffice,
            features: &[
                Feature {
                    api: "renox::grid",
                    why: "Columns, filters, sorting, search and paging from one description in \
                          Rust, kept in the address; the status `select` column filters by \
                          status.",
                },
                Feature {
                    api: "Grid::poll",
                    why: "`poll(30)` reloads the board every 30 seconds while the tab is \
                          visible and nobody is filtering, so a bike returned at the counter \
                          or marked overdue by the scheduled task appears by itself.",
                },
                Feature {
                    api: "Grid::cards_on_mobile",
                    why: "On a phone each bike is a card with its frame number, model, status \
                          and location.",
                },
                Feature {
                    api: "Column::badges",
                    why: "Statuses are badges whose tone and word agree (overdue danger, \
                          maintenance warning…).",
                },
                Feature {
                    api: "Column::related",
                    why: "Owner and location are the stores' names, read by subqueries that \
                          sort and filter like the model's own columns.",
                },
                Feature {
                    api: "scopes_with",
                    why: "The grid starts from `access::visible::<RentalBike>(fleet.view)` \
                          (owner **or** location among the person's stores), then the tab \
                          narrows it by owner and location.",
                },
            ],
            under_hood: "One query for the page (with the related store names as \
                         subqueries), one for the total, two for the models' names, three \
                         counts for the tabs.",
            docs: &[
                "docs/grid.md#polling",
                "docs/grid.md#cards-on-phones",
                "docs/grid.md#columns-from-other-tables",
                "docs/grid.md#options-every-column-takes",
                "docs/authorization.md#lists-scopes_with",
            ],
            sources: &[
                FLEET,
                POLICY,
                "examples/bikeshop/resources/views/rentals/fleet.html",
                TESTS,
            ],
        },
        Explanation {
            route: "rentals.fleet.show",
            path: "/staff/fleet/{bike}",
            title: "A rental bike",
            purpose: "One bike: its owner store and where it stands, what each of those \
                      stores may do with it, the hours to its next service, and its history: \
                      rentals, placements at other stores, work orders.",
            who: "Staff of the bike's owner store and of the store where it stands.",
            audience: &[Audience::Manager, Audience::Staff],
            flow: Flow::BackOffice,
            features: &[
                Feature {
                    api: "has_permission_in",
                    why: "ABAC: renting it out, taking it back and repairing it are checked in \
                          the **location** store; its rates, placing it elsewhere and retiring \
                          it in the **owner** store. The page lists each action with the store \
                          it is checked in and whether the person may do it.",
                },
                Feature {
                    api: "relations::belongs_to",
                    why: "Its rentals come with their customers and stores in a fixed number \
                          of queries (`RentalRow::load`).",
                },
                Feature {
                    api: "UI kit: infolist + tabs",
                    why: "The bike's details as an `infolist`, its history in the kit's `tabs`.",
                },
            ],
            under_hood: "`access::find::<RentalBike>` answers 404 to staff of a third store. \
                         Then the stores, the latest 20 rentals with their relations, \
                         placements and work orders.",
            docs: &[
                "docs/authorization.md#checking-one-record-has_permission_in",
                "docs/relations.md#a-page-of-rows-with-their-relations-no-n1",
                "docs/ui.md#infolists-read-only-details",
            ],
            sources: &[
                FLEET,
                POLICY,
                "examples/bikeshop/resources/views/rentals/fleet_show.html",
                TESTS,
            ],
        },
    ]
}

/// GET routes of this area that aren't pages (JSON, files, streams).
pub fn not_pages() -> Vec<NotAPage> {
    vec![
        NotAPage {
            route: "rentals.customers",
            reason: "JSON options for the walk-in form's customer select (renox::select)",
        },
        NotAPage {
            route: "rentals.photo",
            reason: "a redirect to a counter photo's signed temporary URL",
        },
        NotAPage {
            route: "rentals.identities.photo",
            reason: "a redirect to an ID photo's signed temporary URL",
        },
    ]
}
