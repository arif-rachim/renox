//! "About this page" entries for the workshop area's pages (see `crate::explain`).

use crate::explain::{Audience, Explanation, Feature, Flow, NotAPage};

const MOD: &str = "examples/bikeshop/src/app/workshop/mod.rs";
const MODEL: &str = "examples/bikeshop/src/app/workshop/model.rs";
const BIKES: &str = "examples/bikeshop/src/app/workshop/bikes.rs";
const BOOKING: &str = "examples/bikeshop/src/app/workshop/booking.rs";
const CAPACITY: &str = "examples/bikeshop/src/app/workshop/capacity.rs";
const STATUS: &str = "examples/bikeshop/src/app/workshop/status.rs";
const BOARD: &str = "examples/bikeshop/src/app/workshop/board.rs";
const ORDER: &str = "examples/bikeshop/src/app/workshop/order.rs";
const APPROVAL: &str = "examples/bikeshop/src/app/workshop/approval.rs";
const TASKS: &str = "examples/bikeshop/src/app/workshop/tasks.rs";
const NOTIFY: &str = "examples/bikeshop/src/app/rentals/notify.rs";
const TESTS: &str = "examples/bikeshop/tests/workshop.rs";
const BROWSER: &str = "tests/browser/bikeshop-workshop.test.mjs";

