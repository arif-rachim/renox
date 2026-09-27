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
    Order::create(app.db(), Order::new("INV-1", 150_000))
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
    app.request()
        .without_csrf()
        .header("x-callback-token", "xnd-token-test")
        .post_body("/webhooks/xendit", "application/json", invoice)
        .await
        .assert_ok();
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
    app.run_jobs().await;
    assert_eq!(status(&app).await, ("paid".into(), Some("stripe".into())));
}

#[renox::test]
async fn the_order_page_shows_payment_status() {
    let app = app().await;
    app.get("/")
        .await
        .assert_ok()
        .assert_see("INV-1")
        .assert_see("pending");
}
