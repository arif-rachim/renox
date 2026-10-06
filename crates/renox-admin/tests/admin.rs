//! The admin panel (#148): who gets in, the list (grid, filters, exports,
//! actions), create, edit, view, delete with soft deletes and the trash,
//! and the pages an app replaces.

use renox::chrono::NaiveDate;
use renox::grid::Column;
use renox::prelude::*;
use renox::testing::TestApp;
use renox_admin::{Admin, AdminAction, AdminResource, Entry, Field, Filter};
use serde::{Deserialize, Serialize};

const PASSWORD: &str = "secret-password-1";
const VIEWER: &str = "viewer@example.com";

#[derive(Model, Serialize, Deserialize, Default, Debug, Clone)]
#[model(table = "products", soft_deletes)]
struct Product {
    id: i64,
    name: String,
    sku: String,
    price: i64,
    status: String,
    active: bool,
    category_id: Option<i64>,
    released_on: Option<NaiveDate>,
    created_at: Option<DateTime>,
    updated_at: Option<DateTime>,
    deleted_at: Option<DateTime>,
}

/// The viewer looks; everyone else in the panel may do anything.
impl Policy for Product {
    fn allows(&self, user: &User, ability: &str) -> bool {
        matches!(ability, "viewAny" | "view") || user.email != VIEWER
    }
}

#[derive(Deserialize, Serialize, Validate)]
struct ProductForm {
    #[validate(required, max = 100)]
    name: String,
    #[validate(required, max = 20)]
    sku: String,
    #[validate(required, min = 0)]
    price: i64,
    #[validate(required, one_of(&["draft", "live"]))]
    status: String,
    active: bool,
    #[validate(exists("categories", "id"))]
    category_id: Option<i64>,
    released_on: Option<NaiveDate>,
}

struct Products;

impl AdminResource for Products {
    type Model = Product;
    type Form = ProductForm;

    fn label(&self) -> &str {
        "Product"
    }

    fn plural_label(&self) -> &str {
        "Products"
    }

    fn navigation_group(&self) -> Option<&str> {
        Some("Shop")
    }

    fn columns(&self) -> Vec<Column> {
        vec![
            Column::text("name", "Name").searchable(),
            Column::text("sku", "SKU").searchable(),
            Column::money("price", "Price"),
            Column::select("status", "Status", [("draft", "Draft"), ("live", "Live")]),
            Column::bool("active", "Active"),
            Column::custom("stock", "Stock"),
        ]
    }

    fn fields(&self) -> Vec<Field> {
        vec![
            Field::text("name", "Name").required(),
            Field::text("sku", "SKU").required().readonly_on_edit(),
            Field::money("price", "Price").required(),
            Field::select("status", "Status", [("draft", "Draft"), ("live", "Live")]).required(),
            Field::toggle("active", "Active").default_value(true),
            Field::belongs_to("category_id", "Category", "categories", "name"),
            Field::date("released_on", "Released"),
        ]
    }

    fn entries(&self) -> Vec<Entry> {
        let mut entries: Vec<Entry> = self
            .columns()
            .iter()
            .filter_map(Entry::from_column)
            .collect();
        entries.push(Entry::new("released_on", "Released").format("date"));
        entries
    }

    fn fill(&self, product: &mut Product, form: ProductForm) {
        product.name = form.name;
        product.sku = form.sku.to_uppercase();
        product.price = form.price;
        product.status = form.status;
        product.active = form.active;
        product.category_id = form.category_id;
        product.released_on = form.released_on;
    }

    fn rules(&self, form: &ProductForm, record: Option<&Product>, v: &mut Validator) {
        let upper = form.sku.to_uppercase();
        let sku = v
            .field("sku", &upper)
            .label("SKU")
            .unique("products", "sku");
        if let Some(product) = record {
            sku.ignore(product.id);
        }
    }

    fn filters(&self) -> Vec<Filter<Product>> {
        vec![Filter::new("live", "Live", |q| {
            q.where_eq("status", "live")
        })]
    }

