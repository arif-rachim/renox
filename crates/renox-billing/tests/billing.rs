//! Subscriptions against fake gateways (`TestApp::fake_http`: no network)
//! and signed webhooks: Stripe's checkout, plan changes, cancellations and
//! resumes; Xendit's recurring plans and cycles; trials without a payment
//! method; idempotent and out-of-order webhooks; the route guards; the
//! pages; and accounts deleted with their subscriptions.

use std::time::Duration;

use renox::http::{FakeHttp, FakeResponse};
use renox::prelude::*;
use renox::serde_json::Value;
use renox::testing::{TestApp, TestResponse};
use renox_billing::{
    Billing, Interval, PaymentFailed, PaymentSucceeded, Plan, Stripe, Subscription,
    SubscriptionCanceled, SubscriptionCreated, SubscriptionRoutes, SubscriptionStatus,
    SubscriptionUpdated,
};

const PASSWORD: &str = "a long password 12";
const STRIPE: &str = "https://api.stripe.com/v1";
const XENDIT: &str = "https://api.xendit.co";
const DAY: u64 = 24 * 60 * 60;

/// Pages only subscribers see.
struct Area;

impl Module for Area {
    fn name(&self) -> &'static str {
        "area"
    }

    fn routes(&self) -> Routes {
        Routes::new()
            .get("/reports", || async { "reports" })
            .require_subscription()
            .merge(
                Routes::new()
                    .get("/exports", || async { "exports" })
                    .require_plan(&["pro"]),
            )
            .require_auth()
    }
}

fn billing() -> Billing {
    Billing::new()
        .plan(
            Plan::new("basic", "Basic")
                .price(900, "USD", Interval::Month)
                .price_id("stripe", "price_basic"),
        )
        // Its Stripe price comes from STRIPE_PRICE_PRO.
        .plan(
            Plan::new("pro", "Pro")
                .price(1_900, "USD", Interval::Month)
                .trial_days(14)
                .feature("Unlimited projects"),
        )
        .plan(
            Plan::new("local", "Local")
                .price(99_000, "IDR", Interval::Month)
                .trial_days(7)
                .via("xendit"),
        )
        .plan(
            Plan::new("local-year", "Local yearly")
                .price(990_000, "IDR", Interval::Year)
                .via("xendit"),
        )
        .gateway(Stripe::new("sk_test_123", "whsec_test"))
        .xendit()
}

async fn app_with(billing: Billing) -> TestApp {
    let app = App::new()
        .module(Auth::new().account())
        .module(billing)
        .module(Area);
    TestApp::with_config(app, |config| {
        config
            .vars
            .insert("STRIPE_PRICE_PRO".into(), "price_pro".into());
        config
            .vars
            .insert("XENDIT_SECRET_KEY".into(), "xnd_development_1".into());
        config
            .vars
            .insert("XENDIT_CALLBACK_TOKEN".into(), "callback-token".into());
    })
    .await
}

async fn app() -> TestApp {
    app_with(billing()).await
}

async fn user(app: &TestApp, name: &str) -> User {
    let email = format!("{}@example.com", name.to_lowercase());
    let user = User::register(app.db(), name, &email, PASSWORD)
        .await
        .unwrap();
    app.acting_as(&user);
    user
}

/// Now, as the app sees it (after `travel`).
async fn now(app: &TestApp) -> i64 {
    app.at_travelled_time(async { renox::db::now().timestamp() })
        .await
}

/// Stripe answers a new customer and a checkout session.
fn stripe_checkout(app: &TestApp) -> FakeHttp {
    let http = app.fake_http();
    http.on(
        &format!("POST {STRIPE}/customers"),
        FakeResponse::json(200, json!({ "id": "cus_1", "object": "customer" })),
    );
    http.on(
        &format!("POST {STRIPE}/checkout/sessions"),
        FakeResponse::json(
            200,
            json!({ "id": "cs_1", "url": "https://checkout.stripe.com/c/pay/cs_1" }),
        ),
    );
    http
}

/// The form of a request the fake received, as pairs.
fn form(body: &str) -> Vec<(String, String)> {
    body.split('&')
        .filter(|p| !p.is_empty())
        .map(|pair| {
            let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
            (decode(k), decode(v))
        })
        .collect()
}

fn decode(text: &str) -> String {
    let text = text.replace('+', " ");
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            out.push(u8::from_str_radix(&text[i + 1..i + 3], 16).unwrap());
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).unwrap()
}