/// The explanation of every page in this area.
pub fn entries() -> Vec<Explanation> {
    vec![
        Explanation {
            route: "workshop.bikes",
            path: "/bikes",
            title: "My bikes",
            purpose: "The customer's own bikes, registered by hand (a catalogue model or \
                      free text, a frame number, a size, a photo) or by themselves when \
                      bought in the shop, each with its last service and a button to book \
                      the next one.",
            who: "Customers.",
            audience: &[Audience::Customer],
            flow: Flow::Service,
            features: &[
                Feature {
                    api: "relations::has_many",
                    why: "Each bike's work orders come in one query for all the bikes \
                          (`has_many` keyed by `customer_bike_id`), newest first, so the \
                          \"last service\" line costs nothing per bike.",
                },
                Feature {
                    api: "Upload",
                    why: "The photo is an optional `Upload` field checked with `image()` (by \
                          content) and stored privately; only its owner opens it, through a \
                          signed temporary link.",
                },
                Feature {
                    api: "derive(Validate)",
                    why: "The form's rules are attributes on the struct (`required`, `max`, \
                          `image`), so the handler only runs with clean input.",
                },
                Feature {
                    api: "UI kit: select (searchable) + form_grid",
                    why: "The catalogue's bike models in a searchable select, the fields side \
                          by side from tablet width.",
                },
            ],
            under_hood: "Three queries: the customer, their bikes, the work orders of all of \
                         them; then the catalogue's bike models for the form. Adding a bike \
                         stores the photo, writes the `customer_bikes` row and opens its \
                         history page.",
            docs: &[
                "docs/relations.md#the-loaders",
                "docs/validation.md#derivevalidate",
                "docs/validation.md#uploads",
            ],
            sources: &[
                BIKES,
                MODEL,
                "examples/bikeshop/resources/views/workshop/bikes.html",
                TESTS,
            ],
        },
        Explanation {
            route: "workshop.bikes.show",
            path: "/bikes/{bike}",
            title: "Bike service history",
            purpose: "One of the customer's bikes and everything done to it: each work order \
                      on a timeline with its tasks (ticked when done), the parts fitted and \
                      the mechanic's notes, plus what is open now and what it has cost.",
            who: "The bike's owner (anyone else gets a 404).",
            audience: &[Audience::Customer],
            flow: Flow::Service,
            features: &[
                Feature {
                    api: "UI kit: infolist",
                    why: "The bike's details as labels and values, money and dates formatted, \
                          the frame number copyable.",
                },
                Feature {
                    api: "blocks: history",
                    why: "The kit has no timeline; the example's `history` block draws the work \
                          orders as an ordered list with a dated marker each, its body in \
                          Markdown (the tasks and parts as a list).",
                },
                Feature {
                    api: "relations::has_many",
                    why: "Tasks, parts and notes of every work order come in one query each, \
                          the task names and part names in one more: seven queries however \
                          long the history.",
                },
                Feature {
                    api: "Policy (own rows only)",
                    why: "The bike is looked up by its id **and** the customer, so another \
                          customer's id simply isn't found.",
                },
            ],
            under_hood: "The bike, its work orders, their tasks, the tasks' names, parts, \
                         notes, stores and the parts' names, each one query.",
            docs: &[
                "docs/ui.md#infolists-read-only-details",
                "docs/relations.md#a-page-of-rows-with-their-relations-no-n1",
            ],
            sources: &[
                BIKES,
                "examples/bikeshop/resources/views/workshop/bike.html",
                "examples/bikeshop/resources/views/blocks/history.html",
                TESTS,
            ],
        },
        Explanation {
            route: "workshop.book",
            path: "/service/book",
            title: "Book a service",
            purpose: "Book a bike into a store's workshop: a package (tune-up, overhaul) or \
                      single tasks, a day that still has room, and a note. The estimate \
                      (price and mechanic time) and the days that are full update as the \
                      form changes.",
            who: "Customers with a registered bike.",
            audience: &[Audience::Customer],
            flow: Flow::Service,
            features: &[
                Feature {
                    api: "blocks: date_picker_blocked",
                    why: "The kit's `date_picker` with the full days and the store's closed \
                          weekdays greyed out (Cally's `isDateDisallowed`), so people pick a day \
                          with room; the server checks again.",
                },
                Feature {
                    api: "UI kit: checkbox_list + radio",
                    why: "The tasks as a `checkbox_list` (a `Vec<i64>`, each with its time and \
                          price as a hint), the packages as a `radio`.",
                },
                Feature {
                    api: "Htmx",
                    why: "The form `hx-get`s this page on every change and swaps only the day \
                          picker and the estimate (`hx-select`), since full days depend on the \
                          store and on the minutes chosen.",
                },
                Feature {
                    api: "Valid<T> + after hook",
                    why: "The hook checks the bike is the customer's, that something was \
                          chosen and that the day has room: the first capacity check.",
                },
                Feature {
                    api: "Db::begin_immediate + lock_for_update",
                    why: "The second capacity check, in the transaction that writes the work \
                          order: the store's row is taken first, so two bookings of the last \
                          slot run one after the other and the second is refused.",
                },
                Feature {
                    api: "notify",
                    why: "The confirmation goes out as a mail and an in-app notification, in \
                          the customer's language.",
                },
            ],
            under_hood: "Loading: the customer's bikes, the stores, the tasks, and the period's \
                         work orders of the chosen store (one query, summed per day in Rust) \
                         for the full days. Booking: `Valid<BookForm>` with its `after` hook, \
                         then `capacity::book` (store row locked, the day's minutes summed \
                         again, the work order and its tasks written), the confirmation, the \
                         work order's page. `workshop:reminders` mails the day before.",
            docs: &[
                "docs/validation.md#hooks-prepare-authorize-after",
                "docs/ui.md#form-fields",
                "docs/ui.md#what-htmx-sent-the-htmx-extractor",
                "docs/mail.md#notifications",
            ],
            sources: &[
                BOOKING,
                CAPACITY,
                "examples/bikeshop/resources/views/workshop/book.html",
                "examples/bikeshop/resources/views/blocks/date_picker_blocked.html",
                TESTS,
            ],
        },
        Explanation {
            route: "workshop.service.show",
            path: "/service/{order}",
            title: "My service",
            purpose: "A work order for its customer: a progress line (booked, checked in, in \
                      progress, ready, collected), the tasks ticked as the mechanic does \
                      them, the parts, the notes, extra work and its answer, the total; \
                      reschedule or cancel until 24 hours before, pay online when it's ready.",
            who: "The customer whose bike it is.",
            audience: &[Audience::Customer],
            flow: Flow::Service,
            features: &[
                Feature {
                    api: "notify",
                    why: "Every status change sends a mail and an in-app notification (a \
                          `DatabaseMessage` the bell shows, live through the notification \
                          stream when the layout has it).",
                },
                Feature {
                    api: "renox::webhook",
                    why: "\"Pay\" starts an online payment through the shared payments \
                          contract; the gateway's webhook emits `PaymentSucceeded`, which marks \
                          the work order paid.",
                },
                Feature {
                    api: "UI kit: action_sheet + date_picker",
                    why: "Rescheduling is a small form in a sheet (sent with htmx; a full day \
                          answers 422 inside the sheet).",
                },
                Feature {
                    api: "Schedule::daily_at",
                    why: "`workshop:reminders` (18:00) mails tomorrow's bookings once; a \
                          rescheduled booking is reminded again.",
                },
            ],
            under_hood: "The work order is found through the customer's bikes (a 404 for \
                         anyone else and for fleet repairs); its tasks, parts, notes and extra \
                         work, a query each.",
            docs: &[
                "docs/mail.md#database-notifications",
                "docs/ui.md#actions",
                "docs/scheduling.md#scheduled-tasks",
            ],
            sources: &[
                BOOKING,
                STATUS,
                TASKS,
                "examples/bikeshop/resources/views/workshop/service.html",
                TESTS,
            ],
        },
        Explanation {
            route: "workshop.extra.show",
            path: "/service/approve/{extra}",
            title: "Approve extra work",
            purpose: "The mechanic found more to do: the customer reads what and for how much, \
                      and approves or refuses it from the mail's link, without logging in.",
            who: "The customer, from the link in their mail.",
            audience: &[Audience::Customer, Audience::Visitor],
            flow: Flow::Service,
            features: &[
                Feature {
                    api: "Signed URLs",
                    why: "`state.signed_url` makes the link (HMAC-SHA256 with `APP_KEY`, valid \
                          72 hours); `ValidSignature` refuses one changed by hand or expired with \
                          a 403. The buttons post to the same signed address.",
                },
                Feature {
                    api: "Single use",
                    why: "The answer is recorded once (`status`, `decided_at`); afterwards the \
                          page shows the decision and a second answer is refused (409).",
                },
                Feature {
                    api: "StockMovement::record",
                    why: "Approved parts are taken from the store's stock in a transaction \
                          with a ledger movement (reason `service`), or waited for.",
                },
            ],
            under_hood: "Approving adds the tasks to the work order, takes the parts, adds up \
                         the totals and puts the work back in progress; refusing only moves \
                         it on. The store's mechanics get an in-app notification either way.",
            docs: &["docs/routing.md#signed-urls", "docs/routing.md#csrf"],
            sources: &[
                APPROVAL,
                ORDER,
                "examples/bikeshop/resources/views/workshop/approve.html",
                TESTS,
            ],
        },
        Explanation {
            route: "workshop.board",
            path: "/staff/workshop",
            title: "Workshop board",
            purpose: "The active store's work orders in columns by status (scheduled, \
                      checked in, in progress, waiting for parts, waiting for approval, \
                      ready): bookings, walk-ins, plan visits and fleet repairs together, \
                      labelled by source; filtered by mechanic and day; moved by dragging.",
            who: "Mechanics and store managers (`workorders.view`; moving needs \
                  `workorders.update`).",
            audience: &[Audience::Mechanic, Audience::Manager],
            flow: Flow::Service,
            features: &[
                Feature {
                    api: "blocks: kanban",
                    why: "The kit has no board; the example's `kanban` block moves cards by \
                          mouse, touch or keyboard and sends each move as an htmx POST. The \
                          server checks the step is allowed and the person may work on that \
                          work order in its store, or the card goes back.",
                },
                Feature {
                    api: "renox::context",
                    why: "The board is the active store's; its mechanics (for the filter) are \
                          the people holding `workorders.update` there now.",
                },
                Feature {
                    api: "relations::belongs_to",
                    why: "The cards' bikes, customers, fleet bikes, models and mechanics come \
                          in a fixed number of queries.",
                },
                Feature {
                    api: "Htmx polling",
                    why: "The board reloads itself every minute (`hx-trigger=\"every 60s\"`), \
                          so other mechanics' moves and new bookings show up.",
                },
            ],
            under_hood: "One query for the open work orders (filters applied), then the \
                         cards' relations, then the mechanics. A move: `Valid<Move>`, \
                         `access::require(workorders.update, Operating)`, the step checked \
                         against `status::allowed`, the status saved and the customer told.",
            docs: &[
                "docs/authorization.md#checking-one-record-has_permission_in",
                "docs/relations.md#a-page-of-rows-with-their-relations-no-n1",
                "docs/ui.md#htmx-response-headers",
            ],
            sources: &[
                BOARD,
                STATUS,
                "examples/bikeshop/resources/views/workshop/board.html",
                "examples/bikeshop/resources/views/blocks/kanban.html",
                TESTS,
                BROWSER,
            ],
        },
        Explanation {
            route: "workshop.walkin",
            path: "/staff/workshop/new",
            title: "Walk-in work order",
            purpose: "A work order for someone at the counter: a known customer (searched) or \
                      a new one without an account, the bike, the tasks and the day (today by \
                      default, checked in at once).",
            who: "Staff at the counter and mechanics (`workorders.view`).",
            audience: &[Audience::Staff, Audience::Mechanic, Audience::Cashier],
            flow: Flow::Service,
            features: &[
                Feature {
                    api: "renox::select",
                    why: "The customer is a searchable select answered by the server \
                          (`options_url`, `OptionQuery`), by name, email or phone.",
                },
                Feature {
                    api: "blocks: date_picker_blocked",
                    why: "The store's full days and closed weekdays can't be picked.",
                },
                Feature {
                    api: "Valid<T> + after hook",
                    why: "`required_if` asks for a new customer's name when none was picked; \
                          the hook checks the day's capacity.",
                },
            ],
            under_hood: "The customer is found or created (no user account), the bike \
                         registered to them, and the same `capacity::book` transaction as \
                         online bookings writes the work order.",
            docs: &[
                "docs/ui.md#options-from-the-server",
                "docs/validation.md#every-rule",
            ],
            sources: &[
                BOARD,
                CAPACITY,
                "examples/bikeshop/resources/views/workshop/walk_in.html",
                TESTS,
            ],
        },
        Explanation {
            route: "workshop.order",
            path: "/staff/workshop/{order}",
            title: "Work order",
            purpose: "The bench: the checklist from the tasks, the parts used (found by what \
                      fits the bike and taken from the store's stock, or waited for), notes \
                      and before/after photos, extra work proposed to the customer, who works \
                      on it, the next status, and the payment at the counter when it's ready.",
            who: "Mechanics; cashiers for the payment.",
            audience: &[Audience::Mechanic, Audience::Cashier, Audience::Manager],
            flow: Flow::Service,
            features: &[
                Feature {
                    api: "renox::select",
                    why: "The part select asks the server as the mechanic types: the parts \
                          that **fit** this bike (`part_fits`, a `Pivot`, through \
                          `FITTING_PARTS.ids`), each with the store's stock.",
                },
                Feature {
                    api: "StockMovement::record",
                    why: "A part used is a stock ledger movement (reason `service`, a `Morph` \
                          reference to the work order) and its level in one transaction; out of \
                          stock, the line waits and the order goes to \"waiting for parts\" \
                          (purchasing, #240, sees what is waited for).",
                },
                Feature {
                    api: "Signed URLs",
                    why: "Proposing extra work mails the customer a signed link to approve or \
                          refuse it without logging in.",
                },
                Feature {
                    api: "has_permission_in",
                    why: "Every action checks `workorders.update` (or `orders.sell` for the \
                          payment) in the work order's own store; another store's staff get a \
                          404.",
                },
                Feature {
                    api: "Events and listeners",
                    why: "Collecting emits `WorkOrderClosed` (#245 bills fleet repairs to the \
                          owner store); a collected fleet repair puts the bike back in the \
                          fleet, serviced.",
                },
                Feature {
                    api: "Upload",
                    why: "Before/after photos are private uploads shown through signed links.",
                },
            ],
            under_hood: "The order, its tasks and their names, parts and their names, notes, \
                         extra work, the bike, the customer and the stores. Each action is one \
                         short handler that redirects back with a toast.",
            docs: &[
                "docs/ui.md#options-from-the-server",
                "docs/relations.md#changing-a-many-to-many",
                "docs/relations.md#polymorphic-relations",
                "docs/authorization.md#checking-one-record-has_permission_in",
                "docs/scheduling.md#events",
            ],
            sources: &[
                ORDER,
                STATUS,
                APPROVAL,
                NOTIFY,
                MOD,
                "examples/bikeshop/src/app/stock/model.rs",
                "examples/bikeshop/resources/views/workshop/order.html",
                TESTS,
                BROWSER,
            ],
        },
    ]
}

/// GET routes of this area that aren't pages (JSON, files, streams).
pub fn not_pages() -> Vec<NotAPage> {
    vec![
        NotAPage {
            route: "workshop.bikes.photo",
            reason: "a redirect to the bike photo's signed temporary URL",
        },
        NotAPage {
            route: "workshop.customers",
            reason: "JSON options for the walk-in form's customer select (renox::select)",
        },
        NotAPage {
            route: "workshop.parts",
            reason: "JSON options for the part select: parts that fit the bike (renox::select)",
        },
        NotAPage {
            route: "workshop.order.photo",
            reason: "a redirect to a note photo's signed temporary URL",
        },
    ]
}
