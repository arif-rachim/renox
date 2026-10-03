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
    User::register(app.db(), "Ben", email, "password123")
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
    let mut coffee = product(&app, "Coffee Latte", 25_000, 5).await;
    coffee.category_id = Some(drinks.id);
    coffee.save(app.db()).await.unwrap();
    product(&app, "Milk Tea", 18_000, 5).await;
    product(&app, "Black Coffee", 15_000, 0).await;
    let mut hidden = product(&app, "Secret Coffee", 99_000, 5).await;
    hidden.active = false;
    hidden.save(app.db()).await.unwrap();

    // The home page lists what can be bought.
    app.get("/")
        .await
        .assert_ok()
        .assert_view("catalog/home.html")
        .assert_see("Coffee Latte")
        .assert_see("Rp 25,000")
        .assert_dont_see("Black Coffee")
        .assert_dont_see("Secret Coffee");

    let page = app.get("/products?q=coffee&sort=price_asc").await;
    page.assert_ok()
        .assert_see("2 products found")
        .assert_dont_see("Milk Tea")
        .assert_dont_see("Secret Coffee");
    let text = page.text();
    assert!(
        text.find("Black Coffee") < text.find("Coffee Latte"),
        "cheapest first"
    );

    app.get("/products?category=drinks")
        .await
        .assert_see("One product found")
        .assert_see("Coffee Latte");
    app.get("/products?q=nothing")
        .await
        .assert_see("0 products found");
    app.get("/products?category=missing")
        .await
        .assert_not_found();

    // The search box asks with htmx and gets only the results.
    let results = app.htmx().get("/products?q=tea").await;
    results
        .assert_ok()
        .assert_see("Milk Tea")
        .assert_dont_see("<header");
}

