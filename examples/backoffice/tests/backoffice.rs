//! The back office: who may do what, invoices and the stock ledger, the CSV
//! import, exports in the background, online payments, staff and settings.

use backoffice::app::customers::Customer;
use backoffice::app::invoices::{Invoice, InvoiceLine};
use backoffice::app::products::Product;
use backoffice::app::products::stock::StockMovement;
use renox::prelude::*;
use renox::testing::TestApp;
use renox::webhook;

async fn app() -> TestApp {
    let app = TestApp::with_config(backoffice::app(), |c| {
        // Keys as they'd be in .env.
        c.vars
            .insert("XENDIT_SECRET_KEY".into(), "xnd_development_test".into());
        c.vars
            .insert("XENDIT_CALLBACK_TOKEN".into(), "xnd-token-test".into());
        c.vars
            .insert("MIDTRANS_SERVER_KEY".into(), "SB-Mid-server-test".into());
    })
    .await;
    backoffice::define_roles(app.db()).await.unwrap();
    app
}

/// A verified member of staff with `role`, logged in.
async fn staff(app: &TestApp, role: &str) -> User {
    let email = format!("{role}@example.com");
    let user = User::register(app.db(), role, &email, "password123")
        .await
        .unwrap();
    renox::db::sql("UPDATE users SET email_verified_at = ? WHERE id = ?")
        .bind(renox::db::now())
        .bind(user.id)
        .execute(app.db())
        .await
        .unwrap();
    user.assign_role(app.db(), role).await.unwrap();
    app.acting_as(&user);
    user
}