fn param(pairs: &[(String, String)], name: &str) -> Option<String> {
    pairs
        .iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.clone())
}

/// A Stripe subscription object.
fn stripe_sub(user: &User, status: &str, price: &str, plan: &str, period_end: i64) -> Value {
    json!({
        "id": "sub_1",
        "object": "subscription",
        "customer": "cus_1",
        "status": status,
        "cancel_at_period_end": false,
        "cancel_at": null,
        "ended_at": null,
        "trial_end": null,
        "metadata": {
            "renox_billable": format!("user:{}", user.id),
            "renox_name": "default",
            "renox_plan": plan,
        },
        "items": { "data": [{
            "id": "si_1",
            "price": { "id": price },
            "current_period_end": period_end,
        }] },
    })
}

/// A signed Stripe webhook.
async fn stripe_webhook(
    app: &TestApp,
    id: &str,
    kind: &str,
    created: i64,
    object: Value,
) -> TestResponse {
    let payload = json!({
        "id": id,
        "object": "event",
        "type": kind,
        "created": created,
        "data": { "object": object },
    })
    .to_string();
    let t = now(app).await;
    let signature = renox::webhook::hmac_sha256_hex("whsec_test", format!("{t}.{payload}"));
    app.request()
        .header("stripe-signature", &format!("t={t},v1={signature}"))
        .post_body("/billing/webhooks/stripe", "application/json", payload)
        .await
}

/// A Xendit webhook with the callback token.
async fn xendit_webhook(app: &TestApp, event: &str, created: &str, data: Value) -> TestResponse {
    let payload = json!({
        "created": created,
        "business_id": "biz_1",
        "event": event,
        "data": data,
        "api_version": "v1",
    })
    .to_string();
    app.request()
        .header("x-callback-token", "callback-token")
        .post_body("/billing/webhooks/xendit", "application/json", payload)
        .await
}

/// Ana, subscribed to Pro through Stripe (active until `period_end`).
async fn subscribed(app: &TestApp) -> (User, i64) {
    let ana = user(app, "Ana").await;
    stripe_checkout(app);
    app.post("/billing/checkout/pro", &[])
        .await
        .assert_status(303);
    let period_end = now(app).await + 30 * DAY as i64;
    stripe_webhook(
        app,
        "evt_created",
        "customer.subscription.created",
        now(app).await,
        stripe_sub(&ana, "active", "price_pro", "pro", period_end),
    )
    .await
    .assert_ok();
    app.run_jobs().await;
    (ana, period_end)
}

async fn latest(app: &TestApp, user: &User) -> Subscription {
    Billing::of(app.state(), user)
        .subscription()
        .await
        .unwrap()
        .expect("a subscription")
}

#[renox::test]
async fn checkout_sends_the_user_to_stripe_with_the_plan_and_its_trial() {
    let app = app().await;
    let ana = user(&app, "Ana").await;
    let http = stripe_checkout(&app);
    let response = app.post("/billing/checkout/pro", &[]).await;
    response.assert_redirect("https://checkout.stripe.com/c/pay/cs_1");

    let sent = http.sent();
    assert_eq!(sent.len(), 2, "a customer, then the session");
    assert_eq!(sent[0].header("authorization"), Some("Bearer sk_test_123"));
    let customer = form(&sent[0].body);
    assert_eq!(param(&customer, "email").unwrap(), "ana@example.com");
    assert_eq!(
        param(&customer, "metadata[renox_billable]").unwrap(),
        format!("user:{}", ana.id)
    );
    let session = form(&sent[1].body);
    assert_eq!(param(&session, "mode").unwrap(), "subscription");
    assert_eq!(param(&session, "customer").unwrap(), "cus_1");
    assert_eq!(
        param(&session, "line_items[0][price]").unwrap(),
        "price_pro"
    );
    assert_eq!(
        param(&session, "subscription_data[metadata][renox_plan]").unwrap(),
        "pro"
    );
    assert!(
        param(&session, "success_url")
            .unwrap()
            .ends_with("/billing/return")
    );
    // A first subscription gets the plan's 14 days.
    let trial_end: i64 = param(&session, "subscription_data[trial_end]")
        .unwrap()
        .parse()
        .unwrap();
    let expected = now(&app).await + 14 * DAY as i64;
    assert!(
        (trial_end - expected).abs() < 60,
        "{trial_end} vs {expected}"
    );
    app.assert_database_has(
        "billing_customers",
        &[("gateway", &"stripe"), ("gateway_id", &"cus_1")],
    )
    .await;

    // The customer is made once; nothing is subscribed until the webhook.
    app.post("/billing/checkout/basic", &[])
        .await
        .assert_status(303);
    let sessions: Vec<_> = http
        .sent()
        .into_iter()
        .filter(|r| r.url.ends_with("/customers"))
        .collect();
    assert_eq!(sessions.len(), 1);
    assert!(!Billing::of(app.state(), &ana).subscribed().await.unwrap());
    // An unknown plan is refused with a message.
    let refused = app.post("/billing/checkout/gold", &[]).await;
    refused.assert_redirect("/billing");
}

