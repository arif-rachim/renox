use coffee::app::menu::category::Category;
use coffee::app::menu::model::MenuItem;
use renox::prelude::*;
use renox::testing::TestApp;

/// An app with the demo menu, and Bea (who added it) found again.
async fn seeded() -> (TestApp, User) {
    let app = TestApp::new(coffee::app()).await;
    app.kernel().seed().await.unwrap();
    let bea = User::query()
        .where_eq("email", "bea@example.com")
        .first(app.db())
        .await
        .unwrap()
        .unwrap();
    (app, bea)
}

/// The id of the category with this name.
async fn category(app: &TestApp, name: &str) -> String {
    Category::query()
        .where_(Category::NAME.eq(name))
        .first(app.db())
        .await
        .unwrap()
        .unwrap()
        .id
        .to_string()
}

#[renox::test]
async fn guests_are_sent_to_login() {
    let (app, _) = seeded().await;
    app.get("/menu/new").await.assert_redirect("/login");
}

// [learn:menu.store-test]
#[renox::test]
async fn a_valid_item_is_saved_with_its_author() {
    let (app, bea) = seeded().await;
    let pastry = category(&app, "Pastry").await;
    app.acting_as(&bea);
    let response = app
        .post(
            "/menu",
            &[
                ("category_id", pastry.as_str()),
                ("name", "Almond croissant"),
                ("price", "425"),
                ("stock", "6"),
                ("available", "on"),
            ],
        )
        .await;
    let saved = MenuItem::query()
        .where_(MenuItem::NAME.eq("Almond croissant"))
        .first(app.db())
        .await
        .unwrap()
        .unwrap();
    response.assert_redirect(&format!("/menu/{}", saved.id));
    app.assert_database_has(
        "menu_items",
        &[("name", &"Almond croissant"), ("user_id", &bea.id)],
    )
    .await;
    assert_eq!(saved.stock, Some(6));
}
// [/learn:menu.store-test]

// [learn:menu.errors-test]
#[renox::test]
async fn errors_come_back_next_to_the_fields() {
    let (app, bea) = seeded().await;
    let coffee = category(&app, "Coffee").await;
    app.acting_as(&bea);
    app.htmx()
        .post(
            "/menu",
            &[
                ("category_id", coffee.as_str()),
                ("name", ""),
                ("price", ""),
            ],
        )
        .await
        .assert_invalid("name")
        .assert_invalid("price");
}
// [/learn:menu.errors-test]

#[renox::test]
async fn a_name_is_used_once() {
    let (app, bea) = seeded().await;
    let coffee = category(&app, "Coffee").await;
    app.acting_as(&bea);
    app.htmx()
        .post(
            "/menu",
            &[
                ("category_id", coffee.as_str()),
                ("name", "Espresso"),
                ("price", "300"),
            ],
        )
        .await
        .assert_invalid("name");
}

#[renox::test]
async fn an_unknown_category_is_refused() {
    let (app, bea) = seeded().await;
    app.acting_as(&bea);
    app.htmx()
        .post(
            "/menu",
            &[
                ("category_id", "999"),
                ("name", "Mystery"),
                ("price", "300"),
            ],
        )
        .await
        .assert_invalid("category_id");
}
