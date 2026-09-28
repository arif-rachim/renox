use jobs::{Order, OrderStatus, SendReceipt};
use renox::http::FakeResponse;
use renox::prelude::*;
use renox::testing::TestApp;

/// The payment gateway answers with `response` (tests never reach it).
fn gateway(app: &TestApp, response: FakeResponse) {
    app.fake_http()
        .on("POST */sandbox/gateway/charges", response);
}

async fn app() -> TestApp {
    let app = TestApp::new(jobs::app()).await;
    User::register(app.db(), "Admin", "admin@example.com", "password123")
        .await
        .unwrap();
    app
}

async fn place(app: &TestApp) {
    app.post(
        "/orders",
        &[
            ("customer_email", "buyer@example.com"),
            ("item", "Kopi"),
            ("total", "18000"),
        ],
    )
    .await
    .assert_redirect("/");
}

async fn order(app: &TestApp, id: i64) -> Order {
    Order::find_or_404(app.db(), id).await.unwrap()
}

async fn admin(app: &TestApp) -> User {
    User::find_by_email(app.db(), "admin@example.com")
        .await
        .unwrap()
        .unwrap()
}

#[renox::test]
async fn an_order_notifies_admins() {
    let app = app().await;
    app.post(
        "/orders",
        &[
            ("customer_email", "buyer@example.com"),
            ("item", "Kopi"),
            ("total", "18000"),
        ],
    )
    .await
    .assert_redirect("/");

    // The admin mail went out with the listener; nothing waits until the order is paid.
    app.assert_mail_sent("admin@example.com", "New order #1");
    assert!(app.queued_jobs().await.is_empty());
    let admin = User::find_by_email(app.db(), "admin@example.com")
        .await
        .unwrap()
        .unwrap();
    let unread = admin.unread_notifications(app.db()).await.unwrap();
    assert_eq!(unread[0].kind, "new-order");
    assert_eq!(unread[0].data["total"], 18000);
    assert_eq!(order(&app, 1).await.status, OrderStatus::Unpaid);
    app.get("/")
        .await
        .assert_see("<em>unpaid</em>")
        .assert_see("tok_declined")
        .assert_dont_see("Remind customer");
    app.acting_as(&admin);
    app.get("/")
        .await
        .assert_see("Remind customer")
        .assert_see("Email monthly statements");
}

#[renox::test]
async fn paying_runs_the_chain_in_order() {
    let app = app().await;
    gateway(&app, FakeResponse::json(201, json!({ "id": "ch_1" })));
    place(&app).await;
    app.post("/orders/1/pay", &[("card_token", "tok_visa")])
        .await
        .assert_redirect("/");
    assert_eq!(order(&app, 1).await.status, OrderStatus::Processing);

    // Only the first job is queued; each one queues the next when it succeeds.
    let worker = app.kernel().worker(Vec::new());
    assert_eq!(app.queued_jobs().await, ["charge-payment"]);
    assert!(worker.run_next().await.unwrap());
    assert_eq!(order(&app, 1).await.status, OrderStatus::Paid);
    assert_eq!(app.queued_jobs().await, ["send-receipt"]);
    assert!(worker.run_next().await.unwrap());
    assert_eq!(app.queued_jobs().await, ["notify-warehouse"]);
    assert!(worker.run_next().await.unwrap());
    assert!(app.queued_jobs().await.is_empty());

    let subjects: Vec<String> = app.sent_mail().into_iter().map(|m| m.subject).collect();
    assert_eq!(
        subjects,
        ["New order #1", "Your receipt for order #1", "Pack order #1"]
    );
    let receipt = app
        .sent_mail()
        .into_iter()
        .find(|m| m.is_for("buyer@example.com"))
        .unwrap();
    assert!(receipt.text.contains("Kopi: Rp 18000"), "{}", receipt.text);

    // Pressing "Pay" again doesn't charge twice.
    app.post("/orders/1/pay", &[("card_token", "tok_visa")])
        .await
        .assert_status(409);
}