#[renox::test]
async fn webhooks_make_and_update_the_subscription_once_each() {
    let app = app().await;
    app.fake_events();
    let (ana, period_end) = subscribed(&app).await;
    let subscription = latest(&app, &ana).await;
    assert_eq!(subscription.plan, "pro");
    assert_eq!(subscription.gateway, "stripe");
    assert_eq!(subscription.gateway_id.as_deref(), Some("sub_1"));
    assert_eq!(subscription.status, SubscriptionStatus::Active);
    assert_eq!(
        subscription.current_period_end.map(|t| t.timestamp()),
        Some(period_end)
    );
    let billing = Billing::of(app.state(), &ana);
    assert!(billing.subscribed().await.unwrap());
    assert!(billing.subscribed_to("pro").await.unwrap());
    assert!(!billing.subscribed_to("basic").await.unwrap());
    assert_eq!(app.emitted::<SubscriptionCreated>().len(), 1);

    // The provider sends the same event again: answered, not processed.
    let again = stripe_webhook(
        &app,
        "evt_created",
        "customer.subscription.created",
        now(&app).await,
        stripe_sub(&ana, "active", "price_pro", "pro", period_end),
    )
    .await;
    again.assert_ok();
    assert_eq!(again.text(), "already received");
    assert_eq!(app.run_jobs().await, 0);
    app.assert_database_count("webhook_calls", 1).await;
    app.assert_database_count("subscriptions", 1).await;

    // Another event saying the same changes nothing and emits nothing.
    stripe_webhook(
        &app,
        "evt_same",
        "customer.subscription.updated",
        now(&app).await,
        stripe_sub(&ana, "active", "price_pro", "pro", period_end),
    )
    .await
    .assert_ok();
    app.run_jobs().await;
    assert!(app.emitted::<SubscriptionUpdated>().is_empty());

    // A plan changed in Stripe's dashboard: the price names the plan.
    stripe_webhook(
        &app,
        "evt_swapped",
        "customer.subscription.updated",
        now(&app).await + 5,
        stripe_sub(&ana, "active", "price_basic", "pro", period_end),
    )
    .await
    .assert_ok();
    app.run_jobs().await;
    assert_eq!(latest(&app, &ana).await.plan, "basic");
    app.assert_emitted::<SubscriptionUpdated>(|e| {
        e.previous_plan == "pro" && e.subscription.plan == "basic"
    });
}

#[renox::test]
async fn an_older_event_arriving_late_changes_nothing() {
    let app = app().await;
    let (ana, period_end) = subscribed(&app).await;
    let t = now(&app).await;
    stripe_webhook(
        &app,
        "evt_past_due",
        "customer.subscription.updated",
        t + 100,
        stripe_sub(&ana, "past_due", "price_pro", "pro", period_end),
    )
    .await
    .assert_ok();
    // Sent before the one above, delivered after it.
    stripe_webhook(
        &app,
        "evt_older",
        "customer.subscription.updated",
        t + 50,
        stripe_sub(&ana, "active", "price_pro", "pro", period_end),
    )
    .await
    .assert_ok();
    app.run_jobs().await;
    let subscription = latest(&app, &ana).await;
    assert_eq!(subscription.status, SubscriptionStatus::PastDue);
    assert!(!subscription.valid(), "past due isn't valid");
}

