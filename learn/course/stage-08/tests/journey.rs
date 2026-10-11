use coffee::app::menu::category::Category;
use coffee::app::menu::model::MenuItem;
use coffee::app::menu::stock_counter::StockCounter;
use renox::prelude::*;
use renox::testing::TestApp;

// [learn:tests.journey]
/// The whole trip, as one person makes it: sign up, add an item through the
/// form, open its page and sell one from the counter.
#[renox::test]
async fn a_barista_adds_an_item_and_sells_it() {
    let app = TestApp::new(coffee::app()).await;
    let category = Category::factory().create_one(app.db()).await.unwrap();

    app.post(
        "/register",
        &[
            ("name", "Ana"),
            ("email", "ana@example.com"),
            ("password", "secret123"),
            ("password_confirmation", "secret123"),
        ],
    )
    .await
    .assert_redirect("/");

    let category_id = category.id.to_string();
    app.post(
        "/menu",
        &[
            ("category_id", category_id.as_str()),
            ("name", "Oat cookie"),
            ("price", "275"),
            ("stock", "6"),
            ("available", "on"),
        ],
    )
    .await;
    let cookie = MenuItem::query()
        .where_(MenuItem::NAME.eq("Oat cookie"))
        .first(app.db())
        .await
        .unwrap()
        .unwrap();
    app.get(&format!("/menu/{}", cookie.id))
        .await
        .assert_ok()
        .assert_see("6 left");

    let mut counter = app.live(StockCounter { item_id: cookie.id });
    let sold = counter.call("sell").await;
    sold.assert_ok().assert_see("5 left");
    // The toast travels in the HX-Trigger header, for the page to show.
    sold.assert_header(
        "hx-trigger",
        r#"{"renox:toast":{"toasts":[{"kind":"success","message":"Sold one Oat cookie."}]}}"#,
    );

    let cookie = MenuItem::find_or_404(app.db(), cookie.id).await.unwrap();
    assert_eq!(cookie.stock, Some(5));
}
// [/learn:tests.journey]
