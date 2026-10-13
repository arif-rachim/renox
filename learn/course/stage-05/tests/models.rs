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
async fn typed_columns_filter_by_price() {
    let app = seeded().await;
    let cheap = MenuItem::query()
        .where_(MenuItem::PRICE.lt(400))
        .order_by(MenuItem::NAME)
        .get(app.db())
        .await
        .unwrap();
    let names: Vec<&str> = cheap.iter().map(|item| item.name.as_str()).collect();
    assert_eq!(
        names,
        ["Blueberry muffin", "Croissant", "Earl Grey", "Espresso"]
    );
}

#[renox::test]
async fn latest_comes_first() {
    let app = seeded().await;
    let item = MenuItem {
        category_id: 1,
        user_id: 1,
        name: "Affogato".into(),
        price: 600,
        ..Default::default()
    };
    MenuItem::create(app.db(), item).await.unwrap();

    let items = MenuItem::query().latest().get(app.db()).await.unwrap();
    assert_eq!(items[0].name, "Affogato");
    assert_eq!(items.len(), 10);
}

#[renox::test]
async fn find_or_404_answers_not_found() {
    let app = seeded().await;
    let found = MenuItem::find_or_404(app.db(), 1).await.unwrap();
    assert_eq!(found.name, "Espresso");

    let missing = MenuItem::find_or_404(app.db(), 999).await;
    assert!(matches!(missing, Err(Error::NotFound)));
}

#[renox::test]
async fn save_changes_a_row() {
    let app = seeded().await;
    let original = MenuItem::find_or_404(app.db(), 1).await.unwrap();

    let mut item = original.clone();
    item.price = 325;
    assert!(item.save_changes(app.db(), &original).await.unwrap());
    // Nothing differs now, so nothing is written.
    let saved = MenuItem::find_or_404(app.db(), 1).await.unwrap();
    assert_eq!(saved.price, 325);
    assert!(!item.save_changes(app.db(), &saved).await.unwrap());
}

#[renox::test]
async fn the_seeder_runs_twice_without_duplicates() {
    let app = seeded().await;
    app.kernel().seed().await.unwrap();
    assert_eq!(MenuItem::query().count(app.db()).await.unwrap(), 9);
}
