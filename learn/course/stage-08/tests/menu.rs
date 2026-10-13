use coffee::app::menu::model::MenuItem;
use renox::prelude::*;
use renox::testing::TestApp;

/// An app with the demo menu in its database.
async fn seeded() -> TestApp {
    let app = TestApp::new(coffee::app()).await;
    app.kernel().seed().await.unwrap();
    app
}

#[renox::test]
async fn the_menu_lists_available_items() {
    let app = seeded().await;
    app.get("/menu")
        .await
        .assert_ok()
        .assert_view("menu/index.html")
        .assert_see("Flat white")
        .assert_see("$4.50");
}

#[renox::test]
async fn an_unavailable_item_is_not_listed() {
    let app = seeded().await;
    app.get("/menu")
        .await
        .assert_ok()
        .assert_dont_see("Cold brew");
}

#[renox::test]
async fn an_item_has_its_own_page() {
    let app = seeded().await;
    let croissant = MenuItem::query()
        .where_(MenuItem::NAME.eq("Croissant"))
        .first(app.db())
        .await
        .unwrap()
        .unwrap();
    app.get(&format!("/menu/{}", croissant.id))
        .await
        .assert_ok()
        .assert_view("menu/show.html")
        .assert_see("Baked this morning")
        .assert_see("Pastry");
}

#[renox::test]
async fn a_missing_item_is_a_404() {
    let app = seeded().await;
    app.get("/menu/999").await.assert_not_found();
}
