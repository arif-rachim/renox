# Billing example

"Inkwell", a small writing app that charges for plans with `renox-billing`: Basic and Pro
through Stripe (Pro with a 14-day trial), and Pro (IDR) through Xendit for customers in
Indonesia. The plans page, the subscription's card on the account page and the webhooks come
from the module; the app declares its plans and guards two pages.

```text
cp .env.example .env    # then the gateways' test keys and Stripe's price ids
cargo run -- migrate
cargo run               # http://127.0.0.1:3000: register, then "See plans"
```

Without keys the gateways are off: the plans page shows, "Start free trial" works (no payment
method needed), and "Subscribe" shows an error toast (the log names the missing keys). With Stripe's test keys,
"Subscribe" opens Stripe Checkout (card `4242 4242 4242 4242`). Webhooks need a public URL:
`stripe listen --forward-to 127.0.0.1:3000/billing/webhooks/stripe` while developing (put the
`whsec_…` it prints in `STRIPE_WEBHOOK_SECRET`).

## What to try

- **Plans.** `/billing` lists the three plans with their prices, trials and features.
- **A free trial.** "Start free trial" on Pro: no card, and `/exports` (Pro only) opens. The
  account page says when the trial ends; subscribing before then keeps the days left.
- **Subscribing.** "Subscribe" goes to the gateway's page; back in the app, the subscription
  arrives by webhook. The home page and `/reports` follow it.
- **Changing plan.** On `/billing`, "Switch to …" (prorated at Stripe).
- **Canceling.** On `/account`, "Cancel subscription": access stays until the end of the
  period ("Canceled: you have access until …"), and "Resume" takes it back (Stripe).
- **A failed payment.** The user gets a mail (`MAIL_MAILER=log` prints it): the app listens to
  `PaymentFailed` in `src/lib.rs`.

## Where things are

| Part | Files |
|---|---|
| The plans, the gateways, the mail on a failed payment | `src/lib.rs` (`billing()`, `payment_failed`) |
| The home page, the pages for subscribers (`require_subscription`, `require_plan`) | `src/pages.rs`, `resources/views/` |
| The gateways' keys and price ids | `.env.example` |
| Tests (a fake Stripe, signed webhooks) | `tests/billing.rs` |
