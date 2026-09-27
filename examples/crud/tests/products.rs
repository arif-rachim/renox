use crud::Product;
use renox::prelude::*;
use renox::testing::TestApp;

async fn user(app: &TestApp, email: &str) -> User {
    User::register(app.db(), "Test", email, "password123")
        .await
        .unwrap()
}

#[renox::test]
async fn guests_see_products_but_cannot_add_them() {
    let app = TestApp::new(crud::app()).await;
    let owner = user(&app, "owner@example.com").await;
    Product::create(
        app.db(),
        Product {
            name: "Coffee".into(),
            ..Product::for_owner(&owner)
        },
    )
    .await
    .unwrap();

    app.get("/products").await.assert_ok().assert_see("Coffee");
    app.get("/products/new").await.assert_redirect("/login");
}

#[renox::test]
async fn users_create_products() {
    let app = TestApp::new(crud::app()).await;
    let me = user(&app, "me@example.com").await;
    app.acting_as(&me);

    app.post("/products", &[("name", "Tea"), ("price", "9000")])
        .await
        .assert_redirect("/products");
    app.assert_database_has("products", &[("name", &"Tea"), ("user_id", &me.id)])
        .await;
}

#[renox::test]
async fn invalid_products_are_rejected() {
    let app = TestApp::new(crud::app()).await;
    app.acting_as(&user(&app, "me@example.com").await);

    app.htmx()
        .post("/products", &[("name", ""), ("price", "-5")])
        .await
        .assert_invalid("name")
        .assert_invalid("price");
    app.htmx()
        .post("/products", &[("name", "Tea"), ("price", "cheap")])
        .await
        .assert_invalid("price");
    app.assert_database_count("products", 0).await;
}

#[renox::test]
async fn only_the_owner_edits_a_product() {
    let app = TestApp::new(crud::app()).await;
    let owner = user(&app, "owner@example.com").await;
    let product = Product::create(app.db(), Product::for_owner(&owner))
        .await
        .unwrap();
    let edit = format!("/products/{}/edit", product.id);
    let update = format!("/products/{}", product.id);

    app.acting_as(&user(&app, "other@example.com").await);
    app.get(&edit).await.assert_forbidden();
    app.put(&update, &[("name", "Stolen"), ("price", "1")])
        .await
        .assert_forbidden();

    app.acting_as(&owner);
    app.get(&edit).await.assert_ok().assert_see(&product.name);
    app.put(&update, &[("name", "Renamed"), ("price", "1")])
        .await
        .assert_redirect("/products");
    app.assert_database_has("products", &[("id", &product.id), ("name", &"Renamed")])
        .await;
}

#[renox::test]
async fn deleted_products_go_to_the_trash_and_come_back() {
    let app = TestApp::new(crud::app()).await;
    let owner = user(&app, "owner@example.com").await;
    let espresso = Product {
        name: "Espresso".into(),
        ..Product::for_owner(&owner)
    };
    let product = Product::create(app.db(), espresso).await.unwrap();
    app.acting_as(&owner);

    app.delete(&format!("/products/{}", product.id))
        .await
        .assert_redirect("/products");
    app.get("/products").await.assert_dont_see(&product.name);
    app.get("/products/trash").await.assert_see(&product.name);

    app.post(&format!("/products/{}/restore", product.id), &[])
        .await
        .assert_redirect("/products");
    app.get("/products").await.assert_see(&product.name);
}

#[renox::test]
async fn the_list_offers_edit_and_delete_to_the_owner_only() {
    let app = TestApp::new(crud::app()).await;
    let owner = user(&app, "owner@example.com").await;
    let product = Product::create(app.db(), Product::for_owner(&owner))
        .await
        .unwrap();
    let edit_link = format!("/products/{}/edit", product.id);

    app.get("/products").await.assert_dont_see(&edit_link);
    app.acting_as(&user(&app, "other@example.com").await);
    app.get("/products").await.assert_dont_see(&edit_link);
    app.acting_as(&owner);
    app.get("/products")
        .await
        .assert_see(&edit_link)
        .assert_see(r#"name="_method" value="DELETE""#);

    // What the Delete button sends: a POST with `_method=DELETE`.
    app.post(
        &format!("/products/{}", product.id),
        &[("_method", "DELETE")],
    )
    .await
    .assert_redirect("/products");
    app.assert_database_missing(
        "products",
        &[("id", &product.id), ("deleted_at", &None::<DateTime>)],
    )
    .await;
}