    fn actions(&self) -> Vec<AdminAction<Product>> {
        vec![
            AdminAction::new(
                "publish",
                "Publish",
                |products: Vec<Product>, cx| async move {
                    let ids: Vec<i64> = products.iter().map(|p| p.id).collect();
                    let n = Product::query()
                        .where_in("id", ids)
                        .update(&cx.state.db, &[("status", &"live")])
                        .await?;
                    Ok(Toast::success(format!(
                        "{n} published by {}.",
                        cx.user.name
                    )))
                },
            )
            .confirm("Publish the selected products?")
            .row(),
        ]
    }
}

#[derive(Model, Serialize, Deserialize, Default, Debug, Clone)]
#[model(table = "categories")]
struct Category {
    id: i64,
    name: String,
}

/// Nobody may see categories but the owner.
impl Policy for Category {
    fn allows(&self, user: &User, _ability: &str) -> bool {
        user.email == "owner@example.com"
    }
}

#[derive(Deserialize, Serialize, Validate)]
struct CategoryForm {
    #[validate(required, max = 50)]
    name: String,
}

struct Categories;

impl AdminResource for Categories {
    type Model = Category;
    type Form = CategoryForm;

    fn label(&self) -> &str {
        "Category"
    }

    fn plural_label(&self) -> &str {
        "Categories"
    }

    fn columns(&self) -> Vec<Column> {
        vec![Column::text("name", "Name")]
    }

    fn fields(&self) -> Vec<Field> {
        vec![Field::text("name", "Name").required()]
    }

    /// No view page: rows open the edit page.
    fn entries(&self) -> Vec<Entry> {
        Vec::new()
    }

    fn fill(&self, category: &mut Category, form: CategoryForm) {
        category.name = form.name;
    }
}

fn admin() -> Admin {
    Admin::new()
        .title("Back office")
        .authorize(|user| user.email.ends_with("@example.com"))
        .resource(Products)
        .resource(Categories)
}

async fn app_with(admin: Admin) -> TestApp {
    TestApp::new(
        App::new()
            .module(Auth::new())
            .module(admin)
            .migrations(renox::migrations!("tests/migrations")),
    )
    .await
}

async fn app() -> TestApp {
    app_with(admin()).await
}

async fn user(app: &TestApp, email: &str) -> User {
    User::register(app.db(), "Ana", email, PASSWORD)
        .await
        .unwrap()
}

async fn product(app: &TestApp, name: &str, sku: &str, status: &str) -> Product {
    Product::create(
        app.db(),
        Product {
            name: name.into(),
            sku: sku.into(),
            price: 75_000,
            status: status.into(),
            active: true,
            ..Default::default()
        },
    )
    .await
    .unwrap()
}

#[renox::test]
async fn guests_log_in_and_others_are_refused() {
    let app = app().await;
    app.get("/admin").await.assert_redirect("/login");
    app.get("/admin/products").await.assert_redirect("/login");

    let stranger = user(&app, "someone@elsewhere.test").await;
    app.acting_as(&stranger);
    app.get("/admin").await.assert_forbidden();
    app.get("/admin/products").await.assert_forbidden();
    app.get("/admin/products/create").await.assert_forbidden();
}

#[renox::test]
async fn nobody_gets_in_until_the_app_says_who_may() {
    let app = app_with(Admin::new().resource(Products)).await;
    let ana = user(&app, "ana@example.com").await;
    app.acting_as(&ana);
    app.get("/admin").await.assert_forbidden();
    app.get("/admin/products").await.assert_forbidden();
}

