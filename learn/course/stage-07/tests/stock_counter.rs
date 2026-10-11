use coffee::app::menu::model::MenuItem;
use coffee::app::menu::stock_counter::StockCounter;
use renox::prelude::*;
use renox::testing::TestApp;

/// An app with the demo menu, and a user found by email.
async fn seeded() -> TestApp {
    let app = TestApp::new(coffee::app()).await;
    app.kernel().seed().await.unwrap();
    app
}

async fn user(app: &TestApp, email: &str) -> User {
    User::find_by_email(app.db(), email).await.unwrap().unwrap()
}

async fn item(app: &TestApp, name: &str) -> MenuItem {
    MenuItem::query()
        .where_(MenuItem::NAME.eq(name))
        .first(app.db())
        .await
        .unwrap()
        .unwrap()
}

#[renox::test]
async fn the_item_page_mounts_the_counter() {
    let app = seeded().await;
    let croissant = item(&app, "Croissant").await;
    app.get(&format!("/menu/{}", croissant.id))
        .await
        .assert_ok()
        .assert_see("data-rx-live=\"stock-counter\"");
}

// [learn:tests.live-sell]
#[renox::test]
async fn the_author_sells_one() {
    let app = seeded().await;
    let croissant = item(&app, "Croissant").await;
    app.acting_as(&user(&app, "bea@example.com").await);
    let mut counter = app.live(StockCounter {
        item_id: croissant.id,
    });
    counter.call("sell").await.assert_ok().assert_see("11 left");
    assert_eq!(item(&app, "Croissant").await.stock, Some(11));
}
// [/learn:tests.live-sell]

#[renox::test]
async fn the_author_restocks() {
    let app = seeded().await;
    let bun = item(&app, "Cinnamon bun").await;
    app.acting_as(&user(&app, "bea@example.com").await);
    let mut counter = app.live(StockCounter { item_id: bun.id });
    counter.call_with("restock", json!([6])).await.assert_ok();
    assert_eq!(item(&app, "Cinnamon bun").await.stock, Some(9));
}

// [learn:tests.live-forbidden]
#[renox::test]
async fn other_users_are_forbidden() {
    let app = seeded().await;
    let croissant = item(&app, "Croissant").await;
    app.acting_as(&user(&app, "sam@example.com").await);
    let mut counter = app.live(StockCounter {
        item_id: croissant.id,
    });
    counter.call("sell").await.assert_status(403);
    assert_eq!(item(&app, "Croissant").await.stock, Some(12));
}

#[renox::test]
async fn guests_are_unauthorized() {
    let app = seeded().await;
    let croissant = item(&app, "Croissant").await;
    let mut counter = app.live(StockCounter {
        item_id: croissant.id,
    });
    counter.call("sell").await.assert_status(401);
}
// [/learn:tests.live-forbidden]