#[renox::test]
async fn webhooks_with_a_wrong_or_old_signature_are_refused() {
    let app = app().await;
    let payload = json!({ "id": "evt_1", "type": "customer.subscription.created" }).to_string();
    let t = now(&app).await;
    let forged = renox::webhook::hmac_sha256_hex("not-the-secret", format!("{t}.{payload}"));
    app.request()
        .header("stripe-signature", &format!("t={t},v1={forged}"))
        .post_body(
            "/billing/webhooks/stripe",
            "application/json",
            payload.clone(),
        )
        .await
        .assert_unauthorized();
    let old = t - 600;
    let stale = renox::webhook::hmac_sha256_hex("whsec_test", format!("{old}.{payload}"));
    app.request()
        .header("stripe-signature", &format!("t={old},v1={stale}"))
        .post_body(
            "/billing/webhooks/stripe",
            "application/json",
            payload.clone(),
        )
        .await
        .assert_unauthorized();
    // A sent gateway header doesn't pick another gateway.
    app.request()
        .header("x-renox-billing-gateway", "xendit")
        .header("x-callback-token", "callback-token")
        .post_body(
            "/billing/webhooks/stripe",
            "application/json",
            payload.clone(),
        )
        .await
        .assert_unauthorized();
    app.request()
        .header("x-callback-token", "wrong")
        .post_body(
            "/billing/webhooks/xendit",
            "application/json",
            payload.clone(),
        )
        .await
        .assert_unauthorized();
    app.post_body("/billing/webhooks/paypal", "application/json", payload)
        .await
        .assert_not_found();
    app.assert_database_count("webhook_calls", 0).await;
}

#[renox::test]
async fn swapping_plans_is_prorated_at_stripe() {
    let app = app().await;
    let (ana, period_end) = subscribed(&app).await;
    app.fake_events();
    let http = app.fake_http();
    http.on(
        &format!("GET {STRIPE}/subscriptions/sub_1"),
        FakeResponse::json(
            200,
            stripe_sub(&ana, "active", "price_pro", "pro", period_end),
        ),
    );
    http.on(
        &format!("POST {STRIPE}/subscriptions/sub_1"),
        FakeResponse::json(
            200,
            stripe_sub(&ana, "active", "price_basic", "basic", period_end),
        ),
    );
    app.post("/billing/swap/basic", &[])
        .await
        .assert_redirect("/account");
    let update = http
        .sent()
        .into_iter()
        .find(|r| r.method == "POST" && r.url.ends_with("/subscriptions/sub_1"))
        .unwrap();
    let pairs = form(&update.body);
    assert_eq!(param(&pairs, "items[0][id]").unwrap(), "si_1");
    assert_eq!(param(&pairs, "items[0][price]").unwrap(), "price_basic");
    assert_eq!(
        param(&pairs, "proration_behavior").unwrap(),
        "create_prorations"
    );
    assert_eq!(latest(&app, &ana).await.plan, "basic");
    app.assert_emitted::<SubscriptionUpdated>(|e| e.previous_plan == "pro");
    // A plan sold through another gateway can't be swapped to.
    app.post("/billing/swap/local", &[])
        .await
        .assert_redirect("/billing");
    assert_eq!(latest(&app, &ana).await.plan, "basic");
}

#[renox::test]
async fn without_proration_swaps_say_so() {
    let app = app_with(billing().without_proration()).await;
    let (ana, period_end) = subscribed(&app).await;
    let http = app.fake_http();
    http.on(
        &format!("GET {STRIPE}/subscriptions/sub_1"),
        FakeResponse::json(
            200,
            stripe_sub(&ana, "active", "price_pro", "pro", period_end),
        ),
    );
    http.on(
        &format!("POST {STRIPE}/subscriptions/sub_1"),
        FakeResponse::json(
            200,
            stripe_sub(&ana, "active", "price_basic", "basic", period_end),
        ),
    );
    Billing::of(app.state(), &ana).swap("basic").await.unwrap();
    http.assert_sent(|r| r.method == "POST" && r.body.contains("proration_behavior=none"));
}