#[renox::test]
async fn product_pages_have_seo_tags() {
    let app = shop().await;
    let coffee = product(&app, "Coffee Latte", 25_000, 5).await;
    app.get(&format!("/products/{}", coffee.slug))
        .await
        .assert_ok()
        .assert_see("<title>Coffee Latte · ")
        .assert_see(r#"<meta property="og:type" content="product">"#)
        .assert_see("About Coffee Latte")
        .assert_see("Log in to buy");
    app.get("/products/no-such-thing").await.assert_not_found();

    let sitemap = app.get("/sitemap.xml").await;
    sitemap.assert_ok().assert_see("/products/coffee-latte");
}

#[renox::test]
async fn the_cart_adds_up_and_stays_private() {
    let app = shop().await;
    let coffee = product(&app, "Coffee Latte", 25_000, 5).await;
    let id = coffee.id.to_string();

    app.post("/cart", &[("product_id", &id), ("quantity", "1")])
        .await
        .assert_redirect("/login");

    let ben = customer(&app, "ben@example.com").await;
    app.acting_as(&ben);
    app.post("/cart", &[("product_id", &id), ("quantity", "1")])
        .await;
    app.post("/cart", &[("product_id", &id), ("quantity", "2")])
        .await;
    app.get("/cart")
        .await
        .assert_see(r#"<span>Cart</span><span class="rx-nav-badge">3</span>"#)
        .assert_see("Rp 75,000");
    app.htmx()
        .post("/cart", &[("product_id", &id), ("quantity", "0")])
        .await
        .assert_invalid("quantity");
    app.htmx()
        .post("/cart", &[("product_id", "999"), ("quantity", "1")])
        .await
        .assert_invalid("product_id");

    let item: i64 = renox::db::sql("SELECT id FROM cart_items WHERE user_id = ?")
        .bind(ben.id)
        .scalar(app.db())
        .await
        .unwrap();
    app.patch(&format!("/cart/{item}"), &[("quantity", "4")])
        .await
        .assert_redirect("/cart");
    app.get("/cart")
        .await
        .assert_see(r#"<span>Cart</span><span class="rx-nav-badge">4</span>"#);

    // Someone else can't change or remove Ben's item.
    let sam = customer(&app, "sam@example.com").await;
    app.acting_as(&sam);
    app.patch(&format!("/cart/{item}"), &[("quantity", "1")])
        .await
        .assert_not_found();
    app.delete(&format!("/cart/{item}")).await;
    app.assert_database_has("cart_items", &[("id", &item), ("quantity", &4)])
        .await;

    app.acting_as(&ben);
    app.delete(&format!("/cart/{item}"))
        .await
        .assert_redirect("/cart");
    app.get("/cart").await.assert_see("Your cart is empty.");
}

#[renox::test]
async fn checkout_takes_the_stock_and_confirms_by_mail() {
    let app = shop().await;
    let boss = admin(&app).await;
    let coffee = product(&app, "Coffee Latte", 25_000, 5).await;
    let tea = product(&app, "Milk Tea", 18_000, 1).await;
    let ben = customer(&app, "ben@example.com").await;
    app.acting_as(&ben);
    for (id, quantity) in [(coffee.id, "2"), (tea.id, "1")] {
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
        .assert_see("Rp 68,000");
    app.htmx()
        .post("/checkout", &[("address", "short")])
        .await
        .assert_invalid("address");

    let res = app
        .post(
            "/checkout",
            &[("address", "1 Main Street, Springfield 40111")],
        )
        .await;
    let order = Order::where_eq("user_id", ben.id)
        .first_or_404(app.db())
        .await
        .unwrap();
    res.assert_redirect(&format!("/orders/{}", order.id));
    assert_eq!(order.total, 68_000);
    assert_eq!(order.status, OrderStatus::Pending);
    assert_eq!(stock(&app, coffee.id).await, 3);
    assert_eq!(stock(&app, tea.id).await, 0);
    app.assert_database_count("order_items", 2).await;
    app.assert_database_count("cart_items", 0).await;
    app.get(&format!("/orders/{}", order.id))
        .await
        .assert_view("orders/show.html")
        .assert_see("Thank you! Order #")
        .assert_see(">Coffee Latte</dd>")
        .assert_see("2<span class=\"rx-entry__affix\">× Rp 25,000</span>")
        .assert_see("Waiting for payment");

    // The confirmation goes through the queue; the admin is told at once.
    assert!(app.sent_mail().is_empty());
    app.run_jobs().await;
    app.assert_mail_sent("ben@example.com", &format!("Order #{} received", order.id));
    assert_eq!(ben.unread_notification_count(app.db()).await.unwrap(), 1);
    assert_eq!(boss.unread_notification_count(app.db()).await.unwrap(), 1);
    app.acting_as(&boss);
    // On the dashboard's second tab: every panel is on the page, the
    // first one shown and the others hidden until their tab is picked.
    app.get("/admin")
        .await
        .assert_see(r#"role="tablist" aria-label="Dashboard""#)
        .assert_see(r#"id="dashboard-panel-notifications" aria-labelledby="dashboard-tab-notifications" tabindex="0" hidden"#)
        .assert_see(&format!(
            "New order <a class=\"rx-link\" href=\"/orders/{}\">",
            order.id
        ));
}

#[renox::test]
async fn checkout_never_oversells() {
    let app = shop().await;
    let coffee = product(&app, "Coffee Latte", 25_000, 5).await;
    let tea = product(&app, "Milk Tea", 18_000, 1).await;
    let ben = customer(&app, "ben@example.com").await;
    app.acting_as(&ben);
    app.post(
        "/cart",
        &[("product_id", &coffee.id.to_string()), ("quantity", "2")],
    )
    .await;
    app.post(
        "/cart",
        &[("product_id", &tea.id.to_string()), ("quantity", "2")],
    )
    .await;

    app.post(
        "/checkout",
        &[("address", "1 Main Street, Springfield 40111")],
    )
    .await
    .assert_redirect("/cart");
    app.get("/cart")
        .await
        .assert_see("Not enough stock left for: Milk Tea.");
    // All or nothing: Coffee Latte's stock was taken first, then given back.
    assert_eq!(stock(&app, coffee.id).await, 5);
    assert_eq!(stock(&app, tea.id).await, 1);
    app.assert_database_count("orders", 0).await;
    app.assert_database_count("cart_items", 2).await;

    // An empty cart has nothing to check out.
    renox::db::sql("DELETE FROM cart_items")
        .execute(app.db())
        .await
        .unwrap();
    app.get("/checkout").await.assert_redirect("/cart");
    app.post(
        "/checkout",
        &[("address", "1 Main Street, Springfield 40111")],
    )
    .await
    .assert_redirect("/cart");
}

#[renox::test]
async fn a_pickup_needs_no_address() {
    let app = shop().await;
    let coffee = product(&app, "Coffee Latte", 25_000, 5).await;
    let ben = customer(&app, "ben@example.com").await;
    app.acting_as(&ben);
    app.post(
        "/cart",
        &[("product_id", &coffee.id.to_string()), ("quantity", "1")],
    )
    .await;
    // The form shows both choices, and the address only for the courier.
    app.get("/checkout")
        .await
        .assert_see(r#"name="delivery" value="courier" checked required"#)
        .assert_see(r#"data-rx-show-when="delivery""#);
    // The courier still needs an address.
    app.htmx()
        .post("/checkout", &[("delivery", "courier")])
        .await
        .assert_invalid("address");
    // A pickup sends none (the hidden field is disabled): the store's address
    // goes on the order.
    let res = app.post("/checkout", &[("delivery", "pickup")]).await;
    let order = Order::where_eq("user_id", ben.id)
        .first_or_404(app.db())
        .await
        .unwrap();
    res.assert_redirect(&format!("/orders/{}", order.id));
    assert_eq!(
        order.address,
        "Pick up at the store: 12 Market Street, Springfield"
    );
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
    app.post(
        "/checkout",
        &[("address", "1 Main Street, Springfield 40111")],
    )
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
    let coffee = product(&app, "Coffee Latte", 25_000, 5).await;
    let ben = customer(&app, "ben@example.com").await;
    let order = placed_order(&app, &ben, &coffee, 1).await;
    let url = format!("/orders/{}", order.id);

    app.get("/orders")
        .await
        .assert_see(&format!("#{}", order.id));
    let sam = customer(&app, "sam@example.com").await;
    app.acting_as(&sam);
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

    let ben = customer(&app, "ben@example.com").await;
    app.acting_as(&ben);
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
        .call("shop:make-admin", ["ben@example.com"])
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
    assert_eq!(ben.roles(app.db()).await.unwrap(), ["admin"]);
    // Promoting twice is harmless, and the role list stays the same. Without
    // the email, the command asks for it.
    renox::prompt::answering(
        ["ben@example.com"],
        app.kernel().call("shop:make-admin", [""; 0]),
    )
    .await
    .unwrap();
    assert_eq!(ben.roles(app.db()).await.unwrap(), ["admin"]);
    let admins = shop::admins(app.db()).await.unwrap();
    assert_eq!(admins.iter().map(|u| u.id).collect::<Vec<_>>(), [ben.id]);

    // Taking the role away closes the admin again.
    ben.remove_role(app.db(), "admin").await.unwrap();
    app.get("/admin").await.assert_forbidden();
    app.get("/").await.assert_dont_see(">Admin</a>");
}

#[renox::test]
async fn order_status_changes_are_audited() {
    let app = shop().await;
    let boss = admin(&app).await;
    let coffee = product(&app, "Coffee Latte", 25_000, 5).await;
    let ben = customer(&app, "ben@example.com").await;
    let order = placed_order(&app, &ben, &coffee, 1).await;
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
            ("name", "Coffee Latte"),
            ("category_id", &drinks.id.to_string()),
            ("description", "Creamy"),
            ("price", "25000"),
            ("stock", "10"),
            ("active", "on"),
        ],
        &[("photo", "coffee.png", PNG)],
    )
    .await
    .assert_redirect("/admin/products");
    let coffee = Product::where_eq("slug", "coffee-latte")
        .first_or_404(app.db())
        .await
        .unwrap();
    assert_eq!(coffee.category_id, Some(drinks.id));
    // The admin nav marks the current section only (`route_is(…)`).
    for (page, section) in [
        ("/admin/products?q=coffee", "Products"),
        ("/admin", "Dashboard"),
    ] {
        let html = app.get(page).await.text();
        // The admin's nav (the kit's `link_tabs`), not the dashboard's period filter.
        let start = html.find(r#"aria-label="Admin""#).expect("the admin nav");
        let end = html[start..].find("</nav>").unwrap() + start;
        let current: Vec<&str> = html[start..end]
            .split("<a ")
            .filter(|link| link.contains(r#"aria-current="page""#))
            .collect();
        assert_eq!(current.len(), 1, "{page}");
        assert!(
            current[0].contains(&format!(">{section}<")),
            "{}",
            current[0]
        );
    }
    let photo = coffee.photo.clone().expect("a photo");
    assert!(
        photo.starts_with("public/products/") && photo.ends_with(".png"),
        "{photo}"
    );
    assert!(app.state().storage.exists(&photo).await.unwrap());
    app.get("/")
        .await
        .assert_see("Coffee Latte")
        .assert_see(&app.state().storage.url(&photo));

    // Another product with the same name gets its own address.
    app.post(
        "/admin/products",
        &[("name", "Coffee Latte"), ("price", "1000"), ("stock", "1")],
    )
    .await
    .assert_redirect("/admin/products");
    app.assert_database_has(
        "products",
        &[("slug", &"coffee-latte-2"), ("active", &false)],
    )
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
        &format!("/admin/products/{}", coffee.id),
        &[
            ("name", "Coffee Latte Brown Sugar"),
            ("price", "27000"),
            ("stock", "8"),
            ("active", "on"),
        ],
    )
    .await
    .assert_redirect("/admin/products");
    let coffee = Product::find_or_404(app.db(), coffee.id).await.unwrap();
    assert_eq!(
        (coffee.slug.as_str(), coffee.price),
        ("coffee-latte-brown-sugar", 27_000)
    );
    assert_eq!(
        coffee.photo.as_deref(),
        Some(photo.as_str()),
        "no new photo keeps the old one"
    );
    app.get("/admin/products?q=sugar&sort=price")
        .await
        .assert_see("Coffee Latte Brown Sugar");

    // The list offers delete behind a confirmation sheet; only the sheet's
    // button sends the DELETE.
    let list = app.get("/admin/products").await;
    list.assert_see(&format!("data-rx-open=\"delete-{}\"", coffee.id))
        .assert_see("role=\"alertdialog\"")
        .assert_see(&format!("action=\"/admin/products/{}\"", coffee.id));

    // Deleting asks for the password first: `acting_as` logs in without
    // typing it, so it doesn't count as confirmed (a real login does).
    let destroy = format!("/admin/products/{}", coffee.id);
    app.request()
        .header("referer", "/admin/products")
        .delete(&destroy)
        .await
        .assert_redirect("/confirm-password");
    app.htmx()
        .delete(&destroy)
        .await
        .assert_hx_redirect("/confirm-password");
    app.assert_database_has("products", &[("id", &coffee.id)])
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
    app.assert_database_missing("products", &[("id", &coffee.id)])
        .await;
    assert!(!app.state().storage.exists(&photo).await.unwrap());
    // The list the redirect leads to says so, once, in a toast.
    app.get("/admin/products")
        .await
        .assert_see("“Coffee Latte Brown Sugar” deleted.");
    app.get("/").await.assert_dont_see("Brown Sugar");
}

#[renox::test]
async fn admins_move_orders_along_and_customers_hear_about_it() {
    let app = shop().await;
    let boss = admin(&app).await;
    let coffee = product(&app, "Coffee Latte", 25_000, 5).await;
    let ben = customer(&app, "ben@example.com").await;
    let order = placed_order(&app, &ben, &coffee, 2).await;
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
        "ben@example.com",
        &format!("Order #{} is on its way", order.id),
    );

    // Cancelling a pending order gives its stock back.
    let second = placed_order(&app, &ben, &coffee, 3).await;
    assert_eq!(stock(&app, coffee.id).await, 0);
    app.acting_as(&boss);
    app.put(
        &format!("/admin/orders/{}/status", second.id),
        &[("status", "cancelled")],
    )
    .await;
    assert_eq!(stock(&app, coffee.id).await, 3);
}

#[renox::test]
async fn the_bell_tells_customers_and_admins_about_orders() {
    let app = shop().await;
    let boss = admin(&app).await;
    let coffee = product(&app, "Coffee Latte", 25_000, 5).await;
    let ben = customer(&app, "ben@example.com").await;
    let order = placed_order(&app, &ben, &coffee, 2).await;

    // The customer: their order, what to pay, a link to it.
    app.get("/orders").await.assert_see(
        r#"<span class="rx-bell__badge" data-rx-bell-count aria-hidden="true">1</span>"#,
    );
    app.get("/notifications")
        .await
        .assert_see(&format!(">Order #{} received</button>", order.id))
        .assert_see("Pay Rp 50,000 by bank transfer within 3 days.");
    let id = ben.notifications(app.db(), 1).await.unwrap()[0].id;
    app.post(&format!("/notifications/{id}/open"), &[])
        .await
        .assert_redirect(&format!("/orders/{}", order.id));

    // Every admin: the new order, with its total and address.
    app.acting_as(&boss);
    app.get("/notifications")
        .await
        .assert_see(&format!(">New order #{}</button>", order.id))
        .assert_see("Rp 50,000, to 1 Main Street, Springfield 40111")
        .assert_see(r#"href="/admin/orders?status=pending">All orders</a>"#);
}

#[renox::test]
async fn the_dashboard_shows_figures_and_charts_for_a_period() {
    let app = shop().await;
    let boss = admin(&app).await;
    let coffee = product(&app, "Coffee Latte", 25_000, 9).await;
    let ben = customer(&app, "ben@example.com").await;
    let paid = placed_order(&app, &ben, &coffee, 2).await;
    placed_order(&app, &ben, &coffee, 1).await; // stays pending: not revenue

    app.acting_as(&boss);
    app.put(
        &format!("/admin/orders/{}/status", paid.id),
        &[("status", "paid")],
    )
    .await
    .assert_status(303);
    app.get("/admin?period=7d")
        .await
        .assert_ok()
        .assert_see(r#"href="?period=7d" aria-current="page">7 days</a>"#)
        .assert_see(r#"<p class="rx-stat__label">Revenue</p>"#)
        .assert_see(r#"<p class="rx-stat__value">Rp 50,000</p>"#)
        .assert_see(r#"<p class="rx-stat__value">1</p>"#)
        // Nothing the week before: no delta, only the figure.
        .assert_dont_see("rx-stat__change")
        .assert_see(r#"<figure class="rx-chart rx-chart--line""#)
        .assert_see("This period")
        .assert_see(r#"<figure class="rx-chart rx-chart--bar""#)
        .assert_see(r#"hx-get="/admin/widgets/statuses" hx-trigger="load, every 60s""#);
    app.htmx()
        .get("/admin/widgets/statuses")
        .await
        .assert_see(r#"<figure class="rx-chart rx-chart--doughnut""#)
        .assert_see(r#"<span class="rx-chart__name">pending</span><span class="rx-chart__value">1 <span class="rx-chart__share">50.0%</span>"#);

    // A month later the week is empty; the 90 days still hold it.
    app.travel(std::time::Duration::from_secs(30 * 86_400));
    app.acting_as(&boss); // the session ran out meanwhile
    app.get("/admin?period=7d")
        .await
        .assert_see(r#"<p class="rx-stat__value">Rp 0</p>"#);
    app.get("/admin?period=90d")
        .await
        .assert_see(r#"<p class="rx-stat__value">Rp 50,000</p>"#);

    app.acting_as(&ben);
    app.get("/admin/widgets/statuses").await.assert_forbidden();
}

async fn status(app: &TestApp, order_id: i64) -> OrderStatus {
    Order::find_or_404(app.db(), order_id).await.unwrap().status
}

#[renox::test]
async fn unpaid_orders_are_cancelled_after_three_days() {
    let app = shop().await;
    let coffee = product(&app, "Coffee Latte", 25_000, 5).await;
    let ben = customer(&app, "ben@example.com").await;
    let old = placed_order(&app, &ben, &coffee, 2).await;
    // Two days later, another order; then two more days pass.
    app.travel(2 * DAY);
    let recent = placed_order(&app, &ben, &coffee, 1).await;
    app.travel(2 * DAY);

    // What the scheduler runs at 03:00 (`schedule:run cancel-unpaid-orders`),
    // on the moved clock.
    app.at_travelled_time(app.kernel().run_scheduled("cancel-unpaid-orders"))
        .await
        .unwrap();
    assert_eq!(status(&app, old.id).await, OrderStatus::Cancelled);
    assert_eq!(status(&app, recent.id).await, OrderStatus::Pending);
    assert_eq!(stock(&app, coffee.id).await, 4);
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
    assert_eq!(stock(&app, coffee.id).await, 5);
}

#[renox::test]
async fn checkout_emits_order_placed_and_notifies_without_sending() {
    let app = shop().await;
    let boss = admin(&app).await;
    let coffee = product(&app, "Coffee Latte", 25_000, 5).await;
    let ben = customer(&app, "ben@example.com").await;

    // The listener runs, but its notifications are only recorded: no mail,
    // no jobs, no database rows.
    app.fake_notifications();
    placed_order(&app, &ben, &coffee, 1).await;
    app.assert_notified(&ben, "order-confirmation")
        .assert_notified(&boss, "new-order");
    assert!(app.queued_jobs().await.is_empty());
    assert_eq!(boss.unread_notification_count(app.db()).await.unwrap(), 0);

    // With events faked the listener doesn't run at all: the checkout
    // handler is tested alone.
    app.fake_events();
    let second = placed_order(&app, &ben, &coffee, 1).await;
    app.assert_emitted::<OrderPlaced>(|event| event.order_id == second.id);
    assert_eq!(app.emitted::<OrderPlaced>().len(), 1, "only while faked");
    assert_eq!(app.notifications().len(), 2, "no listener, no new ones");
}

#[renox::test]
async fn the_shop_speaks_spanish_with_plurals() {
    let app = shop().await;
    product(&app, "Coffee Latte", 25_000, 1).await;
    product(&app, "Black Coffee", 15_000, 7).await;
    app.get("/language/es").await;
    app.get("/products")
        .await
        .assert_see("<html lang=\"es\">")
        .assert_see("2 productos encontrados")
        .assert_see(">Registrarse<")
        .assert_see("Rp 25.000");
    app.get("/products?q=latte")
        .await
        .assert_see("Un producto encontrado");
    app.get("/products?q=tea")
        .await
        .assert_see("0 productos encontrados");
    app.get("/login")
        .await
        .assert_see("Iniciar sesión")
        .assert_see("Recordarme");

    let ben = customer(&app, "ben@example.com").await;
    app.acting_as(&ben);
    app.get("/products/coffee-latte")
        .await
        .assert_see("Solo queda uno");
    app.get("/products/black-coffee")
        .await
        .assert_see("7 en stock");
    app.htmx()
        .post("/checkout", &[("address", "")])
        .await
        .assert_invalid("address")
        .assert_json_path("errors.address.0", "El campo dirección es obligatorio.");
    app.get("/language/xx").await; // unknown: ignored
    app.get("/").await.assert_see("Recién tostado");
}

#[renox::test]
async fn the_shop_runs_the_same_with_database_sessions() {
    // SESSION_DRIVER=database: login, the flashed message and the chosen
    // language live in the `sessions` table instead of the cookie.
    let app = TestApp::with_config(shop::app(), |c| {
        c.session_driver = renox::SessionDriver::Database
    })
    .await;
    let coffee = product(&app, "Coffee Latte", 25_000, 5).await;
    let ben = customer(&app, "ben@example.com").await;
    app.acting_as(&ben);
    app.get("/language/es").await;
    app.post(
        "/cart",
        &[("product_id", &coffee.id.to_string()), ("quantity", "2")],
    )
    .await;
    app.get("/cart")
        .await
        .assert_see("Coffee Latte está en tu carrito.")
        .assert_see("Rp 50.000");
    assert_eq!(app.session_get::<String>("_locale").as_deref(), Some("es"));
}

#[renox::test]
async fn error_pages_keep_the_shop_layout() {
    // With APP_DEBUG an undefined value in the layout is an error, so this
    // also checks that error pages get the shared `cart_count`.
    let app = TestApp::with_config(shop::app(), |c| c.debug = true).await;
    let ben = customer(&app, "ben@example.com").await;
    app.acting_as(&ben);
    let res = app.get("/products/no-such-coffee").await;
    res.assert_not_found()
        .assert_see(r#"class="rx-navbar__brand""#)
        .assert_see("<span>Cart</span>")
        .assert_see("That page isn&#39;t here.");
}

#[renox::test]
async fn the_home_page_shows_recently_viewed_products() {
    let app = shop().await;
    for (name, stock) in [
        ("Coffee A", 5),
        ("Coffee B", 5),
        ("Coffee C", 5),
        ("Coffee D", 5),
        ("Coffee E", 5),
    ] {
        product(&app, name, 10_000, stock).await;
    }
    app.get("/").await.assert_dont_see("Recently viewed");
    for slug in [
        "coffee-a", "coffee-b", "coffee-c", "coffee-a", "coffee-d", "coffee-e",
    ] {
        app.get(&format!("/products/{slug}")).await.assert_ok();
    }
    let home = app.get("/").await.text();
    let recent = home
        .split("Recently viewed")
        .nth(1)
        .and_then(|rest| rest.split("New in the shop").next())
        .expect("a recently viewed section");
    // Newest first, each once, four at most (`{% break %}`).
    let order: Vec<usize> = ["Coffee E", "Coffee D", "Coffee A", "Coffee C"]
        .iter()
        .map(|name| recent.find(&format!(">{name}<")).expect(name))
        .collect();
    assert!(order.windows(2).all(|w| w[0] < w[1]), "{order:?}");
    assert!(!recent.contains(">Coffee B<"), "only four");
    // The session keeps eight ids at most.
    for _ in 0..6 {
        app.get("/products/coffee-b").await;
    }
    assert!(
        app.session_get::<Vec<i64>>("recently_viewed")
            .unwrap()
            .len()
            <= 8
    );
}

#[renox::test]
async fn stock_texts_use_plural_ranges_and_sold_out_cards_are_marked() {
    let app = shop().await;
    product(&app, "Coffee Latte", 25_000, 3).await;
    product(&app, "Black Coffee", 15_000, 0).await;
    let ben = customer(&app, "ben@example.com").await;
    app.acting_as(&ben);
    app.get("/products/coffee-latte")
        .await
        .assert_see("Only 3 left");
    app.get("/language/es").await;
    app.get("/products/coffee-latte")
        .await
        .assert_see("Solo quedan 3");
    app.get("/products")
        .await
        .assert_see(r#"class="rx-media-card rx-media-card--dimmed""#)
        .assert_see(r#"<article class="rx-media-card">"#);
}

#[renox::test]
async fn the_category_select_searches_adds_and_renames() {
    let app = shop().await;
    let drinks = category(&app, "Drinks").await;
    category(&app, "Snacks").await;
    let mut coffee = product(&app, "Coffee Latte", 25_000, 5).await;
    coffee.category_id = Some(drinks.id);
    coffee.save(app.db()).await.unwrap();

    // Customers can't reach the options.
    let ben = customer(&app, "ben@example.com").await;
    app.acting_as(&ben);
    app.get("/admin/categories/options?q=dr")
        .await
        .assert_forbidden();

    let boss = admin(&app).await;
    app.acting_as(&boss);
    // The form has only the current category, and where to ask for more.
    app.get(&format!("/admin/products/{}/edit", coffee.id))
        .await
        .assert_see(r#"data-rx-options-url="/admin/categories/options""#)
        .assert_see("data-rx-editable")
        .assert_see(&format!(
            r#"<option value="{}" selected>Drinks</option>"#,
            drinks.id
        ))
        .assert_dont_see(">Snacks</option>");

    // Searching, and looking labels up.
    let res = app.get("/admin/categories/options?q=dri").await;
    res.assert_ok();
    let found: renox::serde_json::Value = res.json();
    assert_eq!(
        found,
        renox::serde_json::json!([{"value": drinks.id.to_string(), "label": "Drinks"}])
    );
    let res = app
        .get(&format!(
            "/admin/categories/options?values={}&values=x",
            drinks.id
        ))
        .await;
    let found: renox::serde_json::Value = res.json();
    assert_eq!(found[0]["label"], "Drinks");

    // Adding: the new category comes back to be chosen; a taken name is a 422.
    let res = app
        .htmx()
        .post("/admin/categories/options", &[("label", "  Juice ")])
        .await;
    res.assert_ok();
    let added: renox::serde_json::Value = res.json();
    assert_eq!(added["label"], "Juice");
    let juice = Category::where_eq("slug", "juice")
        .first_or_404(app.db())
        .await
        .unwrap();
    assert_eq!(added["value"], juice.id.to_string());
    app.htmx()
        .post("/admin/categories/options", &[("label", "Juice")])
        .await
        .assert_status(422)
        .assert_invalid("label");

    // Renaming the chosen one: the name changes, the slug (in links) stays.
    let res = app
        .htmx()
        .put(
            "/admin/categories/options",
            &[
                ("value", drinks.id.to_string().as_str()),
                ("label", "Coffee & Tea"),
            ],
        )
        .await;
    res.assert_ok();
    let renamed: renox::serde_json::Value = res.json();
    assert_eq!(renamed["label"], "Coffee & Tea");
    let drinks = Category::find_or_404(app.db(), drinks.id).await.unwrap();
    assert_eq!(
        (drinks.name.as_str(), drinks.slug.as_str()),
        ("Coffee & Tea", "drinks")
    );
    // Not to another category's name.
    app.htmx()
        .put(
            "/admin/categories/options",
            &[
                ("value", drinks.id.to_string().as_str()),
                ("label", "Snacks"),
            ],
        )
        .await
        .assert_status(422);
}

#[renox::test]
async fn admins_adjust_stock_from_the_list() {
    let app = shop().await;
    let boss = admin(&app).await;
    let coffee = product(&app, "Coffee", 20000, 3).await;
    app.acting_as(&boss);
    // The list has the action's button and its sheet, with a field per row.
    app.get("/admin/products")
        .await
        .assert_see(&format!(r#"data-rx-open="stock-{}""#, coffee.id))
        .assert_see(&format!(r#"id="stock-change-{}""#, coffee.id))
        .assert_see(r#"aria-label="Edit Coffee""#);
    let url = format!("/admin/products/{}/stock", coffee.id);
    // Sent from the sheet with htmx: a 422 keeps it open with the message.
    app.htmx()
        .put(&url, &[("change", "-5")])
        .await
        .assert_status(422)
        .assert_json_path("errors.change.0", "Only 3 in stock to take away.");
    app.htmx()
        .put(&url, &[("change", "0")])
        .await
        .assert_status(422);
    assert_eq!(stock(&app, coffee.id).await, 3);
    // A success reloads the page (closing the sheet) with a toast.
    app.htmx()
        .put(&url, &[("change", "7"), ("reason", "Delivery")])
        .await
        .assert_ok()
        .assert_header("hx-refresh", "true");
    assert_eq!(stock(&app, coffee.id).await, 10);
    app.get("/admin/products")
        .await
        .assert_see("“Coffee”: 10 in stock.")
        .assert_see("Delivery");
}

#[renox::test]
async fn mail_and_notifications_speak_the_customers_language() {
    let app = shop().await;
    let boss = admin(&app).await;
    // The admin reads English (a user without a `locale` gets the language
    // of whatever request caused the notification).
    renox::db::sql("UPDATE users SET locale = 'en' WHERE id = ?")
        .bind(boss.id)
        .execute(app.db())
        .await
        .unwrap();
    let coffee = product(&app, "Coffee Latte", 25_000, 5).await;
    let ben = customer(&app, "ben@example.com").await;
    app.acting_as(&ben);
    app.get("/language/es").await; // saved on the user
    app.post(
        "/cart",
        &[("product_id", &coffee.id.to_string()), ("quantity", "1")],
    )
    .await;
    app.post("/checkout", &[("delivery", "pickup")]).await;
    let order = Order::where_eq("user_id", ben.id)
        .first_or_404(app.db())
        .await
        .unwrap();
    assert!(order.pickup);
    app.run_jobs().await;
    app.assert_mail_sent("ben@example.com", &format!("Pedido #{} recibido", order.id));
    let mail = app
        .sent_mail()
        .into_iter()
        .find(|m| m.to.iter().any(|t| t.contains("ben")))
        .unwrap();
    let body = format!("{:?}", mail);
    assert!(
        body.contains("Recógelo en la tienda cuando esté listo"),
        "{body}"
    );
    assert!(!body.contains("Lo enviaremos a"));
    // The admin, still on English, gets the new order in English.
    app.acting_as(&boss);
    app.get("/notifications")
        .await
        .assert_see(&format!("New order #{}", order.id));
    app.put(
        &format!("/admin/orders/{}/status", order.id),
        &[("status", "paid")],
    )
    .await;
    app.put(
        &format!("/admin/orders/{}/status", order.id),
        &[("status", "shipped")],
    )
    .await;
    app.run_jobs().await;
    app.assert_mail_sent(
        "ben@example.com",
        &format!("El pedido #{} está listo para recoger", order.id),
    );
}

#[renox::test]
async fn a_stale_cancel_changes_nothing() {
    let app = shop().await;
    let boss = admin(&app).await;
    let coffee = product(&app, "Coffee Latte", 25_000, 5).await;
    let ben = customer(&app, "ben@example.com").await;
    let order = placed_order(&app, &ben, &coffee, 1).await;
    // Paid by someone else after the admin opened the page.
    renox::db::sql("UPDATE orders SET status = 'paid' WHERE id = ?")
        .bind(order.id)
        .execute(app.db())
        .await
        .unwrap();
    app.acting_as(&boss);
    // The handler reads the order fresh, so this is the normal "can't go
    // from paid to cancelled".
    app.put(
        &format!("/admin/orders/{}/status", order.id),
        &[("status", "cancelled")],
    )
    .await
    .assert_status(409);
    assert_eq!(status(&app, order.id).await, OrderStatus::Paid);
    assert_eq!(stock(&app, coffee.id).await, 4, "the stock stays sold");
}

#[renox::test]
async fn make_admin_finds_the_user_in_any_case() {
    let app = shop().await;
    customer(&app, "ben@example.com").await;
    app.kernel()
        .call("shop:make-admin", ["Ben@Example.COM"])
        .await
        .unwrap();
}

#[renox::test]
async fn the_seeder_fills_the_app_and_can_run_again() {
    let app = TestApp::new(shop::app()).await;
    app.kernel().seed().await.unwrap();
    let seeded = Product::query().count(app.db()).await.unwrap();
    assert!(seeded > 0);
    // A second `db:seed` leaves a seeded database as it is.
    app.kernel().seed().await.unwrap();
    assert_eq!(Product::query().count(app.db()).await.unwrap(), seeded);
}
