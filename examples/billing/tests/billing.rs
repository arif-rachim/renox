//! Inkwell's subscriptions against a fake Stripe (no network): the plans
//! page, a checkout, the signed webhook that makes the subscription, the
//! pages for subscribers, and the mail after a failed payment.

use renox::http::FakeResponse;
use renox::prelude::*;
use renox::serde_json::Value;
use renox::testing::{TestApp, TestResponse};
use renox_billing::Billing;

const STRIPE: &str = "https://api.stripe.com/v1";

async fn app() -> TestApp {
    TestApp::with_config(billing::app(), |config| {
        for (name, value) in [
            ("STRIPE_SECRET", "sk_test_1"),
            ("STRIPE_WEBHOOK_SECRET", "whsec_test"),
            ("STRIPE_PRICE_BASIC", "price_basic"),
            ("STRIPE_PRICE_PRO", "price_pro"),
        ] {
            config.vars.insert(name.into(), value.into());
        }
    })
    .await
}

async fn ana(app: &TestApp) -> User {
    let user = User::register(app.db(), "Ana", "ana@example.com", "a long password 12")
        .await
        .unwrap();
    app.acting_as(&user);
    user
}

/// A Stripe webhook signed with the test secret.
async fn stripe(app: &TestApp, id: &str, kind: &str, object: Value) -> TestResponse {
    let now = renox::db::now().timestamp();
    let payload =
        json!({ "id": id, "type": kind, "created": now, "data": { "object": object } }).to_string();
    let signature = renox::webhook::hmac_sha256_hex("whsec_test", format!("{now}.{payload}"));
    app.request()
        .header("stripe-signature", &format!("t={now},v1={signature}"))
        .post_body("/billing/webhooks/stripe", "application/json", payload)
        .await
}

/// Ana subscribes to `price`'s plan.
async fn subscribe(app: &TestApp, user: &User, plan: &str, price: &str) {
    let http = app.fake_http();
    http.on(
        &format!("POST {STRIPE}/customers"),
        FakeResponse::json(200, json!({ "id": "cus_1" })),
    );
    http.on(
        &format!("POST {STRIPE}/checkout/sessions"),
        FakeResponse::json(
            200,
            json!({ "id": "cs_1", "url": "https://checkout.stripe.com/c/pay/cs_1" }),
        ),
    );
    app.post(&format!("/billing/checkout/{plan}"), &[])
        .await
        .assert_redirect("https://checkout.stripe.com/c/pay/cs_1");
    stripe(
        app,
        "evt_1",
        "customer.subscription.created",
        json!({
            "id": "sub_1",
            "customer": "cus_1",
            "status": "active",
            "metadata": { "renox_billable": format!("user:{}", user.id), "renox_plan": plan },
            "items": { "data": [{ "id": "si_1", "price": { "id": price } }] },
        }),
    )
    .await
    .assert_ok();
    app.run_jobs().await;
}

#[renox::test]
async fn the_plans_page_lists_the_three_plans() {
    let app = app().await;
    ana(&app).await;
    app.get("/").await.assert_ok().assert_see("See plans");
    app.get("/billing")
        .await
        .assert_ok()
        .assert_see("$9.00 / month")
        .assert_see("$19.00 / month")
        .assert_see("Rp 149,000 / month")
        .assert_see("Start free trial");
    // Not subscribed: the pages for subscribers lead to the plans.
    app.get("/reports").await.assert_redirect("/billing");
}

#[renox::test]
async fn basic_opens_the_reports_and_pro_the_exports() {
    let app = app().await;
    let user = ana(&app).await;
    subscribe(&app, &user, "basic", "price_basic").await;
    assert!(
        Billing::of(app.state(), &user)
            .subscribed_to("basic")
            .await
            .unwrap()
    );
    app.get("/")
        .await
        .assert_see("You're on the <strong>basic</strong> plan.");
    app.get("/reports")
        .await
        .assert_ok()
        .assert_see("Words written");
    app.get("/exports").await.assert_redirect("/billing");
    app.get("/account")
        .await
        .assert_see("Basic · $9.00 / month")
        .assert_see("Cancel subscription");
}

#[renox::test]
async fn a_free_trial_opens_the_exports_without_a_card() {
    let app = app().await;
    ana(&app).await;
    app.post("/billing/trial/pro", &[])
        .await
        .assert_redirect("/");
    app.get("/exports")
        .await
        .assert_ok()
        .assert_see("ready to export");
}

#[renox::test]
async fn a_failed_payment_mails_the_user() {
    let app = app().await;
    let user = ana(&app).await;
    subscribe(&app, &user, "pro", "price_pro").await;
    stripe(
        &app,
        "evt_2",
        "invoice.payment_failed",
        json!({
            "id": "in_1",
            "customer": "cus_1",
            "subscription": "sub_1",
            "amount_due": 1_900,
            "currency": "usd",
        }),
    )
    .await
    .assert_ok();
    app.run_all_jobs().await;
    app.assert_mail_sent("ana@example.com", "Your payment didn't go through");
    let mail = app.sent_mail().pop().unwrap();
    assert!(mail.text.contains("$19.00"), "{}", mail.text);
}
