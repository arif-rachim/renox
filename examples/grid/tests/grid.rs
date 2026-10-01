//! The dashboard's grid: filters and pages from the server, the custom
//! cells, and columns remembered per user.

use grid::app::orders::Order;
use renox::prelude::*;
use renox::testing::TestApp;

async fn with_orders(n: usize) -> TestApp {
    let app = TestApp::new(grid::app()).await;
    Order::create_many(app.db(), n).await.unwrap();
    app
}

#[renox::test]
async fn the_dashboard_shows_a_page_of_orders() {
    let app = with_orders(60).await;
    let res = app.get("/").await;
    res.assert_ok()
        .assert_see(r#"<form class="rx-grid" id="grid-orders""#)
        .assert_see("60 rows")
        .assert_see("1–25 of 60")
        // Grouped headings and the custom cells.
        .assert_see(">Customer</th>")
        .assert_see(">Amounts</th>")
        .assert_see(r#"<svg class="rx-spark""#)
        .assert_see(r#"class="progress""#)
        .assert_see(">Open<");
    assert_eq!(res.text().matches("<tr data-id=").count(), 25);
    app.get("/?page=3").await.assert_see("51–60 of 60");
}

#[renox::test]
async fn filters_narrow_the_orders() {
    let app = with_orders(40).await;
    let paid = Order::where_eq("status", "paid")
        .count(app.db())
        .await
        .unwrap();
    let res = app.get("/?in.status=paid&per_page=100").await;
    res.assert_ok().assert_see("filtered");
    assert_eq!(res.text().matches("<tr data-id=").count() as u64, paid);

    let first = Order::query()
        .order_by("id")
        .first(app.db())
        .await
        .unwrap()
        .unwrap();
    app.get(&format!("/?q.number={}&m.number=equals", first.number))
        .await
        .assert_see(&first.customer);
    app.get("/?q.customer=nobody-has-this-name")
        .await
        .assert_see("Nothing matches these filters.");
}

#[renox::test]
async fn an_order_opens_from_its_row() {
    let app = with_orders(1).await;
    let order = Order::query().first(app.db()).await.unwrap().unwrap();
    app.get(&format!("/orders/{}", order.id))
        .await
        .assert_ok()
        .assert_see(&order.number);
    app.get("/orders/999").await.assert_not_found();
}

#[renox::test]
async fn each_user_keeps_their_columns() {
    let app = with_orders(3).await;
    let user = User::register(app.db(), "Dewi", "dewi@example.com", "password123")
        .await
        .unwrap();
    app.acting_as(&user);
    app.post_json(
        "/_renox/grid/orders/prefs",
        &json!({ "compact": ["number", "total"], "left": ["number", "customer"] }),
    )
    .await
    .assert_status(204);
    app.get("/")
        .await
        .assert_see(r#"data-col="customer" data-pin="left""#);
    app.assert_database_count("grid_preferences", 1).await;
}