#[renox::test]
async fn the_card_token_is_encrypted_in_the_queue() {
    let app = app().await;
    gateway(&app, FakeResponse::json(201, json!({ "id": "ch_1" })));
    place(&app).await;
    app.post("/orders/1/pay", &[("card_token", "tok_visa")])
        .await
        .assert_redirect("/");
    let payload: String = renox::db::sql("SELECT payload FROM jobs WHERE job = 'charge-payment'")
        .scalar(app.db())
        .await
        .unwrap();
    assert!(!payload.contains("tok_visa"), "{payload}");
    assert!(!payload.contains("order_id"), "{payload}");
    // The worker opens it with APP_KEY.
    app.run_jobs().await;
    assert_eq!(order(&app, 1).await.status, OrderStatus::Paid);
}

#[renox::test]
async fn a_declined_card_stops_the_chain_and_flags_the_order() {
    let app = app().await;
    gateway(
        &app,
        FakeResponse::json(402, json!({ "error": "card_declined" })),
    );
    place(&app).await;
    app.post("/orders/1/pay", &[("card_token", "tok_declined")])
        .await
        .assert_redirect("/");
    app.run_jobs().await; // a permanent error: no retries

    assert_eq!(order(&app, 1).await.status, OrderStatus::NeedsAttention);
    app.assert_mail_sent("admin@example.com", "Payment for order #1 failed");
    assert!(
        !app.sent_mail()
            .iter()
            .any(|m| m.is_for("buyer@example.com"))
    );
    assert!(
        !app.sent_mail()
            .iter()
            .any(|m| m.subject.starts_with("Pack"))
    );
    let failed = app.state().queue.failed().await.unwrap();
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0].job, "charge-payment");
}

#[renox::test]
async fn the_failed_hook_runs_after_the_last_attempt() {
    let app = app().await;
    gateway(&app, FakeResponse::connection_error());
    place(&app).await;
    app.post("/orders/1/pay", &[("card_token", "tok_unreachable")])
        .await
        .assert_redirect("/");

    for attempt in 1..=3 {
        assert_eq!(
            order(&app, 1).await.status,
            OrderStatus::Processing,
            "still waiting before attempt {attempt}"
        );
        // Skip the wait between attempts (`Job::backoff`).
        renox::db::sql("UPDATE jobs SET available_at = 0")
            .execute(app.db())
            .await
            .unwrap();
        app.run_jobs().await;
    }
    assert_eq!(order(&app, 1).await.status, OrderStatus::NeedsAttention);
    let alert = app
        .sent_mail()
        .into_iter()
        .find(|m| m.subject == "Payment for order #1 failed")
        .unwrap();
    assert!(alert.text.contains("connection refused"), "{}", alert.text);
    assert!(app.queued_jobs().await.is_empty());
}

#[renox::test]
async fn a_reminder_is_queued_once_per_order() {
    let app = app().await;
    place(&app).await;
    // Staff only.
    app.post("/orders/1/remind", &[])
        .await
        .assert_redirect("/login");
    assert!(app.queued_jobs().await.is_empty());

    app.acting_as(&admin(&app).await);
    for _ in 0..2 {
        app.post("/orders/1/remind", &[]).await.assert_redirect("/");
    }
    assert_eq!(app.queued_jobs().await, ["remind-unpaid"]);
    app.run_jobs().await;
    app.assert_mail_sent("buyer@example.com", "Order #1 is waiting for payment");

    // Once it has run, the next reminder is queued again.
    app.post("/orders/1/remind", &[]).await.assert_redirect("/");
    assert_eq!(app.queued_jobs().await, ["remind-unpaid"]);
}

