//! The generated panel on the seeded shop: who gets in, what each role may
//! do, the lists, forms, actions, exports and the trash.

use admin::resources::{Category, Customer, Product};
use renox::prelude::*;
use renox::testing::TestApp;

/// The app, seeded (roles, the two staff, the catalog, customers).
async fn app() -> TestApp {
    let app = TestApp::new(admin::app()).await;
    admin::seed::run(app.state().clone()).await.unwrap();
    app
}

async fn login(app: &TestApp, email: &str) -> User {
    let user = User::find_by_email(app.db(), email).await.unwrap().unwrap();
    app.acting_as(&user);
    user
}

async fn product(app: &TestApp, sku: &str) -> Product {
    Product::where_eq("sku", sku)
        .with_trashed()
        .first(app.db())
        .await
        .unwrap()
        .unwrap()
}

#[renox::test]
async fn the_panel_is_for_staff_only() {
    let app = app().await;
    app.get("/").await.assert_redirect("/admin");
    app.get("/admin").await.assert_redirect("/login");
    // A user without a role is refused.
    let guest = User::register(app.db(), "Sam", "sam@example.com", "password123")
        .await
        .unwrap();
    app.acting_as(&guest);
    app.get("/admin").await.assert_forbidden();

    login(&app, "editor@example.com").await;
    app.get("/admin")
        .await
        .assert_ok()
        .assert_see("Corner Shop")
        .assert_see("Catalog")
        .assert_see(r#"href="/admin/products""#)
        .assert_see(r#"href="/admin/customers""#);
}

#[renox::test]
async fn the_products_list_searches_filters_and_draws_the_stock() {
    let app = app().await;
    login(&app, "editor@example.com").await;
    app.get("/admin/products")
        .await
        .assert_ok()
        .assert_see("Arabica coffee 250 g")
        // The category comes from its own table.
        .assert_see("Coffee")
        // The app's own cells for the custom column.
        .assert_see(r#"<span class="rx-badge rx-badge--error">Out</span>"#)
        .assert_see("New product")
        .assert_see("/admin/products/actions/publish")
        // Editors may not delete, so no bulk delete and no trash.
        .assert_dont_see("/admin/products/actions/delete")
        .assert_dont_see("filter=trashed");
    app.get("/admin/products?search=syrup")
        .await
        .assert_see("Vanilla syrup")
        .assert_dont_see("Arabica");
    app.get("/admin/products?filter=low")
        .await
        .assert_see("Decaf coffee")
        .assert_dont_see("Arabica");
}

#[renox::test]
async fn an_editor_adds_and_changes_products_but_deletes_nothing() {
    let app = app().await;
    login(&app, "editor@example.com").await;
    let coffee = Category::where_eq("name", "Coffee")
        .first(app.db())
        .await
        .unwrap()
        .unwrap();

    app.get("/admin/products/create")
        .await
        .assert_ok()
        .assert_see(">Coffee</option>");
    // A SKU another product has is refused, in the form (htmx).
    let taken = app
        .htmx()
        .post(
            "/admin/products",
            &[
                ("name", "Cold brew 1 L"),
                ("sku", "cof-arb"),
                ("price", "11.99"),
                ("stock", "10"),
                ("status", "live"),
            ],
        )
        .await;
    taken.assert_status(422);
    assert!(taken.json_path("errors.sku").is_array());

    app.htmx()
        .post(
            "/admin/products",
            &[
                ("name", "Cold brew 1 L"),
                ("sku", "cof-cold"),
                ("category_id", &coffee.id.to_string()),
                ("price", "11.99"),
                ("stock", "10"),
                ("status", "live"),
                ("featured", "on"),
                ("released_on", "2026-10-01"),
            ],
        )
        .await
        .assert_hx_redirect("/admin/products");
    let cold = product(&app, "COF-COLD").await;
    assert_eq!(cold.category_id, Some(coffee.id));
    assert!(cold.featured);

    // Its own SKU stays valid on the edit page.
    let url = format!("/admin/products/{}", cold.id);
    app.htmx()
        .put(
            &url,
            &[
                ("name", "Cold brew 1 L"),
                ("sku", "COF-COLD"),
                ("price", "12.99"),
                ("stock", "8"),
                ("status", "live"),
            ],
        )
        .await
        .assert_hx_redirect("/admin/products");
    assert_eq!(product(&app, "COF-COLD").await.price, 1_299);

    app.get(&url)
        .await
        .assert_ok()
        .assert_see("$12.99")
        .assert_dont_see("Delete this product?");
    app.htmx().delete(&url).await.assert_forbidden();
    // Customers: editors only look.
    app.get("/admin/customers")
        .await
        .assert_ok()
        .assert_dont_see("New customer");
    app.get("/admin/customers/create").await.assert_forbidden();
}

#[renox::test]
async fn an_admin_deletes_to_the_trash_and_restores() {
    let app = app().await;
    login(&app, "admin@example.com").await;
    let arabica = product(&app, "COF-ARB").await;
    let robusta = product(&app, "COF-ROB").await;

    let ids = format!("{},{}", arabica.id, robusta.id);
    app.htmx()
        .post("/admin/products/actions/delete", &[("ids", &ids)])
        .await
        .assert_status(204);
    assert!(product(&app, "COF-ARB").await.deleted_at.is_some());
    app.get("/admin/products?filter=trashed")
        .await
        .assert_see("Arabica coffee 250 g")
        .assert_dont_see("Jasmine tea");

    app.htmx()
        .post(
            &format!("/admin/products/{}/actions/restore", arabica.id),
            &[],
        )
        .await
        .assert_status(204);
    assert!(product(&app, "COF-ARB").await.deleted_at.is_none());
    assert!(product(&app, "COF-ROB").await.deleted_at.is_some());
}

#[renox::test]
async fn actions_publish_and_feature_the_selection() {
    let app = app().await;
    login(&app, "editor@example.com").await;
    let decaf = product(&app, "COF-DEC").await;
    assert_eq!(decaf.status, "draft");
    app.htmx()
        .post(
            &format!("/admin/products/{}/actions/publish", decaf.id),
            &[],
        )
        .await
        .assert_status(204);
    assert_eq!(product(&app, "COF-DEC").await.status, "live");

    // "Select all matching" with the Live tab: every live product.
    app.htmx()
        .post(
            "/admin/products/actions/feature?filter=live",
            &[("ids", ""), ("all", "true")],
        )
        .await
        .assert_status(204);
    let not_featured = Product::where_eq("status", "live")
        .where_eq("featured", false)
        .count(app.db())
        .await
        .unwrap();
    assert_eq!(not_featured, 0);
    assert!(!product(&app, "SPOON-WD").await.featured);
}

#[renox::test]
async fn categories_open_their_edit_page_and_count_products() {
    let app = app().await;
    login(&app, "admin@example.com").await;
    let tea = Category::where_eq("name", "Tea")
        .first(app.db())
        .await
        .unwrap()
        .unwrap();
    app.get("/admin/categories")
        .await
        .assert_ok()
        .assert_see(&format!("/admin/categories/{}/edit", tea.id));
    app.get(&format!("/admin/categories/{}", tea.id))
        .await
        .assert_not_found();
    // The name is unique: another category's is refused.
    app.htmx()
        .put(
            &format!("/admin/categories/{}", tea.id),
            &[("name", "Coffee")],
        )
        .await
        .assert_status(422);
}

#[renox::test]
async fn an_admin_manages_customers_and_exports_them() {
    let app = app().await;
    login(&app, "admin@example.com").await;
    app.htmx()
        .post(
            "/admin/customers",
            &[
                ("name", "Indra Kusuma"),
                ("email", "Indra@Example.com"),
                ("tier", "gold"),
                ("newsletter", "on"),
            ],
        )
        .await
        .assert_hx_redirect("/admin/customers");
    let indra = Customer::where_eq("email", "indra@example.com")
        .first(app.db())
        .await
        .unwrap()
        .unwrap();
    assert!(indra.newsletter);

    let csv = app.get("/admin/customers?export=csv&in.tier=gold").await;
    csv.assert_ok();
    let text = csv.text();
    assert!(text.contains("Indra Kusuma"), "{text}");
    assert!(text.contains("Ayu Lestari"), "{text}");
    assert!(!text.contains("Budi Santoso"), "{text}");
}