#[renox::test]
async fn canceling_keeps_access_until_the_period_ends_and_can_be_resumed() {
    let app = app().await;
    let (ana, period_end) = subscribed(&app).await;
    app.fake_events();
    let http = app.fake_http();
    let mut canceling = stripe_sub(&ana, "active", "price_pro", "pro", period_end);
    canceling["cancel_at_period_end"] = json!(true);
    // Stripe answers the cancellation, then the resume.
    http.on(
        &format!("POST {STRIPE}/subscriptions/sub_1"),
        FakeResponse::json(200, canceling),
    )
    .on(
        &format!("POST {STRIPE}/subscriptions/sub_1"),
        FakeResponse::json(
            200,
            stripe_sub(&ana, "active", "price_pro", "pro", period_end),
        ),
    );
    app.post("/billing/cancel", &[])
        .await
        .assert_redirect("/account");
    http.assert_sent(|r| r.body.contains("cancel_at_period_end=true"));
    let subscription = latest(&app, &ana).await;
    assert!(subscription.canceled());
    assert!(subscription.on_grace_period());
    assert!(subscription.valid());
    assert_eq!(
        subscription.ends_at.map(|t| t.timestamp()),
        Some(period_end)
    );
    app.assert_emitted::<SubscriptionCanceled>(|e| e.subscription.id == subscription.id);
    app.get("/reports").await.assert_ok();

    // The account page offers to resume it.
    app.get("/account")
        .await
        .assert_ok()
        .assert_see("Canceled: you have access until")
        .assert_see("Resume");
    // The plans page offers the canceled plan again.
    app.get("/billing")
        .await
        .assert_see("/billing/checkout/pro")
        .assert_dont_see("Your plan");
    app.post("/billing/resume", &[])
        .await
        .assert_redirect("/account");
    http.assert_sent(|r| r.body.contains("cancel_at_period_end=false"));
    let subscription = latest(&app, &ana).await;
    assert!(!subscription.canceled());
    assert!(subscription.valid());
}

#[renox::test]
async fn a_subscription_ends_when_its_period_runs_out() {
    let app = app().await;
    let (ana, period_end) = subscribed(&app).await;
    let mut canceling = stripe_sub(&ana, "active", "price_pro", "pro", period_end);
    canceling["cancel_at_period_end"] = json!(true);
    app.fake_http().on(
        &format!("POST {STRIPE}/subscriptions/sub_1"),
        FakeResponse::json(200, canceling),
    );
    Billing::of(app.state(), &ana).cancel().await.unwrap();

    app.travel(Duration::from_secs(31 * DAY));
    app.acting_as(&ana); // the session ran out too
    let mut ended = stripe_sub(&ana, "canceled", "price_pro", "pro", period_end);
    ended["ended_at"] = json!(period_end);
    stripe_webhook(
        &app,
        "evt_deleted",
        "customer.subscription.deleted",
        now(&app).await,
        ended,
    )
    .await
    .assert_ok();
    app.run_jobs().await;
    let subscription = app.at_travelled_time(latest(&app, &ana)).await;
    assert_eq!(subscription.status, SubscriptionStatus::Canceled);
    assert!(app.at_travelled_time(async { subscription.ended() }).await);
    // The guard sends them to the plans.
    app.get("/reports").await.assert_redirect("/billing");
    app.get("/account")
        .await
        .assert_see("Your Pro subscription has ended.");
}

#[renox::test]
async fn cancel_now_ends_access_at_once() {
    let app = app().await;
    let (ana, period_end) = subscribed(&app).await;
    let mut canceled = stripe_sub(&ana, "canceled", "price_pro", "pro", period_end);
    canceled["ended_at"] = json!(now(&app).await);
    let http = app.fake_http();
    http.on(
        &format!("DELETE {STRIPE}/subscriptions/sub_1"),
        FakeResponse::json(200, canceled),
    );
    let subscription = Billing::of(app.state(), &ana).cancel_now().await.unwrap();
    assert_eq!(subscription.status, SubscriptionStatus::Canceled);
    assert!(!subscription.valid());
    assert!(!Billing::of(app.state(), &ana).subscribed().await.unwrap());
}

#[renox::test]
async fn the_guards_let_subscribers_through() {
    let app = app().await;
    let ben = user(&app, "Ben").await;
    app.get("/reports").await.assert_redirect("/billing");
    app.request()
        .header("accept", "application/json")
        .get("/reports")
        .await
        .assert_status(402);
    app.logout();
    app.get("/reports").await.assert_redirect("/login");

    app.acting_as(&ben);
    let (_ana, _) = subscribed(&app).await; // acting as Ana now
    app.get("/reports").await.assert_ok().assert_see("reports");
    app.get("/exports").await.assert_ok();
    app.acting_as(&ben);
    app.get("/exports").await.assert_redirect("/billing");
}

