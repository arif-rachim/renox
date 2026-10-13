use coffee::app::menu::model::MenuItem;
use renox::prelude::*;
use renox::testing::TestApp;

/// An app with the demo menu in its database.
async fn seeded() -> TestApp {
    let app = TestApp::new(coffee::app()).await;
    app.kernel().seed().await.unwrap();
    app
}

/// The menu item with this name.
async fn item(app: &TestApp, name: &str) -> MenuItem {
    MenuItem::query()
        .where_(MenuItem::NAME.eq(name))
        .first(app.db())
        .await
        .unwrap()
        .unwrap()
}

#[renox::test]
async fn the_price_tag_shows_dollars() {
    let app = seeded().await;
    app.get("/menu")
        .await
        .assert_ok()
        .assert_see("class=\"price-tag price-tag--normal\">$3.75");
}

#[renox::test]
async fn a_sold_out_item_says_so() {
    let app = seeded().await;
    let muffin = item(&app, "Blueberry muffin").await;
    app.get(&format!("/menu/{}", muffin.id))
        .await
        .assert_ok()
        .assert_see("Sold out");
    app.get("/menu").await.assert_see("Sold out");
}

#[renox::test]
async fn few_left_shows_the_count() {
    let app = seeded().await;
    app.get("/menu").await.assert_ok().assert_see("Only 3 left");
}

#[renox::test]
async fn a_counted_item_with_plenty_shows_no_badge() {
    let app = seeded().await;
    let croissant = item(&app, "Croissant").await;
    app.get(&format!("/menu/{}", croissant.id))
        .await
        .assert_ok()
        .assert_dont_see("Only")
        .assert_dont_see("Sold out");
}

#[renox::test]
async fn a_drink_is_priced_by_the_cup() {
    let app = seeded().await;
    let latte = item(&app, "Flat white").await;
    app.get(&format!("/menu/{}", latte.id))
        .await
        .assert_ok()
        .assert_see("price-tag--large\">$4.50 a cup");
}
