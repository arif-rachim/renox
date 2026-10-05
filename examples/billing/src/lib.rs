//! Example: subscriptions with `renox-billing` (#155), for "Inkwell", a
//! small writing app that charges for plans:
//!
//! - three plans declared in code: Basic and Pro through Stripe (Pro with a
//!   14-day trial), and Pro (IDR) through Xendit for Indonesian customers;
//! - the module's plans page (`/billing`), its card on `/account` (change
//!   plan, cancel, resume), and free trials without a payment method;
//! - pages for subscribers only: `/reports` for any plan
//!   (`require_subscription`), `/exports` for Pro (`require_plan`);
//! - the home page asks `Billing::of(&state, &user)` what to show;
//! - the module's events: a mail to the user when a payment fails.
//!
//! Made with `rnx new billing` and `rnx make:module pages`; the gateways'
//! keys and Stripe's price ids go in `.env` (see `.env.example`).
//!
//! ```text
//! cargo run -- migrate
//! cargo run               # http://127.0.0.1:3000, register, then "See plans"
//! ```

pub mod pages;

use renox::mail::Mail;
use renox::prelude::*;
use renox_billing::{Billing, Interval, PaymentFailed, Plan};

pub fn app() -> App {
    App::new()
        .embed(renox::embedded!())
        .module(Auth::new().account().redirect_to("/"))
        .module(billing())
        .module(pages::Pages)
        .listen(payment_failed)
}

/// What Inkwell sells. Stripe charges the Prices named by
/// `STRIPE_PRICE_BASIC` and `STRIPE_PRICE_PRO`; Xendit charges the amount
/// here.
pub fn billing() -> Billing {
    Billing::new()
        .plan(
            Plan::new("basic", "Basic")
                .price(900, "USD", Interval::Month)
                .description("For one writer")
                .feature("Unlimited documents")
                .feature("Monthly reports"),
        )
        .plan(
            Plan::new("pro", "Pro")
                .price(1_900, "USD", Interval::Month)
                .trial_days(14)
                .description("For writers who publish")
                .feature("Everything in Basic")
                .feature("Exports to PDF and EPUB"),
        )
        .plan(
            Plan::new("pro-idr", "Pro (IDR)")
                .price(149_000, "IDR", Interval::Month)
                .trial_days(14)
                .description("Pro, paid in rupiah by card or e-wallet")
                .feature("Everything in Pro")
                .via("xendit"),
        )
        .stripe()
        .xendit()
        .generic_trials()
        .redirect_to("/")
}

/// Tells the user when a payment didn't go through.
async fn payment_failed(event: PaymentFailed, state: AppState) -> Result {
    let Some(owner) = event.owner.filter(|o| o.kind == "user") else {
        return Ok(());
    };
    let Some(user) = User::find(&state.db, owner.id).await? else {
        return Ok(());
    };
    let amount = renox::format_money(
        event.amount as f64 / 10f64.powi(renox::currency_decimals(&event.currency) as i32),
        &event.currency,
        None,
        &state.config.locale,
    );
    let mail = Mail::new(
        user.email,
        "Your payment didn't go through",
        format!(
            "Hello {},\n\nWe couldn't take your payment of {amount}. The payment provider \
             will try again in a few days; please check your payment method.\n",
            user.name
        ),
    );
    state.queue_mail(mail).await?;
    Ok(())
}
