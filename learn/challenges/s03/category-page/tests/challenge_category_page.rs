use coffee::app::menu::category::Category;
use renox::prelude::*;
use renox::testing::TestApp;

#[renox::test]
async fn a_category_lists_its_available_items() {
    let app = TestApp::new(coffee::app()).await;
    app.kernel().seed().await.unwrap();
    let coffee = Category::query()
        .where_(Category::NAME.eq("Coffee"))
        .first(app.db())
        .await
        .unwrap()
        .unwrap();

    app.get(&format!("/categories/{}", coffee.id))
        .await
        .assert_ok()
        .assert_view("menu/category.html")
        .assert_see("Espresso")
        .assert_see("Flat white")
        // Another category, and an item that is not available today.
        .assert_dont_see("Croissant")
        .assert_dont_see("Cold brew");

    app.get("/categories/999").await.assert_not_found();
}