#[renox::test]
async fn the_dashboard_counts_what_the_user_may_see() {
    let app = app().await;
    product(&app, "Coffee", "C-1", "live").await;
    product(&app, "Tea", "T-1", "draft").await;
    let ana = user(&app, "ana@example.com").await;
    app.acting_as(&ana);
    let page = app.get("/admin").await;
    page.assert_ok()
        .assert_view("renox-admin/dashboard.html")
        .assert_see("Back office")
        .assert_see(r#"href="/admin/products""#)
        .assert_see("Shop")
        // Categories' policy lets only the owner see them.
        .assert_dont_see("Categories");
    assert!(page.text().contains("rx-shell"), "{}", page.text());

    let owner = user(&app, "owner@example.com").await;
    app.acting_as(&owner);
    app.get("/admin").await.assert_see("Categories");
    app.get("/admin/categories").await.assert_ok();
    app.acting_as(&ana);
    app.get("/admin/categories").await.assert_forbidden();
}

#[renox::test]
async fn the_list_is_a_grid_with_search_filters_and_actions() {
    let app = app().await;
    product(&app, "Iced coffee", "C-1", "live").await;
    product(&app, "Green tea", "T-1", "draft").await;
    let ana = user(&app, "ana@example.com").await;
    app.acting_as(&ana);

    let page = app.get("/admin/products").await;
    page.assert_ok()
        .assert_view("renox-admin/index.html")
        .assert_see("Iced coffee")
        .assert_see("Green tea")
        .assert_see(r#"href="/admin/products/create""#)
        .assert_see("New product")
        .assert_see("Publish")
        .assert_see("/admin/products/actions/delete")
        .assert_see("/admin/products/actions/publish")
        // Tabs: all, the named filter, the trash.
        .assert_see(r#"href="/admin/products?filter=live""#)
        .assert_see(r#"href="/admin/products?filter=trashed""#);

    // The grid's own search and the named filter.
    app.get("/admin/products?search=iced")
        .await
        .assert_see("Iced coffee")
        .assert_dont_see("Green tea");
    app.get("/admin/products?filter=live")
        .await
        .assert_see("Iced coffee")
        .assert_dont_see("Green tea");
    app.get("/admin/products?in.status=draft")
        .await
        .assert_see("Green tea")
        .assert_dont_see("Iced coffee");
}

#[renox::test]
async fn the_list_exports_csv() {
    let app = app().await;
    product(&app, "Iced coffee", "C-1", "live").await;
    let ana = user(&app, "ana@example.com").await;
    app.acting_as(&ana);
    let file = app.get("/admin/products?export=csv").await;
    file.assert_ok();
    assert!(
        file.header("content-type").unwrap().starts_with("text/csv"),
        "{:?}",
        file.header("content-type")
    );
    let text = file.text();
    assert!(text.contains("Name,SKU,Price,Status,Active"), "{text}");
    assert!(text.contains("Iced coffee,C-1,75000,Live,Yes"), "{text}");
}

#[renox::test]
async fn a_viewer_sees_no_buttons_and_may_change_nothing() {
    let app = app().await;
    let coffee = product(&app, "Coffee", "C-1", "live").await;
    let viewer = user(&app, VIEWER).await;
    app.acting_as(&viewer);
    app.get("/admin/products")
        .await
        .assert_ok()
        .assert_dont_see("New product")
        .assert_dont_see("/admin/products/actions/delete")
        .assert_dont_see("/admin/products/actions/publish");
    app.get(&format!("/admin/products/{}", coffee.id))
        .await
        .assert_ok()
        .assert_dont_see("/edit");
    app.get("/admin/products/create").await.assert_forbidden();
    app.get(&format!("/admin/products/{}/edit", coffee.id))
        .await
        .assert_forbidden();
    app.post("/admin/products", &[("name", "X")])
        .await
        .assert_forbidden();
    app.delete(&format!("/admin/products/{}", coffee.id))
        .await
        .assert_forbidden();
    app.htmx()
        .post(
            "/admin/products/actions/publish",
            &[("ids", &coffee.id.to_string())],
        )
        .await
        .assert_forbidden();
    app.assert_database_count("products", 1).await;
}

#[renox::test]
async fn the_create_page_draws_the_fields() {
    let app = app().await;
    Category::create(
        app.db(),
        Category {
            name: "Drinks".into(),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let ana = user(&app, "ana@example.com").await;
    app.acting_as(&ana);
    let page = app.get("/admin/products/create").await;
    let html = page.assert_ok().assert_view("renox-admin/form.html").text();
    let has = |needle: &str| assert!(html.contains(needle), "missing {needle}\n{html}");
    has("New product");
    has(r#"hx-post="/admin/products""#);
    has(r#"data-live-validate"#);
    has(r#"name="name" type="text""#);
    // Money: a number with the currency's code before it.
    has(r#"<span class="rx-affix__text" id="rx-price-prefix">IDR</span>"#);
    has(r#"name="price" type="number""#);
    // The select and the belongs-to choices.
    has(r#"<option value="live">Live</option>"#);
    has(r#">Drinks</option>"#);
    has("data-rx-combobox");
    // A toggle on by default, and the date picker.
    has(r#"role="switch" id="rx-active" name="active" value="on" checked"#);
    has(r#"name="released_on""#);
    has("data-rx-date");
}

#[renox::test]
async fn creating_checks_the_form_then_saves() {
    let app = app().await;
    let ana = user(&app, "ana@example.com").await;
    app.acting_as(&ana);

    // htmx: errors stay in the form (422).
    let failed = app
        .htmx()
        .post("/admin/products", &[("name", ""), ("price", "-1")])
        .await;
    failed.assert_status(422);
    assert!(failed.json_path("errors.name").is_array());
    assert!(failed.json_path("errors.price").is_array());
    app.assert_database_count("products", 0).await;

    let saved = app
        .htmx()
        .post(
            "/admin/products",
            &[
                ("name", "Iced coffee"),
                ("sku", "c-1"),
                ("price", "25000"),
                ("status", "live"),
                ("active", "on"),
                ("released_on", "2026-10-01"),
            ],
        )
        .await;
    saved.assert_hx_redirect("/admin/products");
    let coffee = Product::query().first(app.db()).await.unwrap().unwrap();
    assert_eq!(coffee.sku, "C-1");
    assert_eq!(coffee.price, 25_000);
    assert!(coffee.active);
    assert_eq!(coffee.released_on, NaiveDate::from_ymd_opt(2026, 10, 1));
    // The toast waits for the list.
    app.get("/admin/products")
        .await
        .assert_see("Product created.");

    // The resource's own rule: the SKU is taken.
    let taken = app
        .htmx()
        .post(
            "/admin/products",
            &[
                ("name", "Other"),
                ("sku", "C-1"),
                ("price", "1"),
                ("status", "draft"),
            ],
        )
        .await;
    taken.assert_status(422);
    assert!(taken.json_path("errors.sku").is_array());

    // Without htmx: a redirect back with the errors and the input.
    app.post(
        "/admin/products",
        &[
            ("name", "Other"),
            ("sku", "C-1"),
            ("price", "1"),
            ("status", "draft"),
        ],
    )
    .await
    .assert_status(303);
    app.assert_database_count("products", 1).await;
}

#[renox::test]
async fn editing_shows_the_record_and_saves_it() {
    let app = app().await;
    let coffee = product(&app, "Coffee", "C-1", "live").await;
    let ana = user(&app, "ana@example.com").await;
    app.acting_as(&ana);
    let url = format!("/admin/products/{}", coffee.id);
    let html = app.get(&format!("{url}/edit")).await.assert_ok().text();
    let has = |needle: &str| assert!(html.contains(needle), "missing {needle}\n{html}");
    has(r#"value="Coffee""#);
    has(r#"value="75000""#);
    has(r#"<option value="live" selected>Live</option>"#);
    // The SKU can't change here, and the page can delete.
    has(r#"name="sku" type="text" value="C-1" required aria-required="true" readonly"#);
    has(r#"name="_method" value="PUT""#);
    has("Delete this product?");

    app.htmx()
        .put(
            &url,
            &[
                ("name", "Hot coffee"),
                ("sku", "C-1"),
                ("price", "30000"),
                ("status", "draft"),
            ],
        )
        .await
        .assert_hx_redirect("/admin/products");
    let saved = Product::find(app.db(), coffee.id).await.unwrap().unwrap();
    assert_eq!(saved.name, "Hot coffee");
    assert_eq!(saved.price, 30_000);
    assert_eq!(saved.status, "draft");
    // An unticked toggle is `false`.
    assert!(!saved.active);

    app.get("/admin/products/999/edit").await.assert_not_found();
    app.get("/admin/products/abc/edit").await.assert_not_found();
}

#[renox::test]
async fn the_view_page_lists_the_entries() {
    let app = app().await;
    let coffee = product(&app, "Coffee", "C-1", "live").await;
    let ana = user(&app, "ana@example.com").await;
    app.acting_as(&ana);
    let page = app.get(&format!("/admin/products/{}", coffee.id)).await;
    page.assert_ok()
        .assert_view("renox-admin/show.html")
        .assert_see("Product #")
        .assert_see("Coffee")
        .assert_see("Rp 75,000")
        .assert_see(r#"<span class="rx-badge">Live</span>"#)
        .assert_see(&format!("/admin/products/{}/edit", coffee.id))
        .assert_see("Released");

    // A resource without entries has no view page: rows open the edit page.
    let owner = user(&app, "owner@example.com").await;
    app.acting_as(&owner);
    let drinks = Category::create(
        app.db(),
        Category {
            name: "Drinks".into(),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    app.get(&format!("/admin/categories/{}", drinks.id))
        .await
        .assert_not_found();
    app.get("/admin/categories")
        .await
        .assert_see(&format!("/admin/categories/{}/edit", drinks.id))
        .assert_dont_see(&format!(r#"href="/admin/categories/{}""#, drinks.id));
}

#[renox::test]
async fn deleting_moves_to_the_trash_where_records_come_back() {
    let app = app().await;
    let coffee = product(&app, "Coffee", "C-1", "live").await;
    let tea = product(&app, "Tea", "T-1", "live").await;
    let ana = user(&app, "ana@example.com").await;
    app.acting_as(&ana);

    // From the grid (htmx): a toast, the grid reloads itself.
    app.htmx()
        .delete(&format!("/admin/products/{}", coffee.id))
        .await
        .assert_status(204);
    // From the edit page's form: back to the list.
    app.delete(&format!("/admin/products/{}", tea.id))
        .await
        .assert_redirect("/admin/products");
    app.assert_database_count("products", 2).await;
    assert_eq!(Product::query().count(app.db()).await.unwrap(), 0);

    let trash = app.get("/admin/products?filter=trashed").await;
    trash
        .assert_ok()
        .assert_see("Coffee")
        .assert_see("/admin/products/actions/restore")
        .assert_see("/admin/products/actions/force-delete")
        .assert_dont_see("New product");
    app.get(&format!("/admin/products/{}", coffee.id))
        .await
        .assert_ok()
        .assert_see("is in the trash")
        .assert_see("Restore");

    app.htmx()
        .post(
            "/admin/products/actions/restore?filter=trashed",
            &[("ids", &coffee.id.to_string())],
        )
        .await
        .assert_status(204);
    assert_eq!(Product::query().count(app.db()).await.unwrap(), 1);
    // Restoring outside the trash isn't offered.
    app.htmx()
        .post(
            "/admin/products/actions/restore",
            &[("ids", &tea.id.to_string())],
        )
        .await
        .assert_not_found();

    app.post(
        &format!("/admin/products/{}/actions/force-delete", tea.id),
        &[],
    )
    .await
    .assert_redirect("/admin/products");
    app.assert_database_count("products", 1).await;
}

#[renox::test]
async fn bulk_actions_run_on_the_selection_or_everything_filtered() {
    let app = app().await;
    let a = product(&app, "Coffee", "C-1", "draft").await;
    let b = product(&app, "Tea", "T-1", "draft").await;
    let c = product(&app, "Juice", "J-1", "draft").await;
    let ana = user(&app, "ana@example.com").await;
    app.acting_as(&ana);

    let ids = format!("{},{}", a.id, b.id);
    let done = app
        .htmx()
        .post("/admin/products/actions/publish", &[("ids", &ids)])
        .await;
    done.assert_status(204);
    let live = Product::where_eq("status", "live")
        .count(app.db())
        .await
        .unwrap();
    assert_eq!(live, 2);

    // One row from its menu.
    app.htmx()
        .post(&format!("/admin/products/{}/actions/publish", c.id), &[])
        .await
        .assert_status(204);
    assert_eq!(
        Product::where_eq("status", "live")
            .count(app.db())
            .await
            .unwrap(),
        3
    );

    // "Select all matching": the grid's filters choose the rows.
    app.htmx()
        .post(
            "/admin/products/actions/delete?search=tea",
            &[("ids", ""), ("all", "true")],
        )
        .await
        .assert_status(204);
    assert_eq!(Product::query().count(app.db()).await.unwrap(), 2);
    assert!(Product::find(app.db(), b.id).await.unwrap().is_none());

    app.htmx()
        .post("/admin/products/actions/nothing", &[("ids", &ids)])
        .await
        .assert_not_found();
}

#[renox::test]
async fn categories_have_no_soft_deletes_and_no_trash() {
    let app = app().await;
    let owner = user(&app, "owner@example.com").await;
    app.acting_as(&owner);
    app.post("/admin/categories", &[("name", "Drinks")])
        .await
        .assert_redirect("/admin/categories");
    let drinks = Category::query().first(app.db()).await.unwrap().unwrap();
    app.get("/admin/categories")
        .await
        .assert_see("Drinks")
        .assert_dont_see("filter=trashed");
    app.htmx()
        .post(
            "/admin/categories/actions/restore",
            &[("ids", &drinks.id.to_string())],
        )
        .await
        .assert_not_found();
    app.htmx()
        .delete(&format!("/admin/categories/{}", drinks.id))
        .await
        .assert_status(204);
    app.assert_database_count("categories", 0).await;
}

#[renox::test]
async fn the_app_replaces_pages_and_draws_custom_cells() {
    let app = TestApp::new(
        App::new()
            .module(Auth::new())
            .module(admin().path("/backoffice").title("Shop admin"))
            .migrations(renox::migrations!("tests/migrations"))
            .templates(|env| {
                env.add_template(
                    "renox-admin/products/cells.html",
                    r#"{% if column.key == "stock" %}<b class="stock">{{ row.sku }} in stock</b>{% endif %}"#,
                )
                .unwrap();
                env.add_template(
                    "renox-admin/dashboard.html",
                    r#"{% extends "renox-admin/layout.html" %}{% block content %}<p>Hello from the app</p>{% endblock %}"#,
                )
                .unwrap();
            }),
    )
    .await;
    product(&app, "Coffee", "C-1", "live").await;
    let ana = user(&app, "ana@example.com").await;
    app.acting_as(&ana);
    app.get("/backoffice")
        .await
        .assert_ok()
        .assert_see("Hello from the app")
        .assert_see("Shop admin");
    app.get("/backoffice/products")
        .await
        .assert_ok()
        .assert_see(r#"<b class="stock">C-1 in stock</b>"#)
        .assert_see("/backoffice/products/create");
    app.get("/admin").await.assert_not_found();
}

#[renox::test]
async fn routes_are_named_and_behind_a_login() {
    let app = app().await;
    let url =
        |name: &str, params: &[&dyn std::fmt::Display]| app.state().url(name, params).unwrap();
    assert_eq!(url("admin.dashboard", &[]), "/admin");
    assert_eq!(url("admin.products.index", &[]), "/admin/products");
    assert_eq!(url("admin.products.create", &[]), "/admin/products/create");
    assert_eq!(url("admin.products.edit", &[&7]), "/admin/products/7/edit");
    assert_eq!(url("admin.products.show", &[&7]), "/admin/products/7");
    assert_eq!(url("admin.categories.index", &[]), "/admin/categories");
}

// ---------- #260: every field kind and modifier, entries, actions, the gate ----------

/// The products again, drawn with every kind of field (the pages are only
/// shown here, not saved).
struct Specs;

impl AdminResource for Specs {
    type Model = Product;
    type Form = ProductForm;

    fn label(&self) -> &str {
        "Spec"
    }

    fn plural_label(&self) -> &str {
        "Specs"
    }

    fn slug(&self) -> &str {
        "specs"
    }

    fn columns(&self) -> Vec<Column> {
        vec![
            Column::text("name", "Name"),
            Column::number("price", "Price"),
        ]
    }

    fn fields(&self) -> Vec<Field> {
        vec![
            Field::email("name", "Contact email")
                .hint("We never share it.")
                .placeholder("ana@example.com")
                .autocomplete("email"),
            Field::password("sku", "Secret"),
            Field::url("website", "Site").prefix("https://"),
            Field::tel("phone", "Phone").suffix("ext"),
            Field::textarea("notes", "Notes")
                .rows(4)
                .span_full()
                .only_on_edit(),
            Field::number("price", "Price").step("0.5").min(1).max(120),
            Field::datetime("created_at", "Created"),
            Field::checkbox("active", "Active"),
            Field::select("status", "Status", [("draft", "Draft"), ("live", "Live")]).searchable(),
            Field::text("coupon", "Coupon").only_on_create(),
        ]
    }

    fn entries(&self) -> Vec<Entry> {
        let mut entries = vec![
            Entry::text("name", "Name").copyable(),
            Entry::new("sku", "SKU").span_full(),
        ];
        entries.extend(self.columns().iter().filter_map(Entry::from_column));
        entries
    }

    fn fill(&self, product: &mut Product, form: ProductForm) {
        product.name = form.name;
    }

    fn actions(&self) -> Vec<AdminAction<Product>> {
        vec![
            AdminAction::new("archive", "Archive", |_: Vec<Product>, _cx| async {
                Ok(Toast::success("Archived."))
            })
            .danger()
            .ability("update"),
            AdminAction::new("print", "Print label", |_: Vec<Product>, _cx| async {
                Ok(Toast::success("Printed."))
            })
            .row_only(),
        ]
    }
}

async fn specs() -> (TestApp, Product) {
    let app = app_with(admin().resource(Specs)).await;
    let coffee = product(&app, "Coffee", "C-SECRET", "live").await;
    renox::db::sql("UPDATE products SET created_at = ? WHERE id = ?")
        .bind(
            renox::chrono::DateTime::parse_from_rfc3339("2026-10-05T09:30:45Z")
                .unwrap()
                .with_timezone(&renox::chrono::Utc),
        )
        .bind(coffee.id)
        .execute(app.db())
        .await
        .unwrap();
    let ana = user(&app, "ana@example.com").await;
    app.acting_as(&ana);
    (app, coffee)
}

#[renox::test]
async fn every_field_kind_and_modifier_is_drawn() {
    let (app, coffee) = specs().await;
    let create = app.get("/admin/specs/create").await;
    create.assert_ok();
    let html = create.text();
    for needle in [
        r#"type="email""#,
        "We never share it.",
        r#"placeholder="ana@example.com""#,
        r#"autocomplete="email""#,
        r#"type="password""#,
        r#"type="url""#,
        "https://",
        r#"type="tel""#,
        "ext",
        r#"type="number""#,
        r#"step="0.5""#,
        r#"min="1""#,
        r#"max="120""#,
        r#"type="datetime-local""#,
        r#"type="checkbox""#,
        r#"name="coupon""#,
    ] {
        assert!(html.contains(needle), "the create page lacks {needle}");
    }
    assert!(
        !html.contains(r#"name="notes""#),
        "notes are for editing only"
    );

    let edit = app.get(&format!("/admin/specs/{}/edit", coffee.id)).await;
    edit.assert_ok();
    let html = edit.text();
    assert!(html.contains(r#"name="notes""#) && html.contains(r#"rows="4""#));
    assert!(
        !html.contains(r#"name="coupon""#),
        "the coupon is for creating only"
    );
    // A password is never filled in, whatever the record holds.
    assert!(
        !html.contains("C-SECRET"),
        "the stored value leaked into the form"
    );
    // A moment fits datetime-local: minutes, no zone.
    assert!(
        html.contains(r#"value="2026-10-05T09:30""#),
        "the date-time value"
    );
}

#[renox::test]
async fn entries_and_actions_follow_their_options() {
    let (app, coffee) = specs().await;
    let show = app.get(&format!("/admin/specs/{}", coffee.id)).await;
    show.assert_ok().assert_see("Coffee").assert_see("C-SECRET");
    let list = app.get("/admin/specs").await.text();
    // A danger action is a red bulk button; a row-only one is only in each
    // row's menu, never offered for a selection.
    assert!(list.contains(
        r#"class="rx-button rx-button--small rx-button--danger" data-grid-bulk-action data-url="/admin/specs/actions/archive""#
    ));
    assert!(list.contains(&format!(
        r#"data-url="/admin/specs/{}/actions/print""#,
        coffee.id
    )));
    assert!(!list.contains(r#"data-url="/admin/specs/actions/print""#));
    // An action the resource doesn't have is a 404.
    app.post(&format!("/admin/specs/{}/actions/nope", coffee.id), &[])
        .await
        .assert_not_found();
}

#[renox::test]
async fn a_gate_can_guard_the_panel() {
    let app = TestApp::new(
        App::new()
            .module(Auth::new())
            .module(Admin::new().gate("back-office").resource(Products))
            .migrations(renox::migrations!("tests/migrations"))
            .gate("back-office", |user| user.email == "owner@example.com"),
    )
    .await;
    let ana = user(&app, "ana@example.com").await;
    app.acting_as(&ana);
    app.get("/admin").await.assert_forbidden();
    let owner = user(&app, "owner@example.com").await;
    app.acting_as(&owner);
    app.get("/admin").await.assert_ok();
}
