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

/// Logs a user in: changing orders needs one.
async fn log_in(app: &TestApp) {
    let user = User::register(app.db(), "Dewi", "dewi@example.com", "password123")
        .await
        .unwrap();
    app.acting_as(&user);
}

#[renox::test]
async fn guests_look_but_dont_change() {
    let app = with_orders(2).await;
    let order = Order::query().first(app.db()).await.unwrap().unwrap();
    let page = app.get("/").await.text();
    assert!(!page.contains("data-edit="), "no editing for guests");
    assert!(!page.contains("data-grid-select-all"), "no bulk actions");
    assert!(!page.contains("data-grid-drag"), "no dragging");
    let url = format!("/orders/{}", order.id);
    for res in [
        app.htmx().patch(&url, &[("customer", "x")]).await,
        app.htmx().delete(&url).await,
        app.htmx()
            .post("/orders/bulk/status/paid", &[("ids", ""), ("all", "true")])
            .await,
        app.htmx()
            .post("/orders/bulk/delete", &[("ids", ""), ("all", "true")])
            .await,
        app.htmx()
            .post("/orders/reorder", &[("ids", ""), ("offset", "0")])
            .await,
    ] {
        // Sent to the login page (htmx: an HX-Redirect), nothing changed.
        assert!(
            res.header("hx-redirect")
                .is_some_and(|to| to.starts_with("/login")),
            "{} {:?}",
            res.status,
            res.header("hx-redirect")
        );
    }
    assert_eq!(Order::query().count(app.db()).await.unwrap(), 2);
    assert_eq!(
        Order::find_or_404(app.db(), order.id)
            .await
            .unwrap()
            .customer,
        order.customer
    );
    // Logged in, the tools are there.
    log_in(&app).await;
    let page = app.get("/").await.text();
    assert!(page.contains("data-edit=") && page.contains("data-grid-select-all"));
}

#[renox::test]
async fn paid_follows_the_status() {
    let app = with_orders(4).await;
    log_in(&app).await;
    app.htmx()
        .post("/orders/bulk/status/paid", &[("ids", ""), ("all", "true")])
        .await
        .assert_status(204);
    assert_eq!(
        Order::where_eq("paid", false)
            .count(app.db())
            .await
            .unwrap(),
        0
    );
    let order = Order::query().first(app.db()).await.unwrap().unwrap();
    app.htmx()
        .patch(&format!("/orders/{}", order.id), &[("status", "new")])
        .await
        .assert_status(204);
    let order = Order::find_or_404(app.db(), order.id).await.unwrap();
    assert!(!order.paid, "a new order isn't paid");
    assert_eq!(order.updated_by, "Dewi");
}

#[renox::test]
async fn the_dashboard_shows_a_page_of_orders() {
    let app = with_orders(60).await;
    let res = app.get("/").await;
    res.assert_ok()
        .assert_see(r#"<form class="rx-grid rx-grid--cards" id="grid-orders""#)
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
    log_in(&app).await;
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
    log_in(&app).await;
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

#[renox::test]
async fn related_columns_and_the_advanced_filter() {
    let app = with_orders(20).await;
    renox::db::sql("INSERT INTO customers (name, tier) VALUES ('Gold One', 'gold')")
        .execute(app.db())
        .await
        .unwrap();
    let first = Order::query()
        .order_by("id")
        .first(app.db())
        .await
        .unwrap()
        .unwrap();
    renox::db::sql("UPDATE orders SET customer_id = 1 WHERE id = ?")
        .bind(first.id)
        .execute(app.db())
        .await
        .unwrap();
    for body in ["a", "b", "c"] {
        renox::db::sql("INSERT INTO order_notes (order_id, body) VALUES (?, ?)")
            .bind(first.id)
            .bind(body)
            .execute(app.db())
            .await
            .unwrap();
    }
    // The order with notes comes first when sorted by them.
    let html = app.get("/?state=1&sort=-notes").await.text();
    let top = html.split("<tr data-id=").nth(1).unwrap();
    assert!(top.starts_with(&format!("\"{}\"", first.id)));
    // Rules on a related column.
    let gold = app
        .get("/?state=1&r.0.c=tier&r.0.o=equals&r.0.v=gold")
        .await
        .text();
    assert_eq!(gold.matches("<tr data-id=").count(), 1);
    assert!(gold.contains("Advanced filter: 1 rule"));
    // Remembered for the next visit.
    assert_eq!(app.get("/").await.text().matches("<tr data-id=").count(), 1);
}

#[renox::test]
async fn two_grids_page_apart_on_one_page() {
    // `/follow-up`: two grids with their own query string prefixes.
    let app = with_orders(40).await;
    let unpaid = Order::where_eq("paid", false)
        .count(app.db())
        .await
        .unwrap();
    let largest = Order::query()
        .order_by_desc("total")
        .first(app.db())
        .await
        .unwrap()
        .unwrap();
    let res = app.get("/follow-up").await;
    res.assert_ok()
        .assert_see(r#"id="grid-unpaid""#)
        .assert_see(r#"id="grid-largest""#)
        .assert_see(&format!("{unpaid} rows"))
        .assert_see("40 rows")
        // `link`: the order number opens the order.
        .assert_see(&format!(r#"href="/orders/{}""#, largest.id));

    // A page of the second grid leaves the first on its first page.
    let res = app.get("/follow-up?largest.page=2").await;
    res.assert_ok().assert_see("11–20 of 40");
    if unpaid > 10 {
        res.assert_see(&format!("1–10 of {unpaid}"));
    }
}

#[renox::test]
async fn the_seeder_fills_the_app_and_can_run_again() {
    let app = TestApp::new(grid::app()).await;
    app.kernel().seed().await.unwrap();
    let seeded = Order::query().count(app.db()).await.unwrap();
    assert!(seeded > 0);
    // A second `db:seed` leaves a seeded database as it is.
    app.kernel().seed().await.unwrap();
    assert_eq!(Order::query().count(app.db()).await.unwrap(), seeded);
}
