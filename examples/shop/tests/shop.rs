//! The shop end to end: what customers and admins do, and what must not
//! happen (overselling, other people's orders, customers in the admin).

use renox::audit;
use renox::prelude::*;
use renox::testing::TestApp;
use shop::app::orders::OrderPlaced;
use std::time::Duration;

const DAY: Duration = Duration::from_secs(24 * 60 * 60);
use shop::app::catalog::model::{Category, Product};
use shop::app::orders::model::{Order, OrderStatus};

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDRsmall-but-valid-enough-for-sniffing";

async fn shop() -> TestApp {
    TestApp::new(shop::app()).await
}

async fn customer(app: &TestApp, email: &str) -> User {
    User::register(app.db(), "Budi", email, "password123")
        .await
        .unwrap()
}

async fn admin(app: &TestApp) -> User {
    let user = customer(app, "admin@example.com").await;
    shop::make_admin_of(app.db(), &user).await.unwrap();
    user
}

async fn category(app: &TestApp, name: &str) -> Category {
    Category::create(
        app.db(),
        Category {
            name: name.into(),
            slug: name.to_lowercase(),
            ..Default::default()
        },
    )
    .await
    .unwrap()
}

async fn product(app: &TestApp, name: &str, price: i64, stock: i64) -> Product {
    Product::create(
        app.db(),
        Product {
            name: name.into(),
            slug: shop::app::catalog::model::slug(name),
            description: format!("About {name}"),
            price,
            stock,
            active: true,
            ..Default::default()
        },
    )
    .await
    .unwrap()
}

async fn stock(app: &TestApp, id: i64) -> i64 {
    Product::find_or_404(app.db(), id).await.unwrap().stock
}

#[renox::test]
async fn customers_browse_search_filter_and_sort() {
    let app = shop().await;
    let drinks = category(&app, "Drinks").await;
    let mut kopi = product(&app, "Kopi Susu", 25_000, 5).await;
    kopi.category_id = Some(drinks.id);
    kopi.save(app.db()).await.unwrap();
    product(&app, "Teh Tarik", 18_000, 5).await;
    product(&app, "Kopi Hitam", 15_000, 0).await;
    let mut hidden = product(&app, "Kopi Rahasia", 99_000, 5).await;
    hidden.active = false;
    hidden.save(app.db()).await.unwrap();

    // The home page lists what can be bought.
    app.get("/")
        .await
        .assert_ok()
        .assert_view("catalog/home.html")
        .assert_see("Kopi Susu")
        .assert_see("Rp 25.000")
        .assert_dont_see("Kopi Hitam")
        .assert_dont_see("Kopi Rahasia");

    let page = app.get("/products?q=kopi&sort=price_asc").await;
    page.assert_ok()
        .assert_see("2 products found")
        .assert_dont_see("Teh Tarik")
        .assert_dont_see("Kopi Rahasia");
    let text = page.text();
    assert!(
        text.find("Kopi Hitam") < text.find("Kopi Susu"),
        "cheapest first"
    );

    app.get("/products?category=drinks")
        .await
        .assert_see("One product found")
        .assert_see("Kopi Susu");
    app.get("/products?q=nothing")
        .await
        .assert_see("0 products found");
    app.get("/products?category=missing")
        .await
        .assert_not_found();

    // The search box asks with htmx and gets only the results.
    let results = app.htmx().get("/products?q=teh").await;
    results
        .assert_ok()
        .assert_see("Teh Tarik")
        .assert_dont_see("<header");
}

