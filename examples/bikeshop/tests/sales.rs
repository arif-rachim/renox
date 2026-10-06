//! Cart, checkout, payments and counter sales (#234).

use bikeshop::app::catalog::model::{Brand, Category, CategoryKind, Product, ProductVariant};
use bikeshop::app::sales::cart::Cart;
use bikeshop::app::sales::model::SavedCart;
use bikeshop::app::staff::model::Store;
use bikeshop::app::stock::model::StockLevel;
use bikeshop::seed::fixtures;
use renox::prelude::*;
use renox::testing::TestApp;

/// Two stores and a few products with known stock.
struct World {
    north: Store,
    south: Store,
    /// A helmet: 3 at North, 1 at South.
    helmet: ProductVariant,
    /// A chain (a part): 10 at North.
    chain: ProductVariant,
    /// A bike: 1 at North.
    bike: ProductVariant,
}

async fn variant(db: &Db, name: &str, kind: CategoryKind, price: i64) -> ProductVariant {
    let n = bikeshop::seed::unique();
    let category = Category::create(
        db,
        Category {
            name: format!("{name} category {n}"),
            slug: format!("cat-{n}"),
            kind,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let brand = Brand::create(
        db,
        Brand {
            name: format!("Brand {n}"),
            slug: format!("brand-{n}"),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let product = Product::create(
        db,
        Product {
            category_id: category.id,
            brand_id: brand.id,
            name: name.into(),
            slug: format!("{}-{n}", name.to_lowercase().replace(' ', "-")),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    ProductVariant::create(
        db,
        ProductVariant {
            product_id: product.id,
            sku: format!("SKU-{n}"),
            price,
            cost: price / 2,
            ..Default::default()
        },
    )
    .await
    .unwrap()
}

async fn stock(db: &Db, variant: i64, store: i64, on_hand: i64) {
    StockLevel::create(
        db,
        StockLevel {
            variant_id: variant,
            owner_store_id: store,
            location_store_id: store,
            on_hand,
            ..Default::default()
        },
    )
    .await
    .unwrap();
}

async fn world(app: &TestApp) -> World {
    let db = app.db();
    let north = fixtures::store(db, "North").await.unwrap();
    let south = fixtures::store(db, "South").await.unwrap();
    let helmet = variant(db, "Helmet", CategoryKind::Gear, 450_000).await;
    let chain = variant(db, "Chain", CategoryKind::Part, 200_000).await;
    let bike = variant(db, "Road bike", CategoryKind::Bike, 12_000_000).await;
    stock(db, helmet.id, north.id, 3).await;
    stock(db, helmet.id, south.id, 1).await;
    stock(db, chain.id, north.id, 10).await;
    stock(db, bike.id, north.id, 1).await;
    World {
        north,
        south,
        helmet,
        chain,
        bike,
    }
}

fn add(variant: &ProductVariant, quantity: i64) -> Vec<(&'static str, String)> {
    vec![
        ("variant_id", variant.id.to_string()),
        ("quantity", quantity.to_string()),
    ]
}

async fn post(
    app: &TestApp,
    uri: &str,
    form: Vec<(&'static str, String)>,
) -> renox::testing::TestResponse {
    let pairs: Vec<(&str, &str)> = form.iter().map(|(k, v)| (*k, v.as_str())).collect();
    app.post(uri, &pairs).await
}

#[renox::test]
async fn the_cart_holds_no_more_than_the_store_has() {
    let app = TestApp::new(bikeshop::app()).await;
    let w = world(&app).await;
    // The first store (North) by default: 3 helmets there.
    post(&app, "/cart", add(&w.helmet, 2))
        .await
        .assert_status(303);
    post(&app, "/cart", add(&w.helmet, 5))
        .await
        .assert_status(303);
    let cart: Cart = app.session_get("cart").unwrap();
    assert_eq!(cart.store_id, Some(w.north.id));
    assert_eq!(cart.lines[0].quantity, 3, "held to what North has");
    // Nothing left to add: a warning, the cart unchanged.
    let res = app
        .htmx()
        .post(
            "/cart",
            &[("variant_id", &w.helmet.id.to_string()), ("quantity", "1")],
        )
        .await;
    res.assert_ok();
    assert!(
        res.header("hx-trigger")
            .unwrap_or_default()
            .contains("There are no more")
    );
    // The quantity rules: 1 to 20.
    app.htmx()
        .post(
            "/cart",
            &[("variant_id", &w.helmet.id.to_string()), ("quantity", "0")],
        )
        .await
        .assert_invalid("quantity");

    // South has one: switching stores lowers the line and says so.
    app.post("/cart/store", &[("store_id", &w.south.id.to_string())])
        .await
        .assert_status(303);
    let page = app.get("/cart").await;
    page.assert_ok().assert_see("Helmet").assert_see("South");
    let cart: Cart = app.session_get("cart").unwrap();
    assert_eq!(cart.lines[0].quantity, 1);

    // Someone else buys it: the next visit lowers the line to zero and drops it.
    StockLevel::where_eq("variant_id", w.helmet.id)
        .where_eq("location_store_id", w.south.id)
        .update(app.db(), &[("reserved", &1)])
        .await
        .unwrap();
    app.get("/cart")
        .await
        .assert_see("Some quantities changed")
        .assert_see("Your cart is empty");
    let _ = (w.chain, w.bike);
}

#[renox::test]
async fn htmx_changes_answer_the_lines_and_the_navbar_count() {
    let app = TestApp::new(bikeshop::app()).await;
    let w = world(&app).await;
    // Adding from a product page: the navbar's count, out of band, and a toast.
    let added = app
        .htmx()
        .post(
            "/cart",
            &[("variant_id", &w.chain.id.to_string()), ("quantity", "2")],
        )
        .await;
    let text = added.assert_ok().text();
    assert!(
        text.contains("id=\"nav-cart\"") && text.contains("hx-swap-oob=\"true\""),
        "{text}"
    );
    assert!(text.contains(">2</span>"), "the count: {text}");
    assert!(
        added
            .header("hx-trigger")
            .unwrap_or_default()
            .contains("renox:toast")
    );
    // Changing a quantity on the cart page: the lines plus the count.
    let changed = app
        .htmx()
        .patch(&format!("/cart/{}", w.chain.id), &[("quantity", "4")])
        .await;
    let text = changed.assert_ok().text();
    assert!(
        text.contains("id=\"cart\"") && text.contains("id=\"nav-cart\""),
        "{text}"
    );
    assert!(text.contains(">4</span>"));
    // The navbar asks for its count when a page loads.
    app.htmx()
        .get("/cart/mini")
        .await
        .assert_ok()
        .assert_see(">4</span>");
    app.htmx()
        .delete(&format!("/cart/{}", w.chain.id))
        .await
        .assert_ok()
        .assert_see("Your cart is empty");
}

#[renox::test]
async fn a_guest_cart_joins_the_saved_cart_after_logging_in() {
    let app = TestApp::new(bikeshop::app()).await;
    let w = world(&app).await;
    let db = app.db();
    let user = User::register(db, "Ana", "ana@example.com", "password123")
        .await
        .unwrap();
    // Saved from an earlier visit on another device: one chain.
    let mut saved = Cart::default();
    saved.add(w.chain.id, 1);
    SavedCart::create(
        db,
        SavedCart {
            user_id: user.id,
            store_id: Some(w.north.id),
            lines: renox::db::Json(saved.lines.clone()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    // As a guest: two helmets and one more chain.
    post(&app, "/cart", add(&w.helmet, 2)).await;
    post(&app, "/cart", add(&w.chain, 1)).await;
    app.acting_as(&user);
    app.get("/cart")
        .await
        .assert_ok()
        .assert_see("Helmet")
        .assert_see("Chain");
    app.assert_session_missing("cart");
    let row = SavedCart::where_eq("user_id", user.id)
        .first(db)
        .await
        .unwrap()
        .unwrap();
    let mut lines = row.lines.0.clone();
    lines.sort_by_key(|l| l.variant_id);
    let mut want = vec![(w.helmet.id, 2), (w.chain.id, 2)];
    want.sort();
    assert_eq!(
        lines
            .iter()
            .map(|l| (l.variant_id, l.quantity))
            .collect::<Vec<_>>(),
        want
    );
    // From now on, the account's cart is used (another device sees it too).
    post(&app, "/cart", add(&w.helmet, 1)).await;
    let row = SavedCart::where_eq("user_id", user.id)
        .first(db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        row.lines
            .0
            .iter()
            .find(|l| l.variant_id == w.helmet.id)
            .unwrap()
            .quantity,
        3
    );
    let _ = w.bike;
}