#[renox::test]
async fn payments_are_announced_and_a_failure_makes_it_past_due() {
    let app = app().await;
    let (ana, period_end) = subscribed(&app).await;
    app.fake_events();
    let invoice = |id: &str, amount: i64| {
        json!({
            "id": id,
            "object": "invoice",
            "customer": "cus_1",
            "amount_paid": amount,
            "amount_due": amount,
            "currency": "usd",
            "parent": { "subscription_details": { "subscription": "sub_1" } },
        })
    };
    stripe_webhook(
        &app,
        "evt_paid",
        "invoice.payment_succeeded",
        now(&app).await,
        invoice("in_1", 1_900),
    )
    .await
    .assert_ok();
    // The trial's $0 invoice says nothing.
    stripe_webhook(
        &app,
        "evt_zero",
        "invoice.payment_succeeded",
        now(&app).await,
        invoice("in_0", 0),
    )
    .await
    .assert_ok();
    stripe_webhook(
        &app,
        "evt_failed",
        "invoice.payment_failed",
        now(&app).await,
        invoice("in_2", 1_900),
    )
    .await
    .assert_ok();
    stripe_webhook(
        &app,
        "evt_past_due",
        "customer.subscription.updated",
        now(&app).await + 1,
        stripe_sub(&ana, "past_due", "price_pro", "pro", period_end),
    )
    .await
    .assert_ok();
    app.run_jobs().await;
    let paid = app.emitted::<PaymentSucceeded>();
    assert_eq!(paid.len(), 1);
    assert_eq!(paid[0].amount, 1_900);
    assert_eq!(paid[0].currency, "USD");
    assert_eq!(paid[0].owner.as_ref().unwrap().id, ana.id);
    assert_eq!(
        paid[0].subscription.as_ref().unwrap().gateway_id.as_deref(),
        Some("sub_1")
    );
    app.assert_emitted::<PaymentFailed>(|e| e.payment_id == "in_2");
    assert!(!Billing::of(app.state(), &ana).subscribed().await.unwrap());
    app.get("/account")
        .await
        .assert_see("Your last payment failed");
}

#[renox::test]
async fn a_trial_without_a_payment_method_carries_over_to_the_checkout() {
    let app = app_with(billing().generic_trials()).await;
    let ana = user(&app, "Ana").await;
    app.fake_events();
    app.get("/billing")
        .await
        .assert_ok()
        .assert_see("Start free trial");
    app.post("/billing/trial/pro", &[])
        .await
        .assert_redirect("/account");
    let billing = Billing::of(app.state(), &ana);
    assert!(billing.on_trial().await.unwrap());
    assert!(billing.subscribed_to("pro").await.unwrap());
    app.assert_emitted::<SubscriptionCreated>(|e| e.subscription.is_generic_trial());
    // One trial per owner.
    app.post("/billing/trial/pro", &[])
        .await
        .assert_redirect("/billing");
    app.assert_database_count("subscriptions", 1).await;
    app.get("/account")
        .await
        .assert_see("Free trial until")
        .assert_see("Subscribe before then");

    // Ten days later they subscribe: the four days left aren't charged.
    app.travel(Duration::from_secs(10 * DAY));
    app.acting_as(&ana);
    let http = stripe_checkout(&app);
    app.post("/billing/checkout/pro", &[])
        .await
        .assert_status(303);
    let session = http
        .sent()
        .into_iter()
        .find(|r| r.url.ends_with("/checkout/sessions"))
        .unwrap();
    let trial_end: i64 = param(&form(&session.body), "subscription_data[trial_end]")
        .unwrap()
        .parse()
        .unwrap();
    let trial = app.at_travelled_time(latest(&app, &ana)).await;
    assert_eq!(Some(trial_end), trial.trial_ends_at.map(|t| t.timestamp()));

    // Stripe's subscription takes over the trial's row.
    let mut trialing = stripe_sub(&ana, "trialing", "price_pro", "pro", trial_end);
    trialing["trial_end"] = json!(trial_end);
    stripe_webhook(
        &app,
        "evt_trial",
        "customer.subscription.created",
        now(&app).await,
        trialing,
    )
    .await
    .assert_ok();
    app.run_jobs().await;
    app.assert_database_count("subscriptions", 1).await;
    let subscription = app.at_travelled_time(latest(&app, &ana)).await;
    assert_eq!(subscription.gateway, "stripe");
    assert_eq!(subscription.status, SubscriptionStatus::Trialing);
    assert!(
        app.at_travelled_time(async { subscription.on_trial() })
            .await
    );
}