#[renox::test]
async fn product_pages_have_seo_tags() {
    let app = shop().await;
    let kopi = product(&app, "Kopi Susu", 25_000, 5).await;
    app.get(&format!("/products/{}", kopi.slug))
        .await
        .assert_ok()
        .assert_see("<title>Kopi Susu · ")
        .assert_see(r#"<meta property="og:type" content="product">"#)
        .assert_see("About Kopi Susu")
        .assert_see("Log in to buy");
    app.get("/products/no-such-thing").await.assert_not_found();

    let sitemap = app.get("/sitemap.xml").await;
    sitemap.assert_ok().assert_see("/products/kopi-susu");
}

#[renox::test]
async fn the_cart_adds_up_and_stays_private() {
    let app = shop().await;
    let kopi = product(&app, "Kopi Susu", 25_000, 5).await;
    let id = kopi.id.to_string();

    app.post("/cart", &[("product_id", &id), ("quantity", "1")])
        .await
        .assert_redirect("/login");

    let budi = customer(&app, "budi@example.com").await;
    app.acting_as(&budi);
    app.post("/cart", &[("product_id", &id), ("quantity", "1")])
        .await;
    app.post("/cart", &[("product_id", &id), ("quantity", "2")])
        .await;
    app.get("/cart")
        .await
        .assert_see("Cart (3)")
        .assert_see("Rp 75.000");
    app.htmx()
        .post("/cart", &[("product_id", &id), ("quantity", "0")])
        .await
        .assert_invalid("quantity");
    app.htmx()
        .post("/cart", &[("product_id", "999"), ("quantity", "1")])
        .await
        .assert_invalid("product_id");

    let item: i64 = renox::db::sql("SELECT id FROM cart_items WHERE user_id = ?")
        .bind(budi.id)
        .scalar(app.db())
        .await
        .unwrap();
    app.patch(&format!("/cart/{item}"), &[("quantity", "4")])
        .await
        .assert_redirect("/cart");
    app.get("/cart").await.assert_see("Cart (4)");

    // Someone else can't change or remove Budi's item.
    let siti = customer(&app, "siti@example.com").await;
    app.acting_as(&siti);
    app.patch(&format!("/cart/{item}"), &[("quantity", "1")])
        .await
        .assert_not_found();
    app.delete(&format!("/cart/{item}")).await;
    app.assert_database_has("cart_items", &[("id", &item), ("quantity", &4)])
        .await;

    app.acting_as(&budi);
    app.delete(&format!("/cart/{item}"))
        .await
        .assert_redirect("/cart");
    app.get("/cart").await.assert_see("Your cart is empty.");
}

#[renox::test]
async fn checkout_takes_the_stock_and_confirms_by_mail() {
    let app = shop().await;
    let boss = admin(&app).await;
    let kopi = product(&app, "Kopi Susu", 25_000, 5).await;
    let teh = product(&app, "Teh Tarik", 18_000, 1).await;
    let budi = customer(&app, "budi@example.com").await;
    app.acting_as(&budi);
    for (id, quantity) in [(kopi.id, "2"), (teh.id, "1")] {
        app.post(
            "/cart",
            &[("product_id", &id.to_string()), ("quantity", quantity)],
        )
        .await;
    }

    app.get("/checkout")
        .await
        .assert_ok()
        .assert_view("orders/checkout.html")
        .assert_see("Rp 68.000");
    app.htmx()
        .post("/checkout", &[("address", "short")])
        .await
        .assert_invalid("address");

    let res = app
        .post("/checkout", &[("address", "Jl. Merdeka 1, Bandung 40111")])
        .await;
    let order = Order::where_eq("user_id", budi.id)
        .first_or_404(app.db())
        .await
        .unwrap();
    res.assert_redirect(&format!("/orders/{}", order.id));
    assert_eq!(order.total, 68_000);
    assert_eq!(order.status, OrderStatus::Pending);
    assert_eq!(stock(&app, kopi.id).await, 3);
    assert_eq!(stock(&app, teh.id).await, 0);
    app.assert_database_count("order_items", 2).await;
    app.assert_database_count("cart_items", 0).await;
    app.get(&format!("/orders/{}", order.id))
        .await
        .assert_view("orders/show.html")
        .assert_see("Thank you! Order #")
        .assert_see("2 × Kopi Susu")
        .assert_see("Waiting for payment");

    // The confirmation goes through the queue; the admin is told at once.
    assert!(app.sent_mail().is_empty());
    app.run_jobs().await;
    app.assert_mail_sent("budi@example.com", &format!("Order #{} received", order.id));
    assert_eq!(budi.unread_notification_count(app.db()).await.unwrap(), 1);
    assert_eq!(boss.unread_notification_count(app.db()).await.unwrap(), 1);
    app.acting_as(&boss);
    app.get("/admin").await.assert_see(&format!(
        "New order <a class=\"rx-link\" href=\"/orders/{}\">",
        order.id
    ));
}

#[renox::test]
async fn checkout_never_oversells() {
    let app = shop().await;
    let kopi = product(&app, "Kopi Susu", 25_000, 5).await;
    let teh = product(&app, "Teh Tarik", 18_000, 1).await;
    let budi = customer(&app, "budi@example.com").await;
    app.acting_as(&budi);
    app.post(
        "/cart",
        &[("product_id", &kopi.id.to_string()), ("quantity", "2")],
    )
    .await;
    app.post(
        "/cart",
        &[("product_id", &teh.id.to_string()), ("quantity", "2")],
    )
    .await;

    app.post("/checkout", &[("address", "Jl. Merdeka 1, Bandung 40111")])
        .await
        .assert_redirect("/cart");
    app.get("/cart")
        .await
        .assert_see("Not enough stock left for: Teh Tarik.");
    // All or nothing: Kopi's stock was taken first, then given back.
    assert_eq!(stock(&app, kopi.id).await, 5);
    assert_eq!(stock(&app, teh.id).await, 1);
    app.assert_database_count("orders", 0).await;
    app.assert_database_count("cart_items", 2).await;

    // An empty cart has nothing to check out.
    renox::db::sql("DELETE FROM cart_items")
        .execute(app.db())
        .await
        .unwrap();
    app.get("/checkout").await.assert_redirect("/cart");
    app.post("/checkout", &[("address", "Jl. Merdeka 1, Bandung 40111")])
        .await
        .assert_redirect("/cart");
}

async fn placed_order(app: &TestApp, buyer: &User, product: &Product, quantity: i64) -> Order {
    app.acting_as(buyer);
    app.post(
        "/cart",
        &[
            ("product_id", &product.id.to_string()),
            ("quantity", &quantity.to_string()),
        ],
    )
    .await;
    app.post("/checkout", &[("address", "Jl. Merdeka 1, Bandung 40111")])
        .await;
    Order::where_eq("user_id", buyer.id)
        .latest()
        .first_or_404(app.db())
        .await
        .unwrap()
}

#[renox::test]
async fn orders_are_seen_by_their_customer_and_admins_only() {
    let app = shop().await;
    let boss = admin(&app).await;
    let kopi = product(&app, "Kopi Susu", 25_000, 5).await;
    let budi = customer(&app, "budi@example.com").await;
    let order = placed_order(&app, &budi, &kopi, 1).await;
    let url = format!("/orders/{}", order.id);

    app.get("/orders")
        .await
        .assert_see(&format!("#{}", order.id));
    let siti = customer(&app, "siti@example.com").await;
    app.acting_as(&siti);
    app.get(&url).await.assert_forbidden();
    app.get("/orders").await.assert_see("No orders yet.");
    app.acting_as(&boss);
    app.get(&url).await.assert_ok();
    app.logout();
    app.get(&url).await.assert_redirect("/login");
}

#[renox::test]
async fn only_admins_get_into_the_admin() {
    let app = shop().await;
    app.get("/admin").await.assert_redirect("/login");
    app.get("/admin/products").await.assert_redirect("/login");

    let budi = customer(&app, "budi@example.com").await;
    app.acting_as(&budi);
    app.get("/").await.assert_dont_see(">Admin</a>");
    app.get("/admin").await.assert_forbidden();
    app.get("/admin/orders").await.assert_forbidden();
    app.post(
        "/admin/products",
        &[("name", "Sneaky"), ("price", "1"), ("stock", "1")],
    )
    .await
    .assert_forbidden();
    app.assert_database_count("products", 0).await;
    // The role is checked before the password confirmation: a customer
    // isn't even asked for it.
    app.delete("/admin/products/1").await.assert_forbidden();

    // `shop:make-admin` promotes a registered user.
    app.kernel()
        .call("shop:make-admin", ["budi@example.com"])
        .await
        .unwrap();
    assert!(
        app.kernel()
            .call("shop:make-admin", ["nobody@example.com"])
            .await
            .is_err()
    );
    app.get("/").await.assert_see(">Admin</a>");
    app.get("/admin").await.assert_ok();
    assert_eq!(budi.roles(app.db()).await.unwrap(), ["admin"]);
    // Promoting twice is harmless, and the role list stays the same. Without
    // the email, the command asks for it.
    renox::prompt::answering(
        ["budi@example.com"],
        app.kernel().call("shop:make-admin", [""; 0]),
    )
    .await
    .unwrap();
    assert_eq!(budi.roles(app.db()).await.unwrap(), ["admin"]);
    let admins = shop::admins(app.db()).await.unwrap();
    assert_eq!(admins.iter().map(|u| u.id).collect::<Vec<_>>(), [budi.id]);

    // Taking the role away closes the admin again.
    budi.remove_role(app.db(), "admin").await.unwrap();
    app.get("/admin").await.assert_forbidden();
    app.get("/").await.assert_dont_see(">Admin</a>");
}

#[renox::test]
async fn order_status_changes_are_audited() {
    let app = shop().await;
    let boss = admin(&app).await;
    let kopi = product(&app, "Kopi Susu", 25_000, 5).await;
    let budi = customer(&app, "budi@example.com").await;
    let order = placed_order(&app, &budi, &kopi, 1).await;
    let status = format!("/admin/orders/{}/status", order.id);

    // A customer can't move an order along, and nothing is recorded.
    app.put(&status, &[("status", "paid")])
        .await
        .assert_forbidden();
    assert!(
        audit::for_subject(app.db(), "orders", order.id, 10)
            .await
            .unwrap()
            .is_empty()
    );

    app.acting_as(&boss);
    app.put(&status, &[("status", "paid")])
        .await
        .assert_status(303);
    // A refused move (paid → cancelled) records nothing either.
    app.put(&status, &[("status", "cancelled")])
        .await
        .assert_status(409);
    let entries = audit::for_subject(app.db(), "orders", order.id, 10)
        .await
        .unwrap();
    assert_eq!(entries.len(), 1);
    let entry = &entries[0];
    assert_eq!(entry.action, "order.status_changed");
    assert_eq!(entry.user_id, Some(boss.id));
    assert_eq!(entry.data, json!({ "from": "pending", "to": "paid" }));

    // The dashboard lists it.
    app.get("/admin").await.assert_ok().assert_see(&format!(
        "moved order <a class=\"rx-link\" href=\"/orders/{0}\">#{0}</a> from pending to paid",
        order.id
    ));
}

#[renox::test]
async fn admins_manage_products_with_photos() {
    let app = shop().await;
    let boss = admin(&app).await;
    let drinks = category(&app, "Drinks").await;
    app.acting_as(&boss);
    // Fill the home page's cache, which a change must clear.
    app.get("/").await.assert_see("No products yet.");

    app.post_multipart(
        "/admin/products",
        &[
            ("name", "Kopi Susu"),
            ("category_id", &drinks.id.to_string()),
            ("description", "Creamy"),
            ("price", "25000"),
            ("stock", "10"),
            ("active", "on"),
        ],
        &[("photo", "kopi.png", PNG)],
    )
    .await
    .assert_redirect("/admin/products");
    let kopi = Product::where_eq("slug", "kopi-susu")
        .first_or_404(app.db())
        .await
        .unwrap();
    assert_eq!(kopi.category_id, Some(drinks.id));
    let photo = kopi.photo.clone().expect("a photo");
    assert!(
        photo.starts_with("public/products/") && photo.ends_with(".png"),
        "{photo}"
    );
    assert!(app.state().storage.exists(&photo).await.unwrap());
    app.get("/")
        .await
        .assert_see("Kopi Susu")
        .assert_see(&app.state().storage.url(&photo));

    // Another product with the same name gets its own address.
    app.post(
        "/admin/products",
        &[("name", "Kopi Susu"), ("price", "1000"), ("stock", "1")],
    )
    .await
    .assert_redirect("/admin/products");
    app.assert_database_has("products", &[("slug", &"kopi-susu-2"), ("active", &false)])
        .await;

    // Everything wrong is reported at once, and a text file isn't a photo.
    let res = app
        .htmx()
        .post_multipart(
            "/admin/products",
            &[
                ("name", ""),
                ("price", "-1"),
                ("stock", "x"),
                ("category_id", "999"),
            ],
            &[("photo", "notes.txt", b"hello")],
        )
        .await;
    for field in ["name", "price", "stock", "category_id", "photo"] {
        res.assert_invalid(field);
    }

    app.put(
        &format!("/admin/products/{}", kopi.id),
        &[
            ("name", "Kopi Susu Gula Aren"),
            ("price", "27000"),
            ("stock", "8"),
            ("active", "on"),
        ],
    )
    .await
    .assert_redirect("/admin/products");
    let kopi = Product::find_or_404(app.db(), kopi.id).await.unwrap();
    assert_eq!(
        (kopi.slug.as_str(), kopi.price),
        ("kopi-susu-gula-aren", 27_000)
    );
    assert_eq!(
        kopi.photo.as_deref(),
        Some(photo.as_str()),
        "no new photo keeps the old one"
    );
    app.get("/admin/products?q=aren&sort=price")
        .await
        .assert_see("Kopi Susu Gula Aren");

    // The list offers delete behind a confirmation sheet; only the sheet's
    // button sends the DELETE.
    let list = app.get("/admin/products").await;
    list.assert_see(&format!("data-rx-open=\"delete-{}\"", kopi.id))
        .assert_see("role=\"alertdialog\"")
        .assert_see(&format!("action=\"/admin/products/{}\"", kopi.id));

    // Deleting asks for the password first: `acting_as` logs in without
    // typing it, so it doesn't count as confirmed (a real login does).
    let destroy = format!("/admin/products/{}", kopi.id);
    app.request()
        .header("referer", "/admin/products")
        .delete(&destroy)
        .await
        .assert_redirect("/confirm-password");
    app.htmx()
        .delete(&destroy)
        .await
        .assert_hx_redirect("/confirm-password");
    app.assert_database_has("products", &[("id", &kopi.id)])
        .await;
    app.htmx()
        .post("/confirm-password", &[("password", "wrong")])
        .await
        .assert_invalid("password");
    // After the confirmation the admin is back on the list the DELETE was
    // sent from, and presses Delete again.
    app.post("/confirm-password", &[("password", "password123")])
        .await
        .assert_redirect("/admin/products");
    app.delete(&destroy)
        .await
        .assert_redirect("/admin/products");
    app.assert_database_missing("products", &[("id", &kopi.id)])
        .await;
    assert!(!app.state().storage.exists(&photo).await.unwrap());
    // The list the redirect leads to says so, once, in a toast.
    app.get("/admin/products")
        .await
        .assert_see("“Kopi Susu Gula Aren” deleted.");
    app.get("/").await.assert_dont_see("Gula Aren");
}

#[renox::test]
async fn admins_move_orders_along_and_customers_hear_about_it() {
    let app = shop().await;
    let boss = admin(&app).await;
    let kopi = product(&app, "Kopi Susu", 25_000, 5).await;
    let budi = customer(&app, "budi@example.com").await;
    let order = placed_order(&app, &budi, &kopi, 2).await;
    app.run_jobs().await; // the confirmation
    let status = format!("/admin/orders/{}/status", order.id);

    app.acting_as(&boss);
    app.get("/admin/orders?status=pending")
        .await
        .assert_see("Mark paid");
    app.put(&status, &[("status", "shipped")])
        .await
        .assert_status(409);
    app.put(&status, &[("status", "lost")])
        .await
        .assert_status(303);
    app.put(&status, &[("status", "paid")])
        .await
        .assert_status(303);
    app.put(&status, &[("status", "shipped")])
        .await
        .assert_status(303);
    assert_eq!(
        Order::find_or_404(app.db(), order.id).await.unwrap().status,
        OrderStatus::Shipped
    );
    app.run_jobs().await;
    app.assert_mail_sent(
        "budi@example.com",
        &format!("Order #{} is on its way", order.id),
    );

    // Cancelling a pending order gives its stock back.
    let second = placed_order(&app, &budi, &kopi, 3).await;
    assert_eq!(stock(&app, kopi.id).await, 0);
    app.acting_as(&boss);
    app.put(
        &format!("/admin/orders/{}/status", second.id),
        &[("status", "cancelled")],
    )
    .await;
    assert_eq!(stock(&app, kopi.id).await, 3);
}

async fn status(app: &TestApp, order_id: i64) -> OrderStatus {
    Order::find_or_404(app.db(), order_id).await.unwrap().status
}

#[renox::test]
async fn unpaid_orders_are_cancelled_after_three_days() {
    let app = shop().await;
    let kopi = product(&app, "Kopi Susu", 25_000, 5).await;
    let budi = customer(&app, "budi@example.com").await;
    let old = placed_order(&app, &budi, &kopi, 2).await;
    // Two days later, another order; then two more days pass.
    app.travel(2 * DAY);
    let recent = placed_order(&app, &budi, &kopi, 1).await;
    app.travel(2 * DAY);

    // What the scheduler runs at 03:00 (`schedule:run cancel-unpaid-orders`),
    // on the moved clock.
    app.at_travelled_time(app.kernel().run_scheduled("cancel-unpaid-orders"))
        .await
        .unwrap();
    assert_eq!(status(&app, old.id).await, OrderStatus::Cancelled);
    assert_eq!(status(&app, recent.id).await, OrderStatus::Pending);
    assert_eq!(stock(&app, kopi.id).await, 4);
    assert_eq!(
        app.at_travelled_time(shop::app::orders::cancel_unpaid(app.state()))
            .await
            .unwrap(),
        0,
        "only once"
    );
    // A day later the second one goes too.
    app.travel(DAY);
    assert_eq!(
        app.at_travelled_time(shop::app::orders::cancel_unpaid(app.state()))
            .await
            .unwrap(),
        1
    );
    assert_eq!(stock(&app, kopi.id).await, 5);
}

#[renox::test]
async fn checkout_emits_order_placed_and_notifies_without_sending() {
    let app = shop().await;
    let boss = admin(&app).await;
    let kopi = product(&app, "Kopi Susu", 25_000, 5).await;
    let budi = customer(&app, "budi@example.com").await;

    // The listener runs, but its notifications are only recorded: no mail,
    // no jobs, no database rows.
    app.fake_notifications();
    placed_order(&app, &budi, &kopi, 1).await;
    app.assert_notified(&budi, "order-confirmation")
        .assert_notified(&boss, "new-order");
    assert!(app.queued_jobs().await.is_empty());
    assert_eq!(boss.unread_notification_count(app.db()).await.unwrap(), 0);

    // With events faked the listener doesn't run at all: the checkout
    // handler is tested alone.
    app.fake_events();
    let second = placed_order(&app, &budi, &kopi, 1).await;
    app.assert_emitted::<OrderPlaced>(|event| event.order_id == second.id);
    assert_eq!(app.emitted::<OrderPlaced>().len(), 1, "only while faked");
    assert_eq!(app.notifications().len(), 2, "no listener, no new ones");
}

#[renox::test]
async fn the_shop_speaks_indonesian_with_plurals() {
    let app = shop().await;
    product(&app, "Kopi Susu", 25_000, 1).await;
    product(&app, "Kopi Hitam", 15_000, 7).await;
    app.get("/language/id").await;
    app.get("/products")
        .await
        .assert_see("<html lang=\"id\">")
        .assert_see("2 produk ditemukan")
        .assert_see(">Daftar<")
        .assert_see("Rp 25.000");
    app.get("/products?q=susu")
        .await
        .assert_see("Satu produk ditemukan");
    app.get("/products?q=teh")
        .await
        .assert_see("0 produk ditemukan");

    let budi = customer(&app, "budi@example.com").await;
    app.acting_as(&budi);
    app.get("/products/kopi-susu")
        .await
        .assert_see("Tinggal satu");
    app.get("/products/kopi-hitam").await.assert_see("Stok 7");
    app.htmx()
        .post("/checkout", &[("address", "")])
        .await
        .assert_invalid("address")
        .assert_see("Alamat wajib diisi.");
    app.get("/language/xx").await; // unknown: ignored
    app.get("/").await.assert_see("Segar dari sangrai");
}

#[renox::test]
async fn the_shop_runs_the_same_with_database_sessions() {
    // SESSION_DRIVER=database: login, the flashed message and the chosen
    // language live in the `sessions` table instead of the cookie.
    let app = TestApp::with_config(shop::app(), |c| c.session_driver = "database".into()).await;
    let kopi = product(&app, "Kopi Susu", 25_000, 5).await;
    let budi = customer(&app, "budi@example.com").await;
    app.acting_as(&budi);
    app.get("/language/id").await;
    app.post(
        "/cart",
        &[("product_id", &kopi.id.to_string()), ("quantity", "2")],
    )
    .await;
    app.get("/cart")
        .await
        .assert_see("Kopi Susu masuk keranjang.")
        .assert_see("Rp 50.000");
    assert_eq!(app.session_get::<String>("_locale").as_deref(), Some("id"));
}

#[renox::test]
async fn error_pages_keep_the_shop_layout() {
    // With APP_DEBUG an undefined value in the layout is an error, so this
    // also checks that error pages get the shared `cart_count`.
    let app = TestApp::with_config(shop::app(), |c| c.debug = true).await;
    let budi = customer(&app, "budi@example.com").await;
    app.acting_as(&budi);
    let res = app.get("/products/no-such-coffee").await;
    res.assert_not_found()
        .assert_see(r#"class="bar__brand""#)
        .assert_see("Cart (0)")
        .assert_see("That page isn&#39;t here.");
}
