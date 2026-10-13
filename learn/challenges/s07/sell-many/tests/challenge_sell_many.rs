use coffee::app::menu::model::MenuItem;
use coffee::app::menu::stock_counter::StockCounter;
use renox::prelude::*;
use renox::testing::TestApp;

async fn user(app: &TestApp, email: &str) -> User {
    User::find_by_email(app.db(), email).await.unwrap().unwrap()
}

async fn croissant(app: &TestApp) -> MenuItem {
    MenuItem::query()
        .where_(MenuItem::NAME.eq("Croissant"))
        .first(app.db())
        .await
        .unwrap()
        .unwrap()
}

#[renox::test]
async fn the_author_sells_several_at_once() {
    let app = TestApp::new(coffee::app()).await;
    app.kernel().seed().await.unwrap();
    let item = croissant(&app).await;
    assert_eq!(item.stock, Some(12));

    app.acting_as(&user(&app, "bea@example.com").await);
    let mut counter = app.live(StockCounter { item_id: item.id });
    counter
        .call_with("sell_many", json!([5]))
        .await
        .assert_ok()
        .assert_see("7 left");
    assert_eq!(croissant(&app).await.stock, Some(7));

    // Below 1, or more than what is left: 400, and nothing changes.
    counter
        .call_with("sell_many", json!([0]))
        .await
        .assert_status(400);
    counter
        .call_with("sell_many", json!([8]))
        .await
        .assert_status(400);
    assert_eq!(croissant(&app).await.stock, Some(7));
}

#[renox::test]
async fn another_user_is_forbidden() {
    let app = TestApp::new(coffee::app()).await;
    app.kernel().seed().await.unwrap();
    let item = croissant(&app).await;
    app.acting_as(&user(&app, "sam@example.com").await);
    let mut counter = app.live(StockCounter { item_id: item.id });
    counter
        .call_with("sell_many", json!([2]))
        .await
        .assert_status(403);
    assert_eq!(croissant(&app).await.stock, Some(12));
}

#[renox::test]
async fn a_guest_is_unauthorized() {
    let app = TestApp::new(coffee::app()).await;
    app.kernel().seed().await.unwrap();
    let item = croissant(&app).await;
    let mut counter = app.live(StockCounter { item_id: item.id });
    counter
        .call_with("sell_many", json!([2]))
        .await
        .assert_status(401);
}
