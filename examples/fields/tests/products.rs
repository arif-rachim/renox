//! Runs on SQLite, or on PostgreSQL with TEST_DATABASE_URL set.

use fields::{Product, Size};
use renox::chrono::{NaiveDate, NaiveTime};
use renox::prelude::*;
use renox::testing::TestApp;

const FULL: &[(&str, &str)] = &[
    ("name", "Highland Coffee"),
    ("description", "Arabica"),
    ("stock", "12"),
    ("weight_kg", "0.25"),
    ("price", "16.99"),
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
    assert_eq!(product.name, "Highland Coffee");
    assert_eq!(product.description.as_deref(), Some("Arabica"));
    assert_eq!(
        (product.stock, product.weight_kg, product.price),
        (12, 0.25, 1_699)
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
        .assert_see(r#"name="name" type="text" value="Highland Coffee""#)
        .assert_see(">Arabica</textarea>")
        .assert_see(r#"value="0.25""#)
        // The price in dollars, stored in cents.
        .assert_see(r#"name="price" type="number" value="16.99""#)
        .assert_see(r#"name="available" value="on" checked"#)
        .assert_see(r#"name="size" value="large" checked"#)
        .assert_see(r#"value="black" checked"#)
        .assert_see(r#"value="red" checked"#)
        .assert_see(r#"value="07:30:00""#)
        .assert_see(r#"value="2026-10-01T10:30:00""#)
        .assert_see(r#"value="2026-09-27""#)
        // The key, read-only with a copy button; the date in the kit's picker.
        .assert_see(&format!(r#"value="{}" readonly"#, product.id))
        .assert_see(r#"data-rx-copy="rx-key""#)
        .assert_see(r#"popovertarget="rx-released_on-calendar""#);
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
            ("name", "Highland Coffee"),
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
        .assert_see(
            r#"data-error-for="colors" aria-live="polite">The selected colors #1 is invalid."#,
        )
        // Only what was sent is ticked: no listed color.
        .assert_dont_see(r#"name="colors" value="black" checked"#);
}

#[renox::test]
async fn tags_and_specifications_ride_a_nested_form() {
    let app = TestApp::new(fields::app()).await;
    // `specs[0][key]` makes the whole form nested: every other field still
    // parses from its text (numbers, the enum, the checkbox, dates, times).
    let mut form = FULL.to_vec();
    form.extend([
        ("tags", "organic"),
        ("tags", "decaf"),
        ("tags", ""),
        ("specs[0][key]", "Origin"),
        ("specs[0][value]", "Aceh"),
        ("specs[1][key]", ""),
        ("specs[1][value]", ""),
        ("specs[2][key]", "Roast"),
        ("specs[2][value]", "Medium"),
    ]);
    let res = app.post("/products", &form).await;
    res.assert_status(303);
    let edit = res.header("location").unwrap().to_owned();
    let product = Product::query().first(app.db()).await.unwrap().unwrap();
    assert_eq!(*product.tags, ["organic", "decaf"]);
    assert_eq!(
        product.specs.iter().collect::<Vec<_>>(),
        [("Origin", "Aceh"), ("Roast", "Medium")]
    );
    assert_eq!(product.size, Size::Large);
    assert!(product.available);
    assert_eq!(product.opens_at, NaiveTime::from_hms_opt(7, 30, 0));
    assert_eq!(*product.colors, ["black", "red"]);

    // The edit form shows them back: chips, and a row per pair.
    app.get(&edit)
        .await
        .assert_see(r#"<input type="hidden" name="tags" value="decaf">"#)
        .assert_see(r#"name="specs[1][key]" type="text" value="Roast""#)
        .assert_see(r#"name="specs[1][value]" type="text" value="Medium""#);

    // Too many tags: the error is the list's, the input kept.
    let mut form = FULL.to_vec();
    for tag in ["a", "b", "c", "d", "e", "f"] {
        form.push(("tags", tag));
    }
    form.push(("specs[0][key]", "Origin"));
    app.htmx()
        .post("/products", &form)
        .await
        .assert_invalid("tags");
}

#[renox::test]
async fn the_show_page_formats_every_field() {
    let app = TestApp::new(fields::app()).await;
    let mut form: Vec<(&str, &str)> = FULL
        .iter()
        .map(|&(k, v)| {
            (
                k,
                if k == "description" {
                    "**Single** origin <b>beans</b>"
                } else {
                    v
                },
            )
        })
        .collect();
    form.extend([
        ("tags", "organic"),
        ("tags", "decaf"),
        ("specs[0][key]", "Origin"),
        ("specs[0][value]", "Aceh"),
    ]);
    app.post("/products", &form).await.assert_status(303);
    let product = Product::query().first(app.db()).await.unwrap().unwrap();
    app.get("/").await.assert_see(&format!(
        r#"href="/products/{}"><strong>Highland Coffee</strong>"#,
        product.id
    ));
    app.get(&format!("/products/{}", product.id))
        .await
        .assert_ok()
        .assert_view("products/show.html")
        .assert_see(&format!(r#"data-rx-copy-text="{}""#, product.id))
        // Markdown, with the HTML typed in shown as text.
        .assert_see("<strong>Single</strong> origin &lt;b&gt;beans&lt;/b&gt;")
        .assert_see("$16.99")
        .assert_see("0.25<span class=\"rx-entry__affix\">kg</span>")
        .assert_see("</svg>Yes</span>")
        .assert_see(r#"<span class="rx-badge rx-badge--info">Large</span>"#)
        .assert_see(r#"style="background: black""#)
        .assert_see(r#"<span class="rx-badge">organic</span>"#)
        .assert_see(r#"<th scope="row">Origin</th><td>Aceh</td>"#)
        .assert_see(">07:30:00<")
        .assert_see(">01 Oct 2026, 10:30<")
        .assert_see(">27 Sep 2026<")
        .assert_see(">just now</time>");
}

#[renox::test]
async fn the_seeder_fills_the_app_and_can_run_again() {
    let app = TestApp::new(fields::app()).await;
    app.kernel().seed().await.unwrap();
    let seeded = Product::query().count(app.db()).await.unwrap();
    assert!(seeded > 0);
    // A second `db:seed` leaves a seeded database as it is.
    app.kernel().seed().await.unwrap();
    assert_eq!(Product::query().count(app.db()).await.unwrap(), seeded);
}

#[renox::test]
async fn the_index_lists_products_in_a_table_or_says_it_is_empty() {
    let app = TestApp::new(fields::app()).await;
    app.get("/")
        .await
        .assert_ok()
        .assert_see("No products yet")
        .assert_see(r#"href="/products/new""#);

    app.kernel().seed().await.unwrap();
    let coffee = Product::query()
        .where_eq("name", "Highland Coffee")
        .first(app.db())
        .await
        .unwrap()
        .unwrap();
    app.get("/")
        .await
        .assert_dont_see("No products yet")
        .assert_see(r#"<table class="rx-table">"#)
        .assert_see("$16.99")
        .assert_see(r#"<span class="rx-badge rx-badge--info">Medium</span>"#)
        .assert_see(&format!(r#"href="/products/{}/edit""#, coffee.id))
        .assert_see(&format!(r#"action="/products/{}""#, coffee.id));
}

#[renox::test]
async fn saving_shows_a_toast_on_the_next_page() {
    let app = TestApp::new(fields::app()).await;
    let res = app.post("/products", FULL).await;
    let edit = res.header("location").unwrap().to_owned();
    app.get(&edit).await.assert_see("Product created.");

    let update = edit.trim_end_matches("/edit").to_owned();
    app.put(&update, FULL).await.assert_redirect(&edit);
    app.get(&edit).await.assert_see("Saved.");
    // Once only.
    app.get(&edit).await.assert_dont_see("Saved.");
}

#[renox::test]
async fn a_product_can_be_deleted() {
    let app = TestApp::new(fields::app()).await;
    app.kernel().seed().await.unwrap();
    let coffee = Product::query()
        .where_eq("name", "Highland Coffee")
        .first(app.db())
        .await
        .unwrap()
        .unwrap();

    app.delete(&format!("/products/{}", coffee.id))
        .await
        .assert_redirect("/");
    assert!(Product::find(app.db(), coffee.id).await.unwrap().is_none());
    assert_eq!(Product::query().count(app.db()).await.unwrap(), 1);
    app.get("/")
        .await
        .assert_see("“Highland Coffee” deleted.")
        .assert_dont_see("<strong>Highland Coffee</strong>");
    // Gone: a second delete is a 404.
    app.delete(&format!("/products/{}", coffee.id))
        .await
        .assert_status(404);
}

#[renox::test]
async fn the_editors_save_rich_text_cleaned_and_code_as_typed() {
    let app = TestApp::new(fields::app()).await;
    let mut form = FULL.to_vec();
    form.extend([
        (
            "details",
            r#"<div>Roasted <strong>weekly</strong><img src=x onerror="alert(1)"></div>"#,
        ),
        ("settings", "{\"grind\": \"<fine>\"}"),
    ]);
    app.post("/products", &form).await.assert_status(303);
    let product = Product::query().first(app.db()).await.unwrap().unwrap();
    assert_eq!(
        product.details.as_deref(),
        Some("<div>Roasted <strong>weekly</strong></div>")
    );
    assert_eq!(product.settings.as_deref(), Some("{\"grind\": \"<fine>\"}"));

    // The edit form gives each editor its value; the page loads the editors once.
    let edit = app
        .get(&format!("/products/{}/edit", product.id))
        .await
        .assert_ok()
        .assert_see(r#"name="details" value="&lt;div&gt;Roasted &lt;strong&gt;weekly&lt;/strong&gt;&lt;/div&gt;""#)
        .assert_see(r#"data-rx-code-editor data-language="json""#)
        .assert_see(">{&quot;grind&quot;: &quot;&lt;fine&gt;&quot;}</textarea>")
        .assert_see(r#"data-rx-markdown"#)
        .text();
    assert_eq!(edit.matches("data-renox-editors").count(), 1);

    // The page shows the rich text and highlights the code, with buttons
    // beside some entries.
    app.get(&format!("/products/{}", product.id))
        .await
        .assert_see("<div>Roasted <strong>weekly</strong></div>")
        .assert_see(r#"<code class="language-json" data-rx-highlight translate="no">{&quot;grind&quot;: &quot;&lt;fine&gt;&quot;}</code>"#)
        .assert_see(&format!(
            r#"href="/products/{}/edit" aria-label="Edit this product""#,
            product.id
        ));

    // Settings that aren't JSON, and an emptied rich text editor.
    let mut bad = FULL.to_vec();
    bad.extend([("details", "<div><br></div>"), ("settings", "{grind")]);
    app.htmx()
        .post("/products", &bad)
        .await
        .assert_invalid("settings");
    let mut empty = FULL.to_vec();
    empty.extend([("details", "<div><br></div>"), ("settings", "")]);
    let res = app.post("/products", &empty).await;
    let id = res.header("location").unwrap().split('/').nth(2).unwrap();
    let saved = Product::find(app.db(), id.parse().unwrap())
        .await
        .unwrap()
        .unwrap();
    assert_eq!((saved.details, saved.settings), (None, None));
}