#[renox::test]
async fn a_trial_without_a_payment_method_runs_out() {
    let app = app_with(billing().generic_trials()).await;
    let ana = user(&app, "Ana").await;
    Billing::of(app.state(), &ana)
        .start_trial("pro")
        .await
        .unwrap();
    app.get("/reports").await.assert_ok();
    app.travel(Duration::from_secs(15 * DAY));
    app.acting_as(&ana);
    app.get("/reports").await.assert_redirect("/billing");
    assert!(
        !app.at_travelled_time(Billing::of(app.state(), &ana).subscribed())
            .await
            .unwrap()
    );
    // Not offered on a plan without a trial, nor when the module doesn't.
    let other = app_with(billing()).await;
    user(&other, "Ben").await;
    other
        .post("/billing/trial/pro", &[])
        .await
        .assert_not_found();
}

#[renox::test]
async fn xendit_plans_link_a_payment_method_then_charge_by_cycle() {
    let app = app().await;
    let ana = user(&app, "Ana").await;
    app.fake_events();
    let http = app.fake_http();
    http.on(
        &format!("POST {XENDIT}/customers"),
        FakeResponse::json(200, json!({ "id": "cust-1" })),
    );
    http.on(
        &format!("POST {XENDIT}/recurring/plans"),
        FakeResponse::json(
            201,
            json!({
                "id": "repl_1",
                "status": "REQUIRES_ACTION",
                "actions": [{ "action": "AUTH", "url": "https://linking-dev.xendit.co/link/1", "url_type": "WEB", "method": "GET" }],
            }),
        ),
    );
    app.post("/billing/checkout/local", &[])
        .await
        .assert_redirect("https://linking-dev.xendit.co/link/1");
    let sent = http.sent();
    assert!(
        sent[0]
            .header("authorization")
            .unwrap()
            .starts_with("Basic ")
    );
    let plan = sent[1].json();
    assert_eq!(plan["customer_id"], "cust-1");
    assert_eq!(plan["amount"], 99_000);
    assert_eq!(plan["currency"], "IDR");
    assert_eq!(plan["schedule"]["interval"], "MONTH");
    assert_eq!(plan["metadata"]["renox_plan"], "local");
    // A seven-day trial: the first cycle is then, nothing charged now.
    assert!(plan["schedule"]["anchor_date"].is_string());
    assert!(plan.get("immediate_action_type").is_none());
    let trial_end: i64 = plan["metadata"]["renox_trial_ends_at"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();

    let activated = json!({
        "id": "repl_1",
        "customer_id": "cust-1",
        "status": "ACTIVE",
        "currency": "IDR",
        "amount": 99_000,
        "metadata": plan["metadata"],
    });
    xendit_webhook(
        &app,
        "recurring.plan.activated",
        "2026-10-05T10:00:00Z",
        activated.clone(),
    )
    .await
    .assert_ok();
    app.run_jobs().await;
    let subscription = latest(&app, &ana).await;
    assert_eq!(subscription.gateway, "xendit");
    assert_eq!(subscription.status, SubscriptionStatus::Trialing);
    assert_eq!(
        subscription.trial_ends_at.map(|t| t.timestamp()),
        Some(trial_end)
    );
    assert!(subscription.on_trial());

    // The first cycle is paid; the next one is scheduled.
    let cycle = |id: &str, status: &str| {
        json!({
            "id": id,
            "plan_id": "repl_1",
            "customer_id": "cust-1",
            "status": status,
            "amount": 99_000,
            "currency": "IDR",
            "scheduled_timestamp": "2026-12-12T10:00:00Z",
        })
    };
    xendit_webhook(
        &app,
        "recurring.cycle.succeeded",
        "2026-10-12T10:00:00Z",
        cycle("rpcyc_1", "SUCCEEDED"),
    )
    .await
    .assert_ok();
    xendit_webhook(
        &app,
        "recurring.cycle.created",
        "2026-10-12T10:00:01Z",
        cycle("rpcyc_2", "SCHEDULED"),
    )
    .await
    .assert_ok();
    app.run_jobs().await;
    let subscription = latest(&app, &ana).await;
    assert_eq!(subscription.status, SubscriptionStatus::Active);
    assert_eq!(subscription.trial_ends_at, None);
    assert_eq!(
        subscription.current_period_end.map(|t| t.to_rfc3339()),
        Some("2026-12-12T10:00:00+00:00".into())
    );
    app.assert_emitted::<PaymentSucceeded>(|e| e.payment_id == "rpcyc_1" && e.amount == 99_000);

    // Swapping to a yearly plan isn't something Xendit does.
    app.post("/billing/swap/local-year", &[])
        .await
        .assert_redirect("/billing");
    assert_eq!(latest(&app, &ana).await.plan, "local");

    // Canceling stops the charges now and keeps access to the period's end.
    http.on(
        &format!("POST {XENDIT}/recurring/plans/repl_1/deactivate"),
        FakeResponse::json(200, json!({ "id": "repl_1", "status": "INACTIVE" })),
    );
    app.post("/billing/cancel", &[])
        .await
        .assert_redirect("/account");
    let subscription = latest(&app, &ana).await;
    assert!(subscription.on_grace_period());
    assert_eq!(subscription.ends_at, subscription.current_period_end);
    // No resuming at Xendit.
    app.get("/account").await.assert_dont_see(">Resume<");
    let mut inactive = activated;
    inactive["status"] = json!("INACTIVE");
    xendit_webhook(
        &app,
        "recurring.plan.inactivated",
        "2026-10-20T10:00:00Z",
        inactive,
    )
    .await
    .assert_ok();
    app.run_jobs().await;
    let subscription = latest(&app, &ana).await;
    assert_eq!(subscription.status, SubscriptionStatus::Canceled);
    assert!(subscription.on_grace_period(), "the paid period is kept");
}

#[renox::test]
async fn a_failed_xendit_cycle_is_past_due() {
    let app = app().await;
    let ana = user(&app, "Ana").await;
    app.fake_events();
    let metadata = json!({ "renox_billable": format!("user:{}", ana.id), "renox_name": "default", "renox_plan": "local" });
    xendit_webhook(
        &app,
        "recurring.plan.activated",
        "2026-10-05T10:00:00Z",
        json!({ "id": "repl_9", "customer_id": "cust-9", "status": "ACTIVE", "metadata": metadata }),
    )
    .await
    .assert_ok();
    xendit_webhook(
        &app,
        "recurring.cycle.retrying",
        "2026-11-05T10:00:00Z",
        json!({ "id": "rpcyc_9", "plan_id": "repl_9", "status": "RETRYING", "amount": 99_000, "currency": "IDR" }),
    )
    .await
    .assert_ok();
    app.run_jobs().await;
    assert_eq!(latest(&app, &ana).await.status, SubscriptionStatus::PastDue);
    app.assert_emitted::<PaymentFailed>(|e| e.gateway == "xendit" && e.amount == 99_000);
}

#[renox::test]
async fn the_pages_show_the_plans_and_the_subscription() {
    let app = app().await;
    user(&app, "Ana").await;
    app.get("/billing")
        .await
        .assert_ok()
        .assert_see("Choose a plan")
        .assert_see("$19.00 / month")
        .assert_see("Rp 99,000 / month")
        .assert_see("14-day free trial")
        .assert_see("Unlimited projects")
        .assert_see("Subscribe");
    app.get("/account")
        .await
        .assert_ok()
        .assert_see("You&#39;re not subscribed.")
        .assert_see("See plans");
    let app = app_with(billing()).await;
    subscribed(&app).await;
    app.get("/account")
        .await
        .assert_see("Pro · $19.00 / month")
        .assert_see("Renews on")
        .assert_see("Cancel subscription")
        .assert_see("Change plan");
    app.get("/billing")
        .await
        .assert_see("Your plan")
        .assert_see("Switch to Basic");
    // Already subscribed: a second checkout is refused.
    app.post("/billing/checkout/basic", &[])
        .await
        .assert_redirect("/billing");
    app.get("/billing/return").await.assert_redirect("/account");
}

#[renox::test]
async fn deleting_an_account_cancels_its_subscription() {
    let app = app().await;
    let (ana, period_end) = subscribed(&app).await;
    let mut canceled = stripe_sub(&ana, "canceled", "price_pro", "pro", period_end);
    canceled["ended_at"] = json!(now(&app).await);
    let http = app.fake_http();
    http.on(
        &format!("DELETE {STRIPE}/subscriptions/sub_1"),
        FakeResponse::json(200, canceled),
    );
    app.confirm_password();
    app.post("/account", &[("_method", "DELETE"), ("password", PASSWORD)])
        .await
        .assert_status(303);
    http.assert_sent(|r| r.method == "DELETE" && r.url.ends_with("/subscriptions/sub_1"));
    app.assert_database_count("subscriptions", 0).await;
    app.assert_database_count("billing_customers", 0).await;
}
