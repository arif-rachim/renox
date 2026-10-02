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

#[renox::test]
async fn cells_are_edited_in_place() {
    let app = with_orders(1).await;
    let order = Order::query().first(app.db()).await.unwrap().unwrap();
    let user = User::register(app.db(), "Sari", "sari@example.com", "password123")
        .await
        .unwrap();
    app.acting_as(&user);
    let url = format!("/orders/{}", order.id);
    app.get("/")
        .await
        .assert_see(&format!(r#"data-edit="{url}""#));
    let res = app
        .htmx()
        .patch(
            &url,
            &[
                ("customer", "Ibu Ani"),
                ("paid", "true"),
                ("tags", "promo"),
                ("tags", "nope"),
            ],
        )
        .await;
    res.assert_status(204);
    assert!(
        res.header("hx-trigger")
            .is_some_and(|t| t.contains("saved")),
        "a toast"
    );
    let saved = Order::find_or_404(app.db(), order.id).await.unwrap();
    assert_eq!(saved.customer, "Ibu Ani");
    assert!(saved.paid);
    assert_eq!(saved.tags.0, ["promo"], "unknown tags dropped");
    assert_eq!(saved.updated_by, "Sari");
    // Invalid values: 422 with the errors, nothing saved.
    app.htmx()
        .patch(&url, &[("items", "5000"), ("status", "lost")])
        .await
        .assert_invalid("items")
        .assert_invalid("status");
    assert_eq!(
        Order::find_or_404(app.db(), order.id).await.unwrap().items,
        saved.items
    );
}

#[renox::test]
async fn rows_are_dragged_into_order() {
    let app = with_orders(3).await;
    let ids: Vec<i64> = Order::query()
        .order_by("id")
        .get(app.db())
        .await
        .unwrap()
        .iter()
        .map(|o| o.id)
        .collect();
    let page = app.get("/?sort=position").await;
    page.assert_ok().assert_dont_see("data-grid-drag disabled");
    let new_order = format!("{},{},{}", ids[2], ids[0], ids[1]);
    app.post(
        "/orders/reorder",
        &[("ids", new_order.as_str()), ("offset", "0")],
    )
    .await
    .assert_status(204);
    let sorted: Vec<i64> = Order::query()
        .order_by("position")
        .get(app.db())
        .await
        .unwrap()
        .iter()
        .map(|o| o.id)
        .collect();
    assert_eq!(sorted, [ids[2], ids[0], ids[1]]);
}

#[renox::test]
async fn regions_share_merged_cells() {
    let app = with_orders(40).await;
    let html = app.get("/regions").await.text();
    assert!(html.contains("rx-grid__merged"));
    assert!(html.contains("data-covered="));
    assert!(html.contains("<template data-grid-details>"));
    assert!(html.contains("items</div>") || html.contains(" items"));
}

#[renox::test]
async fn every_filtered_row_exports() {
    let app = with_orders(60).await;
    let paid = Order::where_eq("status", "paid")
        .count(app.db())
        .await
        .unwrap();
    let csv = app.get("/?in.status=paid&export=csv").await;
    csv.assert_ok();
    // One heading line, then every paid order (not just a page of 25).
    assert_eq!(csv.text().lines().count() as u64, 1 + paid);
    let xlsx = app.get("/regions?export=xlsx").await;
    xlsx.assert_ok();
    assert!(xlsx.body.starts_with(b"PK"));
    app.get("/?export=print")
        .await
        .assert_ok()
        .assert_see("Print or save as PDF")
        .assert_see("60 rows");
}

#[renox::test]
async fn bulk_and_row_actions_change_orders() {
    let app = with_orders(30).await;
    let new = Order::where_eq("status", "new")
        .count(app.db())
        .await
        .unwrap();
    let res = app
        .htmx()
        .post(
            "/orders/bulk/status/shipped?in.status=new",
            &[("ids", ""), ("all", "true")],
        )
        .await;
    res.assert_status(204);
    assert!(
        res.header("hx-trigger")
            .is_some_and(|t| t.contains(&format!("{new} orders marked shipped")))
    );
    assert_eq!(
        Order::where_eq("status", "new")
            .count(app.db())
            .await
            .unwrap(),
        0
    );
    let first = Order::query()
        .order_by("id")
        .first(app.db())
        .await
        .unwrap()
        .unwrap();
    app.post(
        "/orders/bulk/delete",
        &[("ids", first.id.to_string().as_str()), ("all", "false")],
    )
    .await
    .assert_status(204);
    assert_eq!(Order::query().count(app.db()).await.unwrap(), 29);
    let second = Order::query()
        .order_by("id")
        .first(app.db())
        .await
        .unwrap()
        .unwrap();
    app.delete(&format!("/orders/{}", second.id))
        .await
        .assert_status(204);
    assert_eq!(Order::query().count(app.db()).await.unwrap(), 28);
    app.post("/orders/bulk/status/lost", &[("ids", "1")])
        .await
        .assert_not_found();
}

#[renox::test]
async fn totals_and_groups() {
    let app = with_orders(40).await;
    let total: i64 = Order::query().sum(app.db(), "total").await.unwrap();
    let html = app.get("/").await.text();
    let foot = html.split("<tfoot>").nth(1).expect("a footer");
    assert!(
        foot.contains(&renox::format_number(total as f64, 0, "en")),
        "{foot}"
    );
    let grouped = app.get("/?group=region&per_page=100").await.text();
    assert!(grouped.matches("rx-grid__group-row").count() >= 2);
    assert!(grouped.contains("rx-grid__subtotal"));
}
