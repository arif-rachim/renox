use coffee::app::menu::model::MenuItem;
use renox::prelude::*;
use renox::testing::TestApp;

/// The page of the item with this name, on an app with the demo menu.
async fn item_page(app: &TestApp, name: &str) -> renox::testing::TestResponse {
    let item = MenuItem::query()
        .where_(MenuItem::NAME.eq(name))
        .first(app.db())
        .await
        .unwrap()
        .unwrap();
    app.get(&format!("/menu/{}", item.id)).await
}

#[renox::test]
async fn the_item_page_shows_a_chip_per_category() {
    let app = TestApp::new(coffee::app()).await;
    app.kernel().seed().await.unwrap();

    item_page(&app, "Espresso")
        .await
        .assert_ok()
        .assert_see("rx-badge--info\">Coffee");
    item_page(&app, "Earl Grey")
        .await
        .assert_ok()
        .assert_see("rx-badge--success\">Tea");
    item_page(&app, "Croissant")
        .await
        .assert_ok()
        .assert_see("rx-badge--warning\">Pastry");
}
