//! What the shop sells through renox-billing (docs/billing.md): one
//! renox-billing `Plan` per service plan and way of paying, and the
//! gateways.
//!
//! **Plans in code, plans in the table.** renox-billing declares plans in
//! code (`Plan::new(key, label).price(…)`), because a Stripe Price or a
//! Xendit recurring plan can't change under a subscriber. The shop's
//! `service_plans` rows (what the admin panel edits: the description, the
//! parts discount, whether it is sold) come from the same list,
//! `seed::content::SERVICE_PLANS`, so the two always agree on the keys and
//! prices (`tests/plans.rs` checks it).
//!
//! **Keys.** A plan paid by card is the service plan's slug
//! (`monthly-tune-up`): Stripe when `STRIPE_SECRET` is set, else the
//! [demo gateway](super::demo) (local only). The same plan paid through
//! Xendit, in rupiah by card or e-wallet, is `monthly-tune-up-xendit`
//! (`.via("xendit")`), offered when `XENDIT_SECRET_KEY` is set.
//!
//! **Prices.** A service plan's price is per visit; every plan is charged
//! monthly ([`monthly_price`]): a weekly check at 60,000 a visit is 260,000
//! a month. Plan changes take effect from the next period
//! (`without_proration`), and after subscribing, changing or cancelling the
//! customer comes back to their plans (`redirect_to`).

use renox::Config;
use renox_billing::{Billing, Gateway, Interval, Plan, Stripe, Xendit};

use super::demo::DemoGateway;
use super::model::{Frequency, monthly_price};
use crate::seed::content::{SERVICE_PLANS, SERVICE_TASKS};

/// The currency plans are charged in (the shop's prices are rupiah).
pub const CURRENCY: &str = "IDR";

/// How a customer pays for a plan, as the subscribe form offers it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PayWith {
    /// A card: Stripe, or the demo gateway without Stripe's keys.
    Card,
    /// Xendit: a card or an e-wallet, in rupiah.
    Xendit,
}

impl PayWith {
    /// The form's value: `card` or `xendit`.
    pub fn key(self) -> &'static str {
        match self {
            PayWith::Card => "card",
            PayWith::Xendit => "xendit",
        }
    }

    /// The choice for a form value.
    pub fn from_key(key: &str) -> Option<PayWith> {
        match key {
            "card" => Some(PayWith::Card),
            "xendit" => Some(PayWith::Xendit),
            _ => None,
        }
    }

    /// How a subscription made through `gateway` pays (`xendit` or a card).
    pub fn of_gateway(gateway: &str) -> PayWith {
        if gateway == "xendit" {
            PayWith::Xendit
        } else {
            PayWith::Card
        }
    }
}

/// renox-billing's plan key for the service plan `slug` paid `with`.
pub fn billing_key(slug: &str, with: PayWith) -> String {
    match with {
        PayWith::Card => slug.to_owned(),
        PayWith::Xendit => format!("{slug}-xendit"),
    }
}

/// The service plan's slug of a renox-billing plan key.
pub fn slug_of(key: &str) -> &str {
    key.strip_suffix("-xendit").unwrap_or(key)
}

/// The module, registered in `src/lib.rs`: every service plan twice (card
/// and Xendit), Stripe, the demo gateway and Xendit, in that order (a card
/// plan goes to the first one set up: Stripe, else the demo).
pub fn billing() -> Billing {
    let mut billing = Billing::new();
    for (name, slug, frequency, price, description, task_slugs) in SERVICE_PLANS {
        let frequency = frequency.parse::<Frequency>().unwrap_or_default();
        let monthly = monthly_price(*price, frequency);
        let features: Vec<&str> = SERVICE_TASKS
            .iter()
            .filter(|(_, task, _, _)| task_slugs.contains(task))
            .map(|(task, _, _, _)| *task)
            .collect();
        let plan = |key: String, label: String| {
            let mut plan = Plan::new(key, label)
                .price(monthly, CURRENCY, Interval::Month)
                .description(*description);
            for feature in &features {
                plan = plan.feature(*feature);
            }
            plan
        };
        billing = billing
            .plan(plan(billing_key(slug, PayWith::Card), (*name).to_owned()))
            .plan(
                plan(
                    billing_key(slug, PayWith::Xendit),
                    format!("{name} (Xendit)"),
                )
                .via("xendit"),
            );
    }
    billing
        .stripe()
        .gateway(DemoGateway)
        .xendit()
        .without_proration()
        .redirect_to("/plans/mine")
}

/// The ways of paying set up now, with the gateway that takes each: card
/// (`stripe` or `demo`) and `xendit`.
pub fn ways_to_pay(config: &Config) -> Vec<(PayWith, &'static str)> {
    let mut ways = Vec::new();
    if Stripe::from_config().configured(config) {
        ways.push((PayWith::Card, "stripe"));
    } else if DemoGateway.configured(config) {
        ways.push((PayWith::Card, "demo"));
    }
    if Xendit::from_config().configured(config) {
        ways.push((PayWith::Xendit, "xendit"));
    }
    ways
}

/// A new plan's parts discount, in basis points: 5 % for the weekly
/// check, 10 % for fortnightly and monthly care, 15 % for the quarterly
/// full service (the seeder's choice; the admin panel changes it).
pub fn default_parts_discount_bp(frequency: Frequency) -> i64 {
    match frequency {
        Frequency::Weekly => 500,
        Frequency::Fortnightly | Frequency::Monthly => 1_000,
        Frequency::Quarterly => 1_500,
    }
}
