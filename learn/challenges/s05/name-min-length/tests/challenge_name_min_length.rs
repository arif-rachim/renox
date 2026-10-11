use coffee::app::menu::category::Category;
use coffee::app::menu::model::MenuItem;
use renox::prelude::*;
use renox::testing::TestApp;

#[renox::test]
async fn a_name_needs_at_least_three_characters() {
    let app = TestApp::new(coffee::app()).await;
    app.kernel().seed().await.unwrap();
    let bea = User::query()
        .where_eq("email", "bea@example.com")
        .first(app.db())
        .await
        .unwrap()
        .unwrap();
    let pastry = Category::query()
        .where_(Category::NAME.eq("Pastry"))
        .first(app.db())
        .await
        .unwrap()
        .unwrap()
        .id
        .to_string();
    app.acting_as(&bea);

    // Two characters: refused, next to the name field, and nothing is saved.
    app.htmx()
        .post(
            "/menu",
            &[
                ("category_id", pastry.as_str()),
                ("name", "Ab"),
                ("price", "300"),
            ],
        )
        .await
        .assert_invalid("name");
    let short = MenuItem::query()
        .where_(MenuItem::NAME.eq("Ab"))
        .count(app.db())
        .await
        .unwrap();
    assert_eq!(short, 0);

    // Five characters: saved.
    app.post(
        "/menu",
        &[
            ("category_id", pastry.as_str()),
            ("name", "Mocha"),
            ("price", "300"),
        ],
    )
    .await;
    app.assert_database_has("menu_items", &[("name", &"Mocha")])
        .await;
}