#[renox::test]
async fn statements_go_out_as_a_batch_with_progress() {
    let app = app().await;
    for (email, total) in [
        ("a@example.com", 10_000),
        ("a@example.com", 5_000),
        ("b@example.com", 7_000),
        ("c@example.com", 1_000),
    ] {
        let order = Order {
            customer_email: email.into(),
            item: "Kopi".into(),
            total,
            ..Default::default()
        };
        Order::create(app.db(), order).await.unwrap();
    }
    app.acting_as(&admin(&app).await);
    app.post("/statements", &[])
        .await
        .assert_redirect("/statements/1");
    app.get("/statements/1")
        .await
        .assert_ok()
        .assert_see("0 of 3 sent")
        .assert_see("every 1s");

    app.run_jobs().await;
    let batch = app.state().queue.batch_status(1).await.unwrap().unwrap();
    assert_eq!(batch.progress(), 100);
    assert!(batch.finished);
    assert_eq!(batch.failed, 0);
    // What htmx gets when it polls: the block alone, and no more polling.
    app.htmx()
        .get("/statements/1")
        .await
        .assert_see("3 of 3 sent. Done.")
        .assert_dont_see("every 1s")
        .assert_dont_see("<nav>");

    let statement = app
        .sent_mail()
        .into_iter()
        .find(|m| m.is_for("a@example.com"))
        .unwrap();
    assert!(
        statement
            .text
            .contains("2 order(s) in the last 30 days, Rp 15000"),
        "{}",
        statement.text
    );
    // The batch's `then` job ran after the last statement.
    app.assert_mail_sent("admin@example.com", "Monthly statements sent");
    assert_eq!(
        app.sent_mail().last().unwrap().subject,
        "Monthly statements sent"
    );
}

#[renox::test]
async fn receipts_jump_ahead_of_reports() {
    let app = app().await;
    let order = Order {
        customer_email: "buyer@example.com".into(),
        item: "Kopi".into(),
        total: 18_000,
        ..Default::default()
    };
    let order = Order::create(app.db(), order).await.unwrap();
    // The report's mails are queued first, on `default`…
    jobs::daily_sales(app.state().clone()).await.unwrap();
    // …then the receipt, on `high` (`SendReceipt::QUEUE`).
    app.state()
        .dispatch(SendReceipt { order_id: order.id })
        .await
        .unwrap();

    // What `queue:work --queue high,default` does.
    let worker = app.kernel().worker(vec!["high".into(), "default".into()]);
    assert!(worker.run_next().await.unwrap());
    assert_eq!(app.sent_mail()[0].subject, "Your receipt for order #1");
    assert!(worker.run_next().await.unwrap());
    assert_eq!(app.sent_mail()[1].subject, "Today's sales");
}

#[renox::test]
async fn invalid_orders_are_not_placed() {
    let app = app().await;
    app.htmx()
        .post(
            "/orders",
            &[("customer_email", "nope"), ("item", ""), ("total", "0")],
        )
        .await
        .assert_invalid("customer_email")
        .assert_invalid("item")
        .assert_invalid("total");
    assert!(app.queued_jobs().await.is_empty());
}

#[renox::test]
async fn the_daily_report_sums_todays_orders() {
    let app = app().await;
    for total in [18_000, 9_000] {
        let order = Order {
            customer_email: "b@example.com".into(),
            item: "Kopi".into(),
            total,
            ..Default::default()
        };
        Order::create(app.db(), order).await.unwrap();
    }
    // An order from last week is left out of today's report.
    let old = Order {
        customer_email: "b@example.com".into(),
        item: "Teh".into(),
        total: 5_000,
        created_at: Some(renox::db::now() - renox::chrono::TimeDelta::days(3)),
        ..Default::default()
    };
    Order::create(app.db(), old).await.unwrap();
    jobs::daily_sales(app.state().clone()).await.unwrap(); // what the scheduler runs at 21:00
    app.run_jobs().await;
    let report = app
        .sent_mail()
        .into_iter()
        .find(|m| m.subject == "Today's sales")
        .unwrap();
    assert!(
        report.text.contains("2 order(s) today, Rp 27000"),
        "{}",
        report.text
    );
}

