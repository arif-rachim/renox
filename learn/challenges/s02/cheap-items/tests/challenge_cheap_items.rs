use coffee::app::menu::model::MenuItem;
use renox::prelude::*;
use renox::testing::TestApp;

#[renox::test]
async fn cheap_items_are_under_three_dollars_cheapest_first() {
    let app = TestApp::new(coffee::app()).await;
    app.kernel().seed().await.unwrap();
    // The demo menu has nothing under $3.00, so add three items.
    for (name, price, available) in [
        ("Drip coffee", 250, true),
        ("Sample cup", 100, true),
        ("Last year's special", 150, false),
    ] {
        let item = MenuItem {
            category_id: 1,
            user_id: 1,
            name: name.into(),
            price,
            available,
            ..Default::default()
        };
        MenuItem::create(app.db(), item).await.unwrap();
    }

    let cheap = MenuItem::cheap(app.db()).await.unwrap();
    let names: Vec<&str> = cheap.iter().map(|item| item.name.as_str()).collect();
    assert_eq!(names, ["Sample cup", "Drip coffee"]);
}
