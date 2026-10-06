//! "About this page" entries for the plans area's pages (see `crate::explain`).

use crate::explain::{Audience, Explanation, Feature, Flow, NotAPage};

const MOD: &str = "examples/bikeshop/src/app/plans/mod.rs";
const MODEL: &str = "examples/bikeshop/src/app/plans/model.rs";
const BILLING: &str = "examples/bikeshop/src/app/plans/billing.rs";
const SUBSCRIBE: &str = "examples/bikeshop/src/app/plans/subscribe.rs";
const MINE: &str = "examples/bikeshop/src/app/plans/mine.rs";
const VISITS: &str = "examples/bikeshop/src/app/plans/visits.rs";
const SYNC: &str = "examples/bikeshop/src/app/plans/sync.rs";
const TASKS: &str = "examples/bikeshop/src/app/plans/tasks.rs";
const DEMO: &str = "examples/bikeshop/src/app/plans/demo.rs";
const MAILS: &str = "examples/bikeshop/src/app/plans/mails.rs";
const CAPACITY: &str = "examples/bikeshop/src/app/workshop/capacity.rs";
const TESTS: &str = "examples/bikeshop/tests/plans.rs";
const BROWSER: &str = "tests/browser/bikeshop-plans.test.mjs";

/// The explanation of every page in this area.
pub fn entries() -> Vec<Explanation> {
    vec![
        Explanation {
            route: "plans.index",
            path: "/plans",
            title: "Service plans",
            purpose: "The service plans side by side: a weekly commuter check, e-bike care \
                      every two weeks, a monthly tune-up and a quarterly full service, each \
                      with its monthly price, how often the bike comes in, the parts discount \
                      it gives, and a table comparing what every plan includes, task by task. \
                      This is where a customer who rides a lot starts a plan.",
            who: "Anyone: visitors compare, customers choose a plan for one of their bikes.",
            audience: &[Audience::Visitor, Audience::Customer],
            flow: Flow::Service,
            features: &[
                Feature {
                    api: "renox_billing::Plan + Interval",
                    why: "Each service plan is a renox-billing `Plan` declared in code \
                          (`Plan::new(\"monthly-tune-up\", …).price(amount, \"IDR\", \
                          Interval::Month)`), twice: paid by card (Stripe) and through Xendit. \
                          Plans live in code because a gateway's price can't change under a \
                          subscriber; the `service_plans` rows the page reads come from the \
                          same list, and a test checks they agree.",
                },
                Feature {
                    api: "money filter",
                    why: "Prices are integers in rupiah (`price` per visit, \
                          `ServicePlan::monthly_price()` a month) and the `money` filter \
                          writes them in the visitor's language with `APP_CURRENCY`.",
                },
                Feature {
                    api: "bikeshop block: compare_plans",
                    why: "The kit has no pricing cards or comparison table, so the shop's \
                          `compare_plans` block draws both: cards on a CSS grid (one column \
                          on a phone), the monthly tune-up highlighted, and a table whose \
                          ticks have screen-reader text.",
                },
                Feature {
                    api: "Pivot",
                    why: "A plan's tasks are a many-to-many (`PLAN_TASKS`, the `plan_tasks` \
                          table); the page reads every pair in one query and every task in \
                          another, so the table costs three queries whatever the number of \
                          plans.",
                },
            ],
            under_hood: "Three queries: the plans sold, the `plan_tasks` pairs, the service \
                         tasks. Nothing is written. Each card links to the subscribe form \
                         with the plan chosen (`/plans/subscribe?plan=…`).",
            docs: &["docs/billing.md#plans", "docs/relations.md#pivot-columns"],
            sources: &[
                SUBSCRIBE,
                BILLING,
                MODEL,
                "examples/bikeshop/resources/views/plans/index.html",
                "examples/bikeshop/resources/views/blocks/compare_plans.html",
                TESTS,
            ],
        },
        Explanation {
            route: "plans.subscribe",
            path: "/plans/subscribe",
            title: "Subscribe a bike to a plan",
            purpose: "The customer chooses one of their bikes, a plan, the home store whose \
                      workshop does the visits, the weekday they prefer and how to pay (a \
                      card, or Xendit for rupiah by card or e-wallet). The summary shows the \
                      monthly price and the first visit's day. Sending it takes them to the \
                      gateway's payment page; the plan starts when the gateway confirms.",
            who: "Logged-in customers with a bike registered (`/bikes`).",
            audience: &[Audience::Customer],
            flow: Flow::Service,
            features: &[
                Feature {
                    api: "Billing::of(…).named(…).checkout(…)",
                    why: "A customer may have a plan on each of their bikes, so each bike \
                          is its own renox-billing subscription of the user, named \
                          `bike-{id}`: `Billing::of(&state, &user).named(\"bike-12\")\
                          .checkout(\"monthly-tune-up\")` makes the gateway's customer, \
                          starts its checkout and answers the page to send the customer to. \
                          The name rides in the gateway's metadata and comes back with \
                          every webhook.",
                },
                Feature {
                    api: "renox-billing gateways: Stripe, Xendit, a Gateway of the shop's own",
                    why: "A card plan goes to Stripe when `STRIPE_SECRET` is set; the Xendit \
                          variant (`.via(\"xendit\")`) when `XENDIT_SECRET_KEY` is. Without \
                          keys, a demo `Gateway` written in the example (`plans/demo.rs`) \
                          stands in: its \"hosted page\" is a signed page of the app, and \
                          its webhook takes the very path a real one does.",
                },
                Feature {
                    api: "Registry::provide",
                    why: "renox-billing gives its settings (plans, gateways) to the whole \
                          app with `Registry::provide`, so code without a request (the \
                          webhook's queue job, the event listeners) finds them; the form \
                          asks the same gateways which ways of paying to offer.",
                },
                Feature {
                    api: "Valid<T> + after hook",
                    why: "The rules check the plan and store exist; the `after` hook checks \
                          what needs the database: the bike is the customer's, it has no \
                          running plan (one plan per bike), the store opens on that weekday, \
                          and that way of paying is set up. Errors show next to the fields \
                          with the input kept.",
                },
                Feature {
                    api: "UI kit: select, radio, toggle_buttons, form_grid, infolist",
                    why: "Kit fields only: the bike and store as selects, the plan as radio \
                          cards, the weekday as toggle buttons, the summary as an infolist, \
                          on a CSS grid that becomes one column on a phone.",
                },
            ],
            under_hood: "On send: the plan is written as **pending** (an abandoned checkout of \
                         the same bike is reused), then renox-billing makes the customer at \
                         the gateway (`billing_customers`) and the checkout; the browser goes \
                         to the gateway. Its webhook (`POST /billing/webhooks/{gateway}`) is \
                         verified, stored once in `webhook_calls` and applied by a queue \
                         worker into `subscriptions`; `SubscriptionCreated` and \
                         `PaymentSucceeded` follow, and the shop's listeners \
                         (`plans/sync.rs`) start the plan, make its first visits and mail \
                         the customer.",
            docs: &[
                "docs/billing.md#checking-a-subscription-in-code",
                "docs/billing.md#webhooks",
                "docs/billing.md#another-payment-provider",
                "docs/validation.md#hooks-prepare-authorize-after",
            ],
            sources: &[
                SUBSCRIBE,
                BILLING,
                DEMO,
                SYNC,
                "examples/bikeshop/resources/views/plans/subscribe.html",
                TESTS,
                BROWSER,
            ],
        },
        Explanation {
            route: "plans.mine",
            path: "/plans/mine",
            title: "My plans",
            purpose: "Every bike's plan: its state (running, paused, on hold after a failed \
                      payment, ending at the period's end), the next visit, the visits done, \
                      the last invoice, and the plans that ended.",
            who: "Logged-in customers.",
            audience: &[Audience::Customer],
            flow: Flow::Account,
            features: &[
                Feature {
                    api: "Query::group_by + select_as",
                    why: "The visits done per plan are one grouped query \
                          (`COUNT(*) … GROUP BY plan_subscription_id`), not a count per row.",
                },
                Feature {
                    api: "Billing::redirect_to",
                    why: "renox-billing sends customers back here after paying, changing or \
                          cancelling (`Billing::new().redirect_to(\"/plans/mine\")`), with \
                          its \"payment is being confirmed\" message as a toast.",
                },
                Feature {
                    api: "UI kit: list + badge + empty",
                    why: "Each plan is a row of the kit's list with a status badge whose word \
                          and colour agree; no plan yet shows the kit's empty state with a \
                          link to the plans.",
                },
            ],
            under_hood: "Six queries whatever the number of plans: the customer, their bikes, \
                         the plans of those bikes, the service plans, the stores, the visits \
                         done (grouped) and the invoices. Nothing is written.",
            docs: &[
                "docs/billing.md#what-your-users-see",
                "docs/relations.md#a-page-of-rows-with-their-relations-no-n1",
            ],
            sources: &[
                MINE,
                MODEL,
                "examples/bikeshop/resources/views/plans/mine.html",
                TESTS,
            ],
        },
        Explanation {
            route: "plans.show",
            path: "/plans/mine/{subscription}",
            title: "A plan and its visits",
            purpose: "One bike's plan: the upcoming visits on a month calendar and as a \
                      list, each with \"Skip\" and \"Move\" (to another day with room), the \
                      past visits (done, skipped, missed), the invoices, and the changes: \
                      another plan from the next period, pause, cancel at the period's end, \
                      resume.",
            who: "The bike's owner (anyone else gets a 404).",
            audience: &[Audience::Customer],
            flow: Flow::Service,
            features: &[
                Feature {
                    api: "Customer::swap / cancel / resume",
                    why: "Money changes go through renox-billing: `swap` (without \
                          proration, so the new price starts with the next invoice), `cancel` \
                          (at the period's end: visits go on until then) and `resume` \
                          (Stripe and the demo; Xendit stops charging at once). The gateway \
                          answers, renox-billing stores it and emits `SubscriptionUpdated`, \
                          which the shop mirrors into the plan.",
                },
                Feature {
                    api: "Customer::subscribed (the require_subscription check)",
                    why: "Skipping or moving a visit needs the plan paid up. renox-billing's \
                          `require_subscription` guard checks a user's `default` \
                          subscription; here each bike has its own, so the visit routes ask \
                          `Billing::of(…).named(\"bike-…\").subscribed()` and send the \
                          customer back with a message while a payment is due, as the guard \
                          would.",
                },
                Feature {
                    api: "Schedule::daily_at",
                    why: "The visits on this page were made by `plans:visits`, a task that \
                          runs every morning at 06:00 and books the next week's visits \
                          (see the area's tasks); missed visits are recorded by the same \
                          task.",
                },
                Feature {
                    api: "workshop::capacity (move_booking)",
                    why: "Moving a visit is the workshop's own rescheduling: one \
                          transaction that takes the store's row, checks the day has room \
                          for the visit's minutes, and moves it, so a plan can't overbook \
                          the workshop.",
                },
                Feature {
                    api: "bikeshop block: month_calendar",
                    why: "The kit has no calendar of events, so the shop's `month_calendar` \
                          block shows the upcoming visits on a month grid that turns into a \
                          list on a phone.",
                },
                Feature {
                    api: "UI kit: action_sheet + confirm + date_picker",
                    why: "\"Move\" opens a sheet with a date picker (a 422 stays in the \
                          sheet), \"Skip\" and \"Cancel\" ask first, from the kit.",
                },
            ],
            under_hood: "Loads the plan, its bike, the service plans, the last 40 visits and \
                         their work orders (one query each), the invoices, and the \
                         renox-billing subscription (`subscriptions`). Skip cancels the \
                         visit's work order (its minutes free up); move runs \
                         `capacity::move_booking`; pause cancels the upcoming work orders and \
                         makes none until resumed. A plan paid through the demo gateway can \
                         be renewed from here, paid or failed, to see the renewal and \
                         payment-failed mails and the hold.",
            docs: &[
                "docs/billing.md#changing-plan-canceling-resuming",
                "docs/billing.md#pages-for-subscribers-only",
                "docs/scheduling.md#scheduled-tasks",
                "docs/testing.md#time",
            ],
            sources: &[
                MINE,
                VISITS,
                TASKS,
                CAPACITY,
                "examples/bikeshop/resources/views/plans/show.html",
                "examples/bikeshop/resources/views/blocks/month_calendar.html",
                TESTS,
            ],
        },
        Explanation {
            route: "plans.mails",
            path: "/plans/mails",
            title: "Plan mails",
            purpose: "Every mail a plan sends, previewed for a made-up subscriber: the plan \
                      started (its first payment), renewed (each payment after), a payment \
                      failed (the plan is on hold until paid), a visit booked, a visit \
                      missed.",
            who: "Anyone reading the example; the mails themselves go to customers.",
            audience: &[Audience::Developer, Audience::Customer],
            flow: Flow::Learn,
            features: &[
                Feature {
                    api: "renox_billing events: PaymentSucceeded / PaymentFailed",
                    why: "The billing mails are sent by the shop's listeners on \
                          renox-billing's events (`App::listen` from the plans module's \
                          `register`): a payment the gateway reported by webhook becomes an \
                          invoice row and one of these mails.",
                },
                Feature {
                    api: "Notification (mail + database)",
                    why: "Each message is a `Notification` sent to both channels: a mail in \
                          the recipient's language and a row the kit's notification bell \
                          shows.",
                },
                Feature {
                    api: "AppState::mail_view_in",
                    why: "The previews render the real mail view in the visitor's language, \
                          without sending anything.",
                },
            ],
            under_hood: "Renders five mails from `mail/plans/notice.html` with made-up values; \
                         no query, nothing sent.",
            docs: &[
                "docs/billing.md#events-and-the-activity-log",
                "docs/mail.md#notifications",
                "docs/mail.md#the-preview-page",
            ],
            sources: &[
                MAILS,
                SYNC,
                "examples/bikeshop/resources/views/mail/plans/notice.html",
                "examples/bikeshop/resources/views/plans/mails.html",
            ],
        },
        Explanation {
            route: "plans.demo",
            path: "/plans/demo-pay/{user}/{name}/{plan}",
            title: "Demo payment page",
            purpose: "Stands in for Stripe's or Xendit's payment page when the shop runs \
                      without their keys, so a plan can be subscribed to end to end on a \
                      laptop: pay, or decline to see a failed payment.",
            who: "A customer subscribing in the demo (never in production).",
            audience: &[Audience::Customer, Audience::Developer],
            flow: Flow::Service,
            features: &[
                Feature {
                    api: "renox_billing::Gateway",
                    why: "The demo is one impl of renox-billing's `Gateway` trait (a \
                          customer, a checkout, a swap, a cancel, a resume, its webhook's \
                          check and reading): the module treats it like Stripe.",
                },
                Feature {
                    api: "ValidSignature",
                    why: "The page's link is a signed URL made by the gateway's `checkout`: \
                          only its holder can pay, and it expires in an hour.",
                },
                Feature {
                    api: "Job (queue) + renox::http",
                    why: "Paying queues `DemoBillingNotify`, which posts the webhook a \
                          gateway would send, signed with an HMAC of a key derived from \
                          `APP_KEY`, to this app's own `/billing/webhooks/demo` through \
                          `state.http`.",
                },
            ],
            under_hood: "GET reads the service plan and the bike. POST queues the webhook \
                         and redirects to renox-billing's `billing.return`, which sends the \
                         customer to their plans; the webhook is verified, stored once and \
                         applied by a queue worker a moment later.",
            docs: &[
                "docs/billing.md#another-payment-provider",
                "docs/routing.md#signed-urls",
                "docs/queue.md#a-job",
            ],
            sources: &[DEMO, "examples/bikeshop/resources/views/plans/demo.html"],
        },
        Explanation {
            route: "billing.plans",
            path: "/billing",
            title: "Billing plans (from renox-billing)",
            purpose: "renox-billing's own plans page, which subscribes a user's `default` \
                      subscription. The shop sells plans per bike, so its template is \
                      replaced by the app (`resources/views/billing/plans.html`) with the \
                      plans and a link to subscribe a bike. renox-billing's \
                      subscription guard sends people here too.",
            who: "Customers who land here from renox-billing's links.",
            audience: &[Audience::Customer],
            flow: Flow::Service,
            features: &[Feature {
                api: "renox-billing views (overridable)",
                why: "A plugin's template is used unless the app has a file of the same \
                      name: `billing/plans.html` and `billing/section.html` (the card on \
                      `/account`) are the shop's own, pointing at the per-bike plans.",
            }],
            under_hood: "renox-billing's handler reads the user's `default` subscription (none \
                         here) and renders the shop's template.",
            docs: &["docs/billing.md#changing-the-pages"],
            sources: &[
                MOD,
                "examples/bikeshop/resources/views/billing/plans.html",
                "examples/bikeshop/resources/views/billing/section.html",
            ],
        },
    ]
}

/// GET routes of this area that aren't pages (JSON, files, streams).
pub fn not_pages() -> Vec<NotAPage> {
    vec![NotAPage {
        route: "billing.return",
        reason: "renox-billing's return from the gateway: a redirect to /plans/mine with a toast",
    }]
}
