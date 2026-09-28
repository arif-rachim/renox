use jobs::Order;
use renox::prelude::*;
use renox::testing::TestApp;

async fn app() -> TestApp {
    let app = TestApp::new(jobs::app()).await;
    User::register(app.db(), "Admin", "admin@example.com", "password123")
        .await
        .unwrap();
    app
}

#[renox::test]
async fn an_order_queues_the_receipt_and_notifies_admins() {
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

    // The admin mail went out with the listener; the receipt waits in the queue.
    app.assert_mail_sent("admin@example.com", "New order #1");
    assert_eq!(app.queued_jobs().await, ["send-receipt"]);
    let admin = User::find_by_email(app.db(), "admin@example.com")
        .await
        .unwrap()
        .unwrap();
    let unread = admin.unread_notifications(app.db()).await.unwrap();
    assert_eq!(unread[0].kind, "new-order");
    assert_eq!(unread[0].data["total"], 18000);

    app.run_jobs().await;
    app.assert_mail_sent("buyer@example.com", "Your receipt for order #1");
    let receipt = app
        .sent_mail()
        .into_iter()
        .find(|m| m.is_for("buyer@example.com"))
        .unwrap();
    assert!(receipt.text.contains("Kopi: Rp 18000"), "{}", receipt.text);
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
