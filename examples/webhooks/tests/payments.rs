//! Each provider: a correctly signed call pays the order, a forged one is
//! refused, and a repeated one is processed once.

use renox::prelude::*;
use renox::testing::TestApp;
use renox::webhook;
use webhooks::Order;

async fn app() -> TestApp {
    let app = TestApp::with_config(webhooks::app(), |c| {
        // Secrets as they'd be in .env.
        c.vars
            .insert("MIDTRANS_SERVER_KEY".into(), "SB-Mid-server-test".into());
        c.vars
            .insert("XENDIT_CALLBACK_TOKEN".into(), "xnd-token-test".into());
        c.vars
            .insert("STRIPE_WEBHOOK_SECRET".into(), "whsec_test".into());
    })
    .await;
    Order::create(app.db(), Order::new("INV-1", 4_999))
        .await
        .unwrap();
    app
}

async fn status(app: &TestApp) -> (String, Option<String>) {
    let order = Order::where_eq("code", "INV-1")
        .first(app.db())
        .await
        .unwrap()
        .unwrap();
    (order.status, order.paid_via)
}

// Midtrans charges rupiah only, so its `gross_amount` is IDR.
fn midtrans(transaction_status: &str, server_key: &str) -> String {
    let signature = webhook::sha512_hex(format!("INV-1200150000.00{server_key}"));
    format!(
        r#"{{"order_id":"INV-1","status_code":"200","gross_amount":"150000.00","signature_key":"{signature}","transaction_id":"tx-9","transaction_status":"{transaction_status}","fraud_status":"accept"}}"#
    )
}

#[renox::test]
async fn midtrans_settlement_pays_the_order() {
    let app = app().await;
    let forged = midtrans("settlement", "guessed-key");
    app.post_body("/webhooks/midtrans", "application/json", forged)
        .await
        .assert_status(401);

    let pending = midtrans("pending", "SB-Mid-server-test");
    let settled = midtrans("settlement", "SB-Mid-server-test");
    for body in [&pending, &settled, &settled] {
        app.post_body("/webhooks/midtrans", "application/json", body.as_str())
            .await
            .assert_ok();
    }
    app.assert_database_count("webhook_calls", 2).await; // pending + settlement, once each
    app.run_jobs().await;
    assert_eq!(status(&app).await, ("paid".into(), Some("midtrans".into())));
}

#[renox::test]
async fn xendit_paid_invoice_pays_the_order() {
    let app = app().await;
    let invoice = r#"{"id":"inv_123","external_id":"INV-1","status":"PAID"}"#;
    app.request()
        .without_csrf()
        .header("x-callback-token", "wrong")
        .post_body("/webhooks/xendit", "application/json", invoice)
        .await
        .assert_status(401);
    // Xendit retries until it gets a 2xx: the repeat is stored once.
    for _ in 0..2 {
        app.request()
            .without_csrf()
            .header("x-callback-token", "xnd-token-test")
            .post_body("/webhooks/xendit", "application/json", invoice)
            .await
            .assert_ok();
    }
    app.assert_database_count("webhook_calls", 1).await;
    app.run_jobs().await;
    assert_eq!(status(&app).await, ("paid".into(), Some("xendit".into())));
}

#[renox::test]
async fn stripe_completed_checkout_pays_the_order() {
    let app = app().await;
    let event = r#"{"id":"evt_1","type":"checkout.session.completed","data":{"object":{"client_reference_id":"INV-1","payment_status":"paid"}}}"#;
    let signed = |secret: &str, at: i64| {
        let v1 = webhook::hmac_sha256_hex(secret, format!("{at}.{event}"));
        format!("t={at},v1={v1}")
    };
    let now = renox::db::now().timestamp();
    let send = |header: String| {
        let app = &app;
        async move {
            app.request()
                .without_csrf()
                .header("stripe-signature", &header)
                .post_body("/webhooks/stripe", "application/json", event)
                .await
        }
    };
    send(signed("whsec_other", now)).await.assert_status(401);
    send(signed("whsec_test", now - 3600))
        .await
        .assert_status(401); // replayed
    send(signed("whsec_test", now)).await.assert_ok();
    send(signed("whsec_test", now)).await.assert_ok(); // repeated: once
    app.assert_database_count("webhook_calls", 1).await;
    app.run_jobs().await;
    assert_eq!(status(&app).await, ("paid".into(), Some("stripe".into())));
}

#[renox::test]
async fn the_order_page_shows_payment_status() {
    let app = app().await;
    app.get("/")
        .await
        .assert_ok()
        .assert_see(r#"<table class="rx-table">"#)
        .assert_see("<strong>INV-1</strong>")
        // The amount (cents) through the `money` filter, in APP_CURRENCY.
        .assert_see("$49.99")
        .assert_see(r#"<span class="rx-badge rx-badge--warning">Pending</span>"#);

    // Paid via Xendit: the badge and the provider follow.
    app.request()
        .without_csrf()
        .header("x-callback-token", "xnd-token-test")
        .post_body(
            "/webhooks/xendit",
            "application/json",
            r#"{"id":"inv_1","external_id":"INV-1","status":"PAID"}"#,
        )
        .await
        .assert_ok();
    app.run_jobs().await;
    app.get("/")
        .await
        .assert_see(r#"<span class="rx-badge rx-badge--success">Paid</span>"#)
        .assert_see(">Xendit<");
}

#[renox::test]
async fn the_order_page_says_when_there_are_no_orders() {
    let app = TestApp::new(webhooks::app()).await;
    app.get("/")
        .await
        .assert_ok()
        .assert_see("No orders yet")
        .assert_dont_see("rx-table");
}

#[renox::test]
async fn a_missing_secret_refuses_every_call() {
    // No secrets in the config: nothing can be verified, so every call is
    // refused like a forged one and nothing is stored.
    let app = TestApp::new(webhooks::app()).await;
    let invoice = r#"{"id":"inv_1","external_id":"INV-1","status":"PAID"}"#;
    let res = app
        .request()
        .without_csrf()
        .header("x-callback-token", "anything")
        .post_body("/webhooks/xendit", "application/json", invoice)
        .await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED);
    app.assert_database_count("webhook_calls", 0).await;
}

#[renox::test]
async fn a_payment_for_an_unknown_order_is_logged_and_kept() {
    let app = app().await;
    let invoice = r#"{"id":"inv_9","external_id":"NOPE-9","status":"PAID"}"#;
    app.request()
        .without_csrf()
        .header("x-callback-token", "xnd-token-test")
        .post_body("/webhooks/xendit", "application/json", invoice)
        .await
        .assert_ok();
    app.run_jobs().await;
    // Handled without error (the call is kept for a look); INV-1 untouched.
    app.assert_database_count("webhook_calls", 1).await;
    assert_eq!(status(&app).await, ("pending".into(), None));
}

#[renox::test]
async fn the_seeder_fills_the_app_and_can_run_again() {
    let app = TestApp::new(webhooks::app()).await;
    app.kernel().seed().await.unwrap();
    let seeded = Order::query().count(app.db()).await.unwrap();
    assert!(seeded > 0);
    // A second `db:seed` leaves a seeded database as it is.
    app.kernel().seed().await.unwrap();
    assert_eq!(Order::query().count(app.db()).await.unwrap(), seeded);
}
