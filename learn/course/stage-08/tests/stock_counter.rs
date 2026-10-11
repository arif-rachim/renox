use coffee::app::menu::category::Category;
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

#[renox::test]
async fn nothing_left_to_sell_is_a_400() {
    let app = seeded().await;
    let muffin = item(&app, "Blueberry muffin").await;
    app.acting_as(&user(&app, "bea@example.com").await);
    let mut counter = app.live(StockCounter { item_id: muffin.id });
    counter.call("sell").await.assert_status(400);
    assert_eq!(item(&app, "Blueberry muffin").await.stock, Some(0));
}

// [learn:tests.live-factory]
#[renox::test]
async fn selling_the_last_one_says_sold_out() {
    let app = seeded().await;
    let bea = user(&app, "bea@example.com").await;
    // The seeded "Coffee" exists and names are unique: a state picks another.
    let category = Category::factory()
        .state(|category| category.name = "Specials".into())
        .create_one(app.db())
        .await
        .unwrap();
    let last = MenuItem::factory()
        .state(move |item| {
            item.category_id = category.id;
            item.user_id = bea.id;
            item.stock = Some(1);
        })
        .create_one(app.db())
        .await
        .unwrap();
    app.acting_as(&user(&app, "bea@example.com").await);
    let mut counter = app.live(StockCounter { item_id: last.id });
    counter.call("sell").await.assert_ok().assert_see("0 left");
}
// [/learn:tests.live-factory]

#[renox::test]
async fn restock_starts_counting_a_drink() {
    let app = seeded().await;
    let espresso = item(&app, "Espresso").await;
    assert_eq!(espresso.stock, None);
    app.acting_as(&user(&app, "bea@example.com").await);
    let mut counter = app.live(StockCounter {
        item_id: espresso.id,
    });
    counter.call_with("restock", json!([6])).await.assert_ok();
    assert_eq!(item(&app, "Espresso").await.stock, Some(6));
}

#[renox::test]
async fn restock_refuses_more_than_24() {
    let app = seeded().await;
    let croissant = item(&app, "Croissant").await;
    app.acting_as(&user(&app, "bea@example.com").await);
    let mut counter = app.live(StockCounter {
        item_id: croissant.id,
    });
    counter
        .call_with("restock", json!([25]))
        .await
        .assert_status(400);
    assert_eq!(item(&app, "Croissant").await.stock, Some(12));
}

#[renox::test]
async fn guests_see_no_buttons() {
    let app = seeded().await;
    let croissant = item(&app, "Croissant").await;
    app.get(&format!("/menu/{}", croissant.id))
        .await
        .assert_ok()
        .assert_see("12 left")
        .assert_dont_see("rx-click=\"sell\"");
}