async fn customer(app: &TestApp) -> Customer {
    Customer::create(
        app.db(),
        Customer {
            name: "Sally's Diner".into(),
            email: Some("sally@example.com".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap()
}

/// A product with `stock` on the shelf (received through the ledger).
async fn product(app: &TestApp, sku: &str, price: i64, stock: i64) -> Product {
    let product = Product::create(
        app.db(),
        Product {
            sku: sku.into(),
            name: format!("Product {sku}"),
            price,
            active: true,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let mut tx = app.db().begin().await.unwrap();
    backoffice::app::products::stock::change(
        &mut tx,
        backoffice::app::products::stock::Change {
            product_id: product.id,
            quantity: stock,
            reason: "received",
            note: "",
            invoice_id: None,
            user_name: "test",
        },
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    Product::find_or_404(app.db(), product.id).await.unwrap()
}

async fn stock_of(app: &TestApp, id: i64) -> i64 {
    Product::find_or_404(app.db(), id).await.unwrap().stock
}

/// Writes a draft through the form: one line per (product, quantity).
async fn draft(app: &TestApp, customer: &Customer, lines: &[(&Product, i64)]) -> Invoice {
    let mut form = vec![("customer_id".to_owned(), customer.id.to_string())];
    for (i, (product, quantity)) in lines.iter().enumerate() {
        form.push((format!("lines[{i}][product_id]"), product.id.to_string()));
        form.push((format!("lines[{i}][quantity]"), quantity.to_string()));
        form.push((format!("lines[{i}][unit_price]"), String::new()));
    }
    let pairs: Vec<(&str, &str)> = form.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    let res = app.post("/invoices", &pairs).await;
    assert_eq!(res.status, StatusCode::SEE_OTHER, "{}", res.text());
    Invoice::query()
        .latest()
        .first(app.db())
        .await
        .unwrap()
        .unwrap()
}

#[renox::test]
async fn guests_log_in_and_new_staff_verify_first() {
    let app = app().await;
    app.get("/invoices").await.assert_redirect("/login");
    // The sign-in page wears the company's name and colour.
    app.get("/login")
        .await
        .assert_see("Corner Store")
        .assert_see("--rx-accent: #0f766e");
    // Nobody signs up: an admin adds staff.
    app.get("/register").await.assert_not_found();

    let user = User::register(app.db(), "Nina", "nina@example.com", "password123")
        .await
        .unwrap();
    app.acting_as(&user);
    app.get("/invoices").await.assert_redirect("/verify-email");
}

#[renox::test]
async fn each_role_changes_only_its_own_things() {
    let app = app().await;
    let coffee = product(&app, "COFFEE", 50_000, 10).await;
    let buyer = customer(&app).await;

    staff(&app, "warehouse").await;
    // Everyone on the staff sees the lists…
    app.get("/invoices").await.assert_ok();
    // …but a warehouse keeper writes no invoices and adds no customers.
    app.get("/invoices/new").await.assert_forbidden();
    app.post("/customers", &[("name", "X")])
        .await
        .assert_forbidden();
    app.get("/staff").await.assert_forbidden();
    app.htmx()
        .post(
            &format!("/products/{}/stock", coffee.id),
            &[("reason", "received"), ("quantity", "5")],
        )
        .await
        .assert_ok();
    assert_eq!(stock_of(&app, coffee.id).await, 15);
    // The menu shows what they may open.
    app.get("/")
        .await
        .assert_see("Products")
        .assert_dont_see(">Staff<")
        .assert_dont_see(">Settings<");

    app.logout();
    staff(&app, "cashier").await;
    app.post(
        &format!("/products/{}/stock", coffee.id),
        &[("reason", "received"), ("quantity", "5")],
    )
    .await
    .assert_forbidden();
    app.get("/settings").await.assert_forbidden();
    draft(&app, &buyer, &[(&coffee, 1)]).await;
}

#[renox::test]
async fn issuing_takes_the_stock_and_voiding_brings_it_back() {
    let app = app().await;
    let coffee = product(&app, "COFFEE", 50_000, 10).await;
    let sugar = product(&app, "SUGAR", 20_000, 3).await;
    let buyer = customer(&app).await;
    staff(&app, "cashier").await;

    let invoice = draft(&app, &buyer, &[(&coffee, 2), (&sugar, 1)]).await;
    // The price of the product, 11% tax, the number from the settings.
    assert_eq!(invoice.subtotal, 120_000);
    assert_eq!(invoice.tax, 13_200);
    assert_eq!(invoice.total, 133_200);
    assert_eq!(invoice.number, format!("INV-{:05}", invoice.id));
    assert_eq!(invoice.status, "draft");
    assert_eq!(stock_of(&app, coffee.id).await, 10, "a draft takes nothing");

    let issue = format!("/invoices/{}/issue", invoice.id);
    app.post(&issue, &[])
        .await
        .assert_redirect(&format!("/invoices/{}", invoice.id));
    assert_eq!(stock_of(&app, coffee.id).await, 8);
    assert_eq!(stock_of(&app, sugar.id).await, 2);
    app.assert_database_has(
        "stock_movements",
        &[
            ("product_id", &coffee.id),
            ("quantity", &-2_i64),
            ("reason", &"sold"),
            ("invoice_id", &invoice.id),
        ],
    )
    .await;
    // A second click finds it issued already.
    app.post(&issue, &[]).await.assert_status(409);
    assert_eq!(stock_of(&app, coffee.id).await, 8);

    app.post(&format!("/invoices/{}/void", invoice.id), &[])
        .await
        .assert_redirect(&format!("/invoices/{}", invoice.id));
    assert_eq!(stock_of(&app, coffee.id).await, 10);
    assert_eq!(stock_of(&app, sugar.id).await, 3);
    app.assert_database_has(
        "audit_logs",
        &[("action", &"invoice.voided"), ("subject_id", &invoice.id)],
    )
    .await;
    // The ledger adds up to the stock.
    let total: i64 = StockMovement::where_eq("product_id", coffee.id)
        .sum(app.db(), "quantity")
        .await
        .unwrap();
    assert_eq!(total, 10);
}

#[renox::test]
async fn an_invoice_short_of_stock_changes_nothing() {
    let app = app().await;
    let coffee = product(&app, "COFFEE", 50_000, 10).await;
    let sugar = product(&app, "SUGAR", 20_000, 1).await;
    let buyer = customer(&app).await;
    staff(&app, "cashier").await;

    let invoice = draft(&app, &buyer, &[(&coffee, 2), (&sugar, 5)]).await;
    app.post(&format!("/invoices/{}/issue", invoice.id), &[])
        .await
        .assert_status(409)
        .assert_see("Only 1 of Product SUGAR in stock; 5 needed.");
    // The first line's stock came back with the rollback, and it's a draft.
    assert_eq!(stock_of(&app, coffee.id).await, 10);
    assert_eq!(
        Invoice::find_or_404(app.db(), invoice.id)
            .await
            .unwrap()
            .status,
        "draft"
    );
    app.assert_database_count("stock_movements", 2).await;
}

#[renox::test]
async fn the_invoice_form_checks_each_line() {
    let app = app().await;
    let buyer = customer(&app).await;
    staff(&app, "cashier").await;
    app.htmx()
        .post(
            "/invoices",
            &[
                ("customer_id", &buyer.id.to_string()),
                ("lines[0][product_id]", "999"),
                ("lines[0][quantity]", "0"),
            ],
        )
        .await
        .assert_invalid("lines.0.product_id")
        .assert_invalid("lines.0.quantity");
    app.htmx()
        .post("/invoices", &[("customer_id", &buyer.id.to_string())])
        .await
        .assert_invalid("lines");
    // A row left empty is named like the form names it (resources/lang).
    let res = app
        .htmx()
        .post(
            "/invoices",
            &[
                ("customer_id", &buyer.id.to_string()),
                ("lines[0][product_id]", ""),
                ("lines[0][quantity]", "1"),
            ],
        )
        .await;
    let errors: renox::serde_json::Value = res.json();
    assert_eq!(
        errors["errors"]["lines.0.product_id"][0], "The product field is required.",
        "{errors}"
    );
    app.assert_database_count("invoices", 0).await;
}

#[renox::test]
async fn cash_pays_an_issued_invoice_and_tells_the_cashiers() {
    let app = app().await;
    let coffee = product(&app, "COFFEE", 50_000, 10).await;
    let buyer = customer(&app).await;
    let cashier = staff(&app, "cashier").await;
    let invoice = draft(&app, &buyer, &[(&coffee, 1)]).await;
    let paid = format!("/invoices/{}/paid", invoice.id);
    // A draft isn't paid.
    app.post(&paid, &[]).await.assert_status(409);
    app.post(&format!("/invoices/{}/issue", invoice.id), &[])
        .await;
    app.post(&paid, &[])
        .await
        .assert_redirect(&format!("/invoices/{}", invoice.id));
    let invoice = Invoice::find_or_404(app.db(), invoice.id).await.unwrap();
    assert_eq!(invoice.status, "paid");
    assert_eq!(invoice.paid_via.as_deref(), Some("cash"));
    app.assert_database_has(
        "notifications",
        &[("user_id", &cashier.id), ("kind", &"payment-received")],
    )
    .await;
}

#[renox::test]
async fn stock_is_received_counted_and_never_below_zero() {
    let app = app().await;
    let coffee = product(&app, "COFFEE", 50_000, 10).await;
    staff(&app, "warehouse").await;
    let url = format!("/products/{}/stock", coffee.id);
    app.htmx()
        .post(&url, &[("reason", "damaged"), ("quantity", "11")])
        .await
        .assert_invalid("quantity");
    assert_eq!(stock_of(&app, coffee.id).await, 10);
    // Counted: the shelf has 7, so the ledger says -3.
    app.htmx()
        .post(
            &url,
            &[
                ("reason", "counted"),
                ("quantity", "7"),
                ("note", "Stocktake"),
            ],
        )
        .await
        .assert_ok();
    assert_eq!(stock_of(&app, coffee.id).await, 7);
    app.assert_database_has(
        "stock_movements",
        &[
            ("quantity", &-3_i64),
            ("reason", &"counted"),
            ("note", &"Stocktake"),
        ],
    )
    .await;
    app.get(&format!("/products/{}", coffee.id))
        .await
        .assert_see("Stocktake")
        .assert_see("Stock ledger");
}

#[renox::test]
async fn products_import_from_csv_line_by_line() {
    let app = app().await;
    product(&app, "COFFEE", 50_000, 10).await;
    staff(&app, "warehouse").await;
    let csv = "sku,name,price,stock\n\
               COFFEE,Arabica coffee,55000,5\n\
               tea-01,\"Tea, jasmine\",18000,20\n\
               BAD SKU,Nope,1,1\n\
               SUGAR,Sugar,pricey,1\n";
    let res = app
        .htmx()
        .post_multipart(
            "/products/import",
            &[],
            &[("file", "products.csv", csv.as_bytes())],
        )
        .await;
    res.assert_ok();
    // The toast waits for the refreshed page (HxRefresh).
    app.get("/products")
        .await
        .assert_see("1 added, 1 updated, 2 skipped.")
        .assert_see("line 4: `BAD SKU` isn&#x27;t a SKU")
        .assert_see("line 5: `pricey` isn&#x27;t a price");

    let coffee = Product::where_eq("sku", "COFFEE")
        .first(app.db())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        (coffee.name.as_str(), coffee.price, coffee.stock),
        ("Arabica coffee", 55_000, 15)
    );
    let tea = Product::where_eq("sku", "TEA-01")
        .first(app.db())
        .await
        .unwrap()
        .unwrap();
    assert_eq!((tea.name.as_str(), tea.stock), ("Tea, jasmine", 20));
    app.assert_database_count("products", 2).await;

    // Only CSV files.
    app.htmx()
        .post_multipart("/products/import", &[], &[("file", "x.exe", b"MZ")])
        .await
        .assert_invalid("file");
}

#[renox::test]
async fn exports_run_in_the_background_with_the_grids_filters() {
    let app = app().await;
    let coffee = product(&app, "COFFEE", 50_000, 100).await;
    let buyer = customer(&app).await;
    let cashier = staff(&app, "cashier").await;
    let first = draft(&app, &buyer, &[(&coffee, 1)]).await;
    let second = draft(&app, &buyer, &[(&coffee, 2)]).await;
    app.post(&format!("/invoices/{}/issue", second.id), &[])
        .await;

    // "All matching" with the grid filtered to issued invoices.
    let started = app
        .htmx()
        .post(
            "/invoices/export?in.status=issued",
            &[("ids", ""), ("all", "true")],
        )
        .await;
    assert!(started.status.is_success(), "{}", started.status);
    app.run_jobs().await;
    let link: String = renox::db::sql(
        "SELECT data FROM notifications WHERE user_id = ? AND kind = 'export-ready'",
    )
    .bind(cashier.id)
    .scalar::<String>(app.db())
    .await
    .map(|data| {
        let data: renox::serde_json::Value = renox::serde_json::from_str(&data).unwrap();
        data["url"].as_str().unwrap().to_owned()
    })
    .unwrap();
    let csv = app.get(&link).await;
    csv.assert_ok();
    let text = csv.text();
    assert!(text.contains(&second.number), "{text}");
    assert!(!text.contains(&first.number), "only issued ones: {text}");
    // The link is signed: changed, it fails.
    app.get(&link.replace("invoices-", "invoices-x"))
        .await
        .assert_status(403);

    // Warehouse staff can't export.
    app.logout();
    staff(&app, "warehouse").await;
    app.htmx()
        .post("/invoices/export", &[("ids", "1"), ("all", "false")])
        .await
        .assert_forbidden();
}

#[renox::test]
async fn a_xendit_payment_page_and_its_webhook_pay_the_invoice() {
    let app = app().await;
    let coffee = product(&app, "COFFEE", 50_000, 10).await;
    let buyer = customer(&app).await;
    let cashier = staff(&app, "cashier").await;
    let invoice = draft(&app, &buyer, &[(&coffee, 1)]).await;
    app.post(&format!("/invoices/{}/issue", invoice.id), &[])
        .await;
    let link = format!("/invoices/{}/payment-link", invoice.id);

    // No gateway chosen yet: the button explains, the route refuses.
    app.get(&format!("/invoices/{}", invoice.id))
        .await
        .assert_see("Choose a payment gateway in Settings");
    app.htmx().post(&link, &[]).await.assert_status(422);

    let mut settings = backoffice::Settings::load(app.db()).await.unwrap();
    settings.payment_gateway = "xendit".into();
    settings.save(app.db()).await.unwrap();
    app.fake_http().on(
        "https://api.xendit.co/v2/invoices",
        renox::http::FakeResponse::json(
            200,
            json!({ "id": "inv_1", "invoice_url": "https://checkout.xendit.co/web/inv_1" }),
        ),
    );
    app.htmx().post(&link, &[]).await.assert_ok();
    app.fake_http().assert_sent(|req| {
        req.json()["external_id"] == invoice.number.as_str()
            && req.json()["amount"] == invoice.total
            && req.header("idempotency-key") == Some(invoice.number.as_str())
    });
    let stored = Invoice::find_or_404(app.db(), invoice.id).await.unwrap();
    assert_eq!(
        stored.payment_url.as_deref(),
        Some("https://checkout.xendit.co/web/inv_1")
    );

    // Xendit calls back (twice, as it retries): paid once, cashiers told once.
    let callback = format!(
        r#"{{"id":"inv_1","external_id":"{}","status":"PAID"}}"#,
        invoice.number
    );
    app.request()
        .without_csrf()
        .header("x-callback-token", "wrong")
        .post_body("/webhooks/xendit", "application/json", callback.as_str())
        .await
        .assert_status(401);
    for _ in 0..2 {
        app.request()
            .without_csrf()
            .header("x-callback-token", "xnd-token-test")
            .post_body("/webhooks/xendit", "application/json", callback.as_str())
            .await
            .assert_ok();
    }
    app.run_jobs().await;
    let paid = Invoice::find_or_404(app.db(), invoice.id).await.unwrap();
    assert_eq!(paid.status, "paid");
    assert_eq!(paid.paid_via.as_deref(), Some("xendit"));
    app.assert_database_has(
        "notifications",
        &[("user_id", &cashier.id), ("kind", &"payment-received")],
    )
    .await;
    app.assert_database_count("notifications", 1).await;
}

#[renox::test]
async fn a_midtrans_settlement_pays_the_invoice() {
    let app = app().await;
    let coffee = product(&app, "COFFEE", 50_000, 10).await;
    let buyer = customer(&app).await;
    staff(&app, "cashier").await;
    let invoice = draft(&app, &buyer, &[(&coffee, 1)]).await;
    app.post(&format!("/invoices/{}/issue", invoice.id), &[])
        .await;
    let body = |status: &str, key: &str| {
        let gross = format!("{}.00", invoice.total);
        let signature = webhook::sha512_hex(format!("{}200{gross}{key}", invoice.number));
        format!(
            r#"{{"order_id":"{}","status_code":"200","gross_amount":"{gross}","signature_key":"{signature}","transaction_id":"tx-1","transaction_status":"{status}","fraud_status":"accept"}}"#,
            invoice.number
        )
    };
    app.post_body(
        "/webhooks/midtrans",
        "application/json",
        body("settlement", "guessed"),
    )
    .await
    .assert_status(401);
    app.post_body(
        "/webhooks/midtrans",
        "application/json",
        body("settlement", "SB-Mid-server-test"),
    )
    .await
    .assert_ok();
    app.run_jobs().await;
    let paid = Invoice::find_or_404(app.db(), invoice.id).await.unwrap();
    assert_eq!(
        (paid.status.as_str(), paid.paid_via.as_deref()),
        ("paid", Some("midtrans"))
    );
}

#[renox::test]
async fn admins_add_staff_who_verify_their_email() {
    let app = app().await;
    let admin = staff(&app, "admin").await;
    app.htmx()
        .post(
            "/staff",
            &[
                ("name", "Sarah"),
                ("email", "sarah@example.com"),
                ("password", "first-password"),
                ("role", "cashier"),
            ],
        )
        .await
        .assert_ok();
    app.assert_mail_sent("sarah@example.com", "erif");
    let sarah = User::find_by_email(app.db(), "sarah@example.com")
        .await
        .unwrap()
        .unwrap();
    assert!(sarah.email_verified_at.is_none());
    assert_eq!(sarah.roles(app.db()).await.unwrap(), ["cashier"]);
    app.assert_database_has(
        "audit_logs",
        &[("action", &"staff.added"), ("subject_id", &sarah.id)],
    )
    .await;
    // Roles change from the staff page; an admin keeps their own.
    app.htmx()
        .put(
            &format!("/staff/{}/roles", sarah.id),
            &[("roles", "cashier"), ("roles", "warehouse")],
        )
        .await
        .assert_ok();
    let mut roles = sarah.roles(app.db()).await.unwrap();
    roles.sort();
    assert_eq!(roles, ["cashier", "warehouse"]);
    app.htmx()
        .put(
            &format!("/staff/{}/roles", admin.id),
            &[("roles", "cashier")],
        )
        .await
        .assert_invalid("roles");
    assert_eq!(admin.roles(app.db()).await.unwrap(), ["admin"]);
    app.get("/staff").await.assert_see("sarah@example.com");
    app.get("/activity").await.assert_see("staff.roles_changed");
}

#[renox::test]
async fn settings_shape_invoices_and_the_pages() {
    let app = app().await;
    staff(&app, "admin").await;
    let form = |color: &'static str| {
        vec![
            ("_method", "PUT"),
            ("company_name", "Northwind Coffee"),
            ("company_address", "1 Harbour Road"),
            ("company_email", "hello@northwind.test"),
            ("tax_percent", "10"),
            ("invoice_prefix", "NWC-"),
            ("payment_days", "7"),
            ("payment_gateway", "none"),
            ("brand_color", color),
        ]
    };
    app.htmx()
        .post("/settings", &form("red; }"))
        .await
        .assert_invalid("brand_color");
    app.post("/settings", &form("#7a4520"))
        .await
        .assert_redirect("/settings");
    app.get("/")
        .await
        .assert_see("Northwind Coffee")
        .assert_see("--rx-accent: #7a4520");
    app.assert_database_has("audit_logs", &[("action", &"settings.updated")])
        .await;

    let coffee = product(&app, "COFFEE", 10_000, 10).await;
    let buyer = customer(&app).await;
    let invoice = draft(&app, &buyer, &[(&coffee, 1)]).await;
    assert!(invoice.number.starts_with("NWC-"));
    assert_eq!((invoice.tax, invoice.total), (1_000, 11_000));
    assert_eq!(
        invoice.due_on - invoice.issued_on,
        renox::chrono::TimeDelta::days(7)
    );
}

#[renox::test]
async fn the_dashboard_shows_what_needs_doing() {
    let app = app().await;
    let low = product(&app, "SUGAR", 20_000, 2).await;
    Product::where_eq("id", low.id)
        .update(app.db(), &[("min_stock", &5_i64)])
        .await
        .unwrap();
    let buyer = customer(&app).await;
    let coffee = product(&app, "COFFEE", 50_000, 10).await;
    let cashier = staff(&app, "cashier").await;
    let invoice = draft(&app, &buyer, &[(&coffee, 1)]).await;
    app.post(&format!("/invoices/{}/issue", invoice.id), &[])
        .await;
    app.get("/")
        .await
        .assert_see("Product SUGAR")
        .assert_see("Nothing overdue");
    // Fifteen days on (a new login: sessions don't last that long), it is
    // past due.
    app.travel(std::time::Duration::from_secs(15 * 24 * 3600));
    app.acting_as(&cashier);
    app.get("/")
        .await
        .assert_see(&invoice.number)
        .assert_dont_see("Nothing overdue");
}

#[renox::test]
async fn the_seeder_fills_the_app_and_can_run_again() {
    let app = TestApp::new(backoffice::app()).await;
    for _ in 0..2 {
        app.kernel().seed().await.unwrap();
    }
    app.assert_database_count("users", 3).await;
    app.assert_database_count("invoices", 150).await;
    // The ledger agrees with every product's stock.
    for product in Product::query().get(app.db()).await.unwrap() {
        let total: i64 = StockMovement::where_eq("product_id", product.id)
            .sum(app.db(), "quantity")
            .await
            .unwrap();
        assert_eq!(total, product.stock, "{}", product.sku);
    }
    let lines = InvoiceLine::query().count(app.db()).await.unwrap();
    assert!(lines >= 150);
}
