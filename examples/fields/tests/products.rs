//! Runs on SQLite, or on PostgreSQL with TEST_DATABASE_URL set.

use fields::{Product, Size};
use renox::chrono::{NaiveDate, NaiveTime};
use renox::prelude::*;
use renox::testing::TestApp;

const FULL: &[(&str, &str)] = &[
    ("name", "Kopi Gayo"),
    ("description", "Arabica"),
    ("stock", "12"),
    ("weight_kg", "0.25"),
    ("price", "85000"),
    ("available", "on"),
    ("size", "large"),
    ("colors", "black"),
    ("colors", "red"),
    ("opens_at", "07:30"),
    ("launch_at", "2026-10-01T10:30"),
    ("released_on", "2026-09-27"),
];

#[renox::test]
async fn a_product_round_trips_from_the_form_to_the_database_and_back() {
    let app = TestApp::new(fields::app()).await;
    let res = app.post("/products", FULL).await;
    res.assert_status(303);
    let edit = res.header("location").unwrap().to_owned();

    let product = Product::query().first(app.db()).await.unwrap().unwrap();
    assert_eq!(product.name, "Kopi Gayo");
    assert_eq!(product.description.as_deref(), Some("Arabica"));
    assert_eq!(
        (product.stock, product.weight_kg, product.price),
        (12, 0.25, 85_000)
    );
    assert!(product.available);
    assert_eq!(product.size, Size::Large);
    assert_eq!(*product.colors, ["black", "red"]);
    assert_eq!(product.opens_at, NaiveTime::from_hms_opt(7, 30, 0));
    assert_eq!(
        product.launch_at,
        NaiveDate::from_ymd_opt(2026, 10, 1)
            .unwrap()
            .and_hms_opt(10, 30, 0)
    );
    assert_eq!(product.released_on, NaiveDate::from_ymd_opt(2026, 9, 27));
    assert_eq!(edit, format!("/products/{}/edit", product.id));

    // The edit form shows every value in the format its input expects.
    app.get(&edit)
        .await
        .assert_ok()
        .assert_see(r#"name="name" value="Kopi Gayo""#)
        .assert_see(">Arabica</textarea>")
        .assert_see(r#"value="0.25""#)
        .assert_see(r#"name="available" checked"#)
        .assert_see(r#"<option value="large" selected>"#)
        .assert_see(r#"value="black" checked"#)
        .assert_see(r#"value="red" checked"#)
        .assert_see(r#"value="07:30:00""#)
        .assert_see(r#"value="2026-10-01T10:30:00""#)
        .assert_see(r#"value="2026-09-27""#);
}

#[renox::test]
async fn unchecking_and_emptying_fields_saves_them_empty() {
    let app = TestApp::new(fields::app()).await;
    let edit = app
        .post("/products", FULL)
        .await
        .header("location")
        .unwrap()
        .to_owned();
    let update = edit.trim_end_matches("/edit").to_owned();

    // Unchecked boxes and empty inputs aren't sent (or are sent empty).
    app.put(
        &update,
        &[
            ("name", "Kopi Gayo"),
            ("stock", "0"),
            ("weight_kg", "0"),
            ("price", "0"),
            ("size", "small"),
            ("description", ""),
            ("opens_at", ""),
        ],
    )
    .await
    .assert_status(303);
    let product = Product::query().first(app.db()).await.unwrap().unwrap();
    assert!(!product.available);
    assert!(product.colors.is_empty());
    assert_eq!(product.description, None);
    assert_eq!(product.opens_at, None);
    assert_eq!(product.size, Size::Small);
}

#[renox::test]
async fn invalid_values_are_reported_together() {
    let app = TestApp::new(fields::app()).await;
    app.htmx()
        .post(
            "/products",
            &[
                ("name", ""),
                ("stock", "-1"),
                ("weight_kg", "heavy"),
                ("price", "0"),
                ("size", "huge"),
                ("colors", "purple"),
            ],
        )
        .await
        .assert_invalid("name")
        .assert_invalid("stock")
        .assert_invalid("weight_kg")
        .assert_invalid("size")
        .assert_invalid("colors.0");
}

async fn post_colors(app: &TestApp, colors: &[&str]) -> renox::testing::TestResponse {
    let mut form = vec![("name", "Kaos"), ("size", "small")];
    form.extend(colors.iter().map(|color| ("colors", *color)));
    app.htmx().post("/products", &form).await
}

#[renox::test]
async fn each_color_is_checked_and_repeats_are_refused() {
    let app = TestApp::new(fields::app()).await;
    // Only the unknown one is reported, under its index.
    let res = post_colors(&app, &["black", "purple"]).await;
    res.assert_invalid("colors.1");
    assert!(res.json_path("errors.colors.0").is_null());
    // The repeat gets the error (case and spaces don't make it new).
    post_colors(&app, &["red", "Red "])
        .await
        .assert_invalid("colors.1");

    // Without htmx: back to the form, with the first item's error in the
    // `colors` slot.
    app.request()
        .header("referer", "/products/new")
        .post("/products", &[("name", "Kaos"), ("colors", "purple")])
        .await
        .assert_redirect("/products/new");
    app.get("/products/new")
        .await
        .assert_see(r#"data-error-for="colors">"#)
        .assert_see("The selected colors #1 is invalid.");
}