#[renox::test]
async fn the_weekly_report_covers_seven_days() {
    let app = app().await;
    for (total, days_ago) in [(18_000, 0), (5_000, 3), (1_000, 10)] {
        let order = Order {
            customer_email: "b@example.com".into(),
            item: "Kopi".into(),
            total,
            created_at: Some(renox::db::now() - renox::chrono::TimeDelta::days(days_ago)),
            ..Default::default()
        };
        Order::create(app.db(), order).await.unwrap();
    }
    // Runs it by name, as `my-app schedule:run weekly-sales` does: this also
    // checks that the task is registered.
    app.kernel().run_scheduled("weekly-sales").await.unwrap();
    app.run_jobs().await;
    let report = app
        .sent_mail()
        .into_iter()
        .find(|m| m.subject == "This week's sales")
        .unwrap();
    assert!(
        report
            .text
            .contains("2 order(s) in the last 7 days, Rp 23000"),
        "{}",
        report.text
    );
}

#[renox::test]
async fn a_report_already_running_is_not_sent_twice() {
    let app = app().await;
    let lock = app
        .state()
        .cache
        .lock("sales-report:1", std::time::Duration::from_secs(60));
    let held = lock.try_acquire().await.unwrap().unwrap(); // another run holds it
    app.kernel().run_scheduled("daily-sales").await.unwrap();
    assert!(app.queued_jobs().await.is_empty());

    held.release().await.unwrap();
    app.kernel().run_scheduled("daily-sales").await.unwrap();
    assert_eq!(app.queued_jobs().await, ["renox.send-mail"]);
    assert!(!lock.is_held().await.unwrap(), "released after the run");
}

#[renox::test]
async fn a_failed_report_alerts_someone() {
    let app = app().await;
    renox::db::sql("DROP TABLE orders")
        .execute(app.db())
        .await
        .unwrap();
    assert!(app.kernel().run_scheduled("daily-sales").await.is_err());
    app.run_jobs().await;
    app.assert_mail_sent("admin@example.com", "A sales report failed");
}

#[renox::test]
async fn the_charge_request_carries_the_amount_and_an_idempotency_key() {
    let app = app().await;
    gateway(&app, FakeResponse::status(503)); // then…
    gateway(&app, FakeResponse::json(201, json!({ "id": "ch_1" })));
    place(&app).await;
    app.post("/orders/1/pay", &[("card_token", "tok_visa")])
        .await
        .assert_redirect("/");
    app.run_jobs().await; // 503: retried later
    assert_eq!(order(&app, 1).await.status, OrderStatus::Processing);
    app.run_all_jobs().await; // the retry too, without waiting for its backoff
    assert_eq!(order(&app, 1).await.status, OrderStatus::Paid);

    let http = app.fake_http();
    http.assert_sent_count(2);
    let charge = &http.sent()[1];
    assert_eq!(charge.method, "POST");
    assert_eq!(charge.json()["amount"], 18000);
    assert_eq!(charge.json()["source"], "tok_visa");
    assert_eq!(charge.header("idempotency-key"), Some("order-1"));
    assert!(
        charge
            .header("authorization")
            .unwrap()
            .starts_with("Basic ")
    );
}

#[renox::test]
async fn the_sandbox_gateway_answers_like_a_provider() {
    let app = app().await;
    let charge = |token: &'static str| json!({ "amount": 5000, "source": token });
    app.post_json("/sandbox/gateway/charges", &charge("tok_visa"))
        .await
        .assert_status(201);
    app.post_json("/sandbox/gateway/charges", &charge("tok_declined"))
        .await
        .assert_status(402);
    app.post_json("/sandbox/gateway/charges", &charge("tok_unreachable"))
        .await
        .assert_status(503);
}
