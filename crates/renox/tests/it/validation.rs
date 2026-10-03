use axum::body::Body;
use axum::http::header::{CONTENT_TYPE, COOKIE, LOCATION, SET_COOKIE};
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use renox::Kernel;
use renox::prelude::*;
use serde::{Deserialize, Serialize};
use tower::ServiceExt;

#[derive(Model, Serialize, Default)]
#[model(table = "products", soft_deletes)]
struct Product {
    id: i64,
    name: String,
    price: i64,
    category: Option<String>,
    created_at: Option<DateTime>,
    updated_at: Option<DateTime>,
    deleted_at: Option<DateTime>,
}

#[derive(Deserialize, Serialize, Default)]
struct ProductForm {
    name: String,
    price: i64,
    category: Option<String>,
    email: Option<String>,
    website: Option<String>,
    password: Option<String>,
    password_confirmation: Option<String>,
    agree: Option<bool>,
    #[serde(skip)]
    ignore_id: Option<i64>,
}

impl Validate for ProductForm {
    fn rules(&self, v: &mut Validator) {
        let name = v
            .field("name", &self.name)
            .required()
            .between(3, 20)
            .unique("products", "name");
        if let Some(id) = self.ignore_id {
            name.ignore(id);
        }
        v.field("price", &self.price)
            .label("selling price")
            .min(1_000)
            .max(1_000_000);
        v.field("category", &self.category)
            .one_of(&["coffee", "tea"])
            .exists("products", "category");
        v.field("email", &self.email).email();
        v.field("website", &self.website).url();
        v.field("password", &self.password)
            .min(8)
            .confirmed(&self.password_confirmation);
        v.field("agree", &self.agree)
            .accepted()
            .message("Tick the box to agree first.");
    }
}

fn valid_form() -> ProductForm {
    ProductForm {
        name: "Milk Coffee".into(),
        price: 18_000,
        agree: Some(true),
        ..Default::default()
    }
}

async fn kernel(locale: &str) -> (Kernel, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("form.html"),
        r#"<input name="name" value="{{ old('name') }}"><input name="password" value="{{ old('password') }}"><p data-error-for="name">{{ error('name') }}</p><p>{{ error('price') }}</p>{{ csrf_token }}"#,
    )
    .unwrap();
    let config = {
        let mut c = Config::default();
        c.env = Environment::Testing;
        c.key = Some(renox::generate_key());
        c.views_path = dir.path().to_path_buf();
        c.locale = locale.into();
        c.lang_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/lang");
        c
    };
    let kernel = App::with_config(config)
        .migrations(renox::migrations!("tests/migrations"))
        .module(Shop)
        .boot()
        .await
        .unwrap();
    kernel.migrate().await.unwrap();
    Product::create(
        kernel.db(),
        Product {
            name: "Black Coffee".into(),
            price: 15_000,
            category: Some("coffee".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    (kernel, dir)
}

async fn errors_for(form: &ProductForm) -> Errors {
    let (kernel, _dir) = kernel("en").await;
    Validator::rules_of(form).finish(kernel.db()).await.unwrap()
}

#[tokio::test]
async fn valid_input_has_no_errors() {
    assert!(errors_for(&valid_form()).await.is_empty());
}

#[tokio::test]
async fn rules_report_the_first_failure_per_field() {
    let form = ProductForm {
        name: "Co".into(),
        price: 500,
        category: Some("milk".into()),
        email: Some("not-an-email".into()),
        website: Some("renox.dev".into()),
        password: Some("secret123".into()),
        password_confirmation: Some("different".into()),
        agree: Some(false),
        ..Default::default()
    };
    let errors = errors_for(&form).await;
    assert_eq!(
        errors.first("name"),
        Some("The name must be between 3 and 20 characters.")
    );
    assert_eq!(
        errors.first("price"),
        Some("The selling price must be at least 1000.")
    );
    assert_eq!(
        errors.first("category"),
        Some("The selected category is invalid.")
    );
    assert_eq!(
        errors.first("email"),
        Some("The email must be a valid email address.")
    );
    assert_eq!(
        errors.first("website"),
        Some("The website must be a valid URL.")
    );
    assert_eq!(
        errors.first("password"),
        Some("The password confirmation does not match.")
    );
    assert_eq!(errors.first("agree"), Some("Tick the box to agree first."));
    assert!(errors.iter().all(|(_, messages)| messages.len() == 1));
}

#[tokio::test]
async fn messages_come_in_the_apps_language() {
    // `tests/lang/es.json` translates the messages and the field names.
    let (mut client, _dir) = Client::new("es").await;
    let reply = client
        .post("/products", "name=+&price=2000000&agree=true", true)
        .await;
    let body: serde_json::Value = serde_json::from_str(&reply.body).unwrap();
    assert_eq!(body["errors"]["name"][0], "El campo nombre es obligatorio.");
    // An explicit `label` wins over the lang file's attribute name.
    assert_eq!(
        body["errors"]["price"][0],
        "El campo selling price no debe ser mayor que 1000000."
    );
}

#[tokio::test]
async fn unique_and_exists_query_the_database() {
    let taken = ProductForm {
        name: "Black Coffee".into(),
        ..valid_form()
    };
    assert_eq!(
        errors_for(&taken).await.first("name"),
        Some("The name has already been taken.")
    );

    let editing_itself = ProductForm {
        name: "Black Coffee".into(),
        ignore_id: Some(1),
        ..valid_form()
    };
    assert!(errors_for(&editing_itself).await.is_empty());

    let no_tea_yet = ProductForm {
        category: Some("tea".into()),
        ..valid_form()
    };
    assert_eq!(
        errors_for(&no_tea_yet).await.first("category"),
        Some("The selected category is invalid.")
    );
    let has_coffee = ProductForm {
        category: Some("coffee".into()),
        ..valid_form()
    };
    assert!(errors_for(&has_coffee).await.is_empty());
}

struct Shop;

impl Module for Shop {
    fn name(&self) -> &'static str {
        "shop"
    }

    fn routes(&self) -> Routes {
        Routes::new()
            .get("/products/create", || async { view("form.html", ()) })
            .post("/products", store)
            .get("/search", search)
            .post("/stock", stock)
    }
}

async fn store(State(db): State<Db>, Valid(form): Valid<ProductForm>) -> Result<String> {
    let product = Product::create(
        &db,
        Product {
            name: form.name,
            price: form.price,
            ..Default::default()
        },
    )
    .await?;
    Ok(format!("created {}", product.id))
}

#[derive(Deserialize)]
struct Search {
    q: String,
}

impl Validate for Search {
    fn rules(&self, v: &mut Validator) {
        v.field("q", &self.q).min(3);
    }
}

async fn search(Valid(search): Valid<Search>) -> String {
    format!("searching {}", search.q)
}

async fn stock(Form(form): Form<serde_json::Value>) -> Result<String> {
    let mut errors = Errors::new();
    errors.add("quantity", "Not enough stock.");
    Err(ValidationError::new(errors).with_input(&form).into())
}

struct Client {
    kernel: Kernel,
    cookie: Option<String>,
    token: String,
}

struct Reply {
    status: StatusCode,
    location: Option<String>,
    content_type: Option<String>,
    body: String,
}

impl Client {
    async fn new(locale: &str) -> (Self, tempfile::TempDir) {
        let (kernel, dir) = kernel(locale).await;
        let mut client = Self {
            kernel,
            cookie: None,
            token: String::new(),
        };
        let page = client.get("/products/create").await.body;
        client.token = page.rsplit('>').next().unwrap().to_owned();
        (client, dir)
    }

    async fn send(&mut self, mut req: Request<Body>) -> Reply {
        if let Some(cookie) = &self.cookie {
            req.headers_mut().insert(COOKIE, cookie.parse().unwrap());
        }
        let res = self.kernel.router().oneshot(req).await.unwrap();
        if let Some(set) = res.headers().get(SET_COOKIE) {
            self.cookie = Some(set.to_str().unwrap().split(';').next().unwrap().to_owned());
        }
        let header = |name| {
            res.headers()
                .get(name)
                .map(|v| v.to_str().unwrap().to_owned())
        };
        let (status, location, content_type) =
            (res.status(), header(LOCATION), header(CONTENT_TYPE));
        let body = res.into_body().collect().await.unwrap().to_bytes();
        Reply {
            status,
            location,
            content_type,
            body: String::from_utf8(body.to_vec()).unwrap(),
        }
    }

    async fn get(&mut self, uri: &str) -> Reply {
        self.send(Request::get(uri).body(Body::empty()).unwrap())
            .await
    }

    async fn post(&mut self, uri: &str, body: &str, htmx: bool) -> Reply {
        let mut req = Request::post(uri)
            .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
            .header("referer", "/products/create");
        if htmx {
            req = req
                .header("hx-request", "true")
                .header("x-csrf-token", &self.token);
        }
        let body = if htmx {
            body.to_owned()
        } else {
            format!("_token={}&{body}", self.token)
        };
        self.send(req.body(Body::from(body)).unwrap()).await
    }
}

#[tokio::test]
async fn valid_forms_reach_the_handler() {
    let (mut client, _dir) = Client::new("en").await;
    let reply = client
        .post(
            "/products",
            "name=Pulled+Tea&price=12000&agree=true&category=",
            false,
        )
        .await;
    assert_eq!(
        (reply.status, reply.body.as_str()),
        (StatusCode::OK, "created 2")
    );
}

#[tokio::test]
async fn invalid_forms_redirect_back_with_errors_and_old_input() {
    let (mut client, _dir) = Client::new("es").await;
    let reply = client
        .post(
            "/products",
            "name=Black+Coffee&price=abc&agree=true&password=secret",
            false,
        )
        .await;
    assert_eq!(reply.status, StatusCode::SEE_OTHER);
    assert_eq!(reply.location.as_deref(), Some("/products/create"));

    let page = client.get("/products/create").await.body;
    assert!(
        page.contains(r#"<input name="name" value="Black Coffee">"#),
        "{page}"
    );
    assert!(
        page.contains(r#"<input name="password" value="">"#),
        "passwords are never flashed"
    );
    assert!(
        page.contains("<p>El campo precio debe ser un número.</p>"),
        "{page}"
    );

    let reply = client
        .post(
            "/products",
            "name=Black+Coffee&price=5000&agree=true",
            false,
        )
        .await;
    assert_eq!(reply.status, StatusCode::SEE_OTHER);
    let page = client.get("/products/create").await.body;
    assert!(
        page.contains(r#"<p data-error-for="name">El campo nombre ya está en uso.</p>"#),
        "{page}"
    );

    let page = client.get("/products/create").await.body;
    assert!(!page.contains("ya está en uso"), "errors last one request");
}

#[tokio::test]
async fn htmx_and_json_requests_get_422_json() {
    let (mut client, _dir) = Client::new("en").await;
    let reply = client
        .post("/products", "name=&price=5000&agree=true", true)
        .await;
    assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(reply.content_type.as_deref(), Some("application/json"));
    let body: serde_json::Value = serde_json::from_str(&reply.body).unwrap();
    assert_eq!(body["errors"]["name"][0], "The name field is required.");
    assert_eq!(body["message"], "The name field is required.");

    let token = client.token.clone();
    let reply = client
        .send(
            Request::post("/products")
                .header(CONTENT_TYPE, "application/json")
                .header("x-csrf-token", token)
                .body(Body::from(r#"{"name": "Iced Tea", "price": "cheap"}"#))
                .unwrap(),
        )
        .await;
    assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY);
    let body: serde_json::Value = serde_json::from_str(&reply.body).unwrap();
    assert_eq!(body["errors"]["price"][0], "The price must be a number.");

    // JSON bodies report every field too: a missing one, a wrong type, a rule.
    let token = client.token.clone();
    let reply = client
        .send(
            Request::post("/products")
                .header(CONTENT_TYPE, "application/json")
                .header("x-csrf-token", token)
                .body(Body::from(r#"{"price": true, "email": "x"}"#))
                .unwrap(),
        )
        .await;
    let body: serde_json::Value = serde_json::from_str(&reply.body).unwrap();
    assert_eq!(body["errors"]["name"][0], "The name field is required.");
    assert_eq!(body["errors"]["price"][0], "The price must be a number.");
    assert!(
        body["errors"]["email"][0]
            .as_str()
            .unwrap()
            .contains("email")
    );

    // Errors for API clients are JSON too, from the handler or from CSRF.
    let reply = client
        .send(
            Request::post("/products")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"name": "Coffee"}"#))
                .unwrap(),
        )
        .await;
    assert_eq!(reply.status.as_u16(), 419);
    assert_eq!(reply.body, r#"{"message":"Page Expired"}"#);
    let reply = client
        .send(
            Request::get("/nowhere")
                .header("accept", "application/json")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(reply.status, StatusCode::NOT_FOUND);
    assert_eq!(reply.body, r#"{"message":"Not Found"}"#);

    // A field that doesn't parse doesn't hide the other fields' errors, and
    // its placeholder value (0, below the minimum) adds no error of its own.
    let reply = client
        .post("/products", "name=&price=cheap&agree=maybe&email=x", true)
        .await;
    let body: serde_json::Value = serde_json::from_str(&reply.body).unwrap();
    assert_eq!(body["errors"]["name"][0], "The name field is required.");
    assert_eq!(body["errors"]["price"].as_array().unwrap().len(), 1);
    assert_eq!(body["errors"]["price"][0], "The price must be a number.");
    assert_eq!(body["errors"]["agree"].as_array().unwrap().len(), 1);
    assert!(
        body["errors"]["email"][0]
            .as_str()
            .unwrap()
            .contains("email")
    );
}

#[tokio::test]
async fn query_strings_are_validated_for_get() {
    let (mut client, _dir) = Client::new("en").await;
    assert_eq!(
        client.get("/search?q=coffee").await.body,
        "searching coffee"
    );
    let reply = client
        .send(
            Request::get("/search?q=co")
                .header("hx-request", "true")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn handlers_can_return_their_own_validation_errors() {
    let (mut client, _dir) = Client::new("en").await;
    let reply = client.post("/stock", "quantity=99", true).await;
    assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(reply.body.contains("Not enough stock."));
    let reply = client.post("/stock", "quantity=99", false).await;
    assert_eq!(reply.status, StatusCode::SEE_OTHER);
}

/// Runs `rules` on a validator and returns the errors (no database checks).
async fn errors_of(rules: impl Fn(&mut Validator)) -> Errors {
    struct Rules<F>(F);
    impl<F: Fn(&mut Validator)> Validate for Rules<F> {
        fn rules(&self, v: &mut Validator) {
            (self.0)(v)
        }
    }
    let app = renox::testing::TestApp::new(App::new()).await;
    Validator::rules_of(&Rules(rules))
        .finish(app.db())
        .await
        .unwrap()
}

fn png(width: u32, height: u32) -> renox::Upload {
    let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
    bytes.extend_from_slice(&width.to_be_bytes());
    bytes.extend_from_slice(&height.to_be_bytes());
    renox::Upload::new("banner.png", "image/png", bytes)
}

#[renox::test]
async fn the_rules_added_after_the_parity_review() {
    use renox::validation::Dimensions;
    let errors = errors_of(|v| {
        // Field against field: numbers, text lengths, dates.
        v.field("max_price", &10).gt("min_price", &20);
        v.field("max_ok", &30).gt("min_price", &20);
        v.field("nickname", &"Al").gte("name", &"Alex");
        v.field("ends_on", &"2026-10-01")
            .lt("starts_on", &"2026-09-01".to_owned());
        v.field("ends_ok", &"2026-10-01").gte(
            "starts_on",
            &renox::chrono::NaiveDate::from_ymd_opt(2026, 9, 1).unwrap(),
        );
        v.field("count", &3).lte("limit", &Option::<i64>::None); // the other is empty: skipped
        // Decimals, digits, multiples, numbers in text.
        v.field("price", &"12.5").decimal(2, 2);
        v.field("price_ok", &"12.50").decimal(2, 2);
        v.field("rate", &"1.12345").decimal(1, 4);
        v.field("pin", &123).min_digits(4);
        v.field("year", &20260).max_digits(4);
        v.field("amount", &1250).multiple_of(500);
        v.field("amount_ok", &1500).multiple_of(500);
        v.field("quarter", &0.75).multiple_of(0.25);
        v.field("qty", &"12a").numeric();
        v.field("qty_ok", &" -3.5 ").numeric();
        v.field("whole", &"3.5").integer();
        // Formats.
        v.field("settings", &"{\"a\": 1").json();
        v.field("settings_ok", &"{\"a\": [1, 2]}").json();
        v.field("ulid", &"01ARZ3NDEKTSV4RRFFQ69G5FAI").ulid();
        v.field("ulid_ok", &"01arz3ndektsv4rrffq69g5fav").ulid();
        v.field("zone", &"Asia/Jakarta").timezone();
        v.field("zone_bad", &"Mars/Olympus").timezone();
        v.field("mac", &"00:1A:2B:3C:4D").mac_address();
        v.field("mac_ok", &"001A.2B3C.4D5E").mac_address();
        v.field("colour", &"#12345").hex_color();
        v.field("colour_ok", &"#4f46e5").hex_color();
        v.field("code", &"kopi→").ascii();
        v.field("handle", &"admin-alex")
            .doesnt_start_with(&["admin", "root"]);
        v.field("email", &"a@test.invalid")
            .doesnt_end_with(&[".invalid"]);
        v.field("sku", &"TMP-1").not_matches(r"^TMP-");
        // Presence.
        v.field("coupon", &"SAVE10").prohibits("gift_card", &"GC-1");
        v.field("internal_note", &"hi").prohibited();
        v.field("discount", &"5").prohibited_unless(false);
        v.field("phone", &Option::<String>::None)
            .required_without_all(&[&Option::<String>::None, &""]);
        v.field("postcode", &"")
            .required_with_all(&[&"1 Main St", &"Springfield"]);
        v.field("share_data", &true).declined();
        v.field("share_ok", &"no").declined();
        v.field("terms", &false).accepted_if(true);
        v.field("banner", &Some(png(800, 600)))
            .dimensions(&Dimensions::new().min_width(1200));
        v.field("banner_ok", &Some(png(1600, 900)))
            .dimensions(&Dimensions::new().min_width(1200).ratio(16, 9));
        v.field("square", &Some(png(400, 410)))
            .dimensions(&Dimensions::new().ratio(1, 1));
    })
    .await;

    let expect = [
        ("max_price", "The max price must be greater than min price."),
        ("nickname", "The nickname must be at least as long as name."),
        ("ends_on", "The ends on must be before starts on."),
        ("price", "The price must have 2 decimal places."),
        ("rate", "The rate must have 1-4 decimal places."),
        ("pin", "The pin must have at least 4 digits."),
        ("year", "The year must not have more than 4 digits."),
        ("amount", "The amount must be a multiple of 500."),
        ("qty", "The qty must be a number."),
        ("whole", "The whole must be a whole number."),
        ("settings", "The settings must be valid JSON."),
        ("ulid", "The ulid must be a valid ULID."),
        ("zone_bad", "The zone bad must be a valid time zone."),
        ("mac", "The mac must be a valid MAC address."),
        ("colour", "The colour must be a valid hexadecimal colour."),
        ("code", "The code may only contain ASCII characters."),
        (
            "handle",
            "The handle may not start with one of: admin, root.",
        ),
        ("email", "The email may not end with one of: .invalid."),
        ("sku", "The sku format is invalid."),
        (
            "coupon",
            "The coupon field can't be sent together with gift card.",
        ),
        (
            "internal_note",
            "The internal note field must be empty here.",
        ),
        ("discount", "The discount field must be empty here."),
        ("phone", "The phone field is required."),
        ("postcode", "The postcode field is required."),
        ("share_data", "The share data must be declined."),
        ("terms", "The terms must be accepted."),
        ("banner", "The banner has invalid image dimensions."),
        ("square", "The square has invalid image dimensions."),
    ];
    for (field, message) in expect {
        assert_eq!(errors.first(field), Some(message), "{field}");
    }
    for field in [
        "max_ok",
        "ends_ok",
        "count",
        "price_ok",
        "amount_ok",
        "quarter",
        "qty_ok",
        "settings_ok",
        "ulid_ok",
        "zone",
        "mac_ok",
        "colour_ok",
        "share_ok",
        "banner_ok",
    ] {
        assert!(!errors.has(field), "{field}: {:?}", errors.first(field));
    }
    let all: Vec<&str> = errors.iter().map(|(field, _)| field).collect();
    assert_eq!(all.len(), expect.len(), "unexpected errors: {all:?}");
}

#[derive(Deserialize, Serialize, renox::Validate)]
struct Range {
    #[validate(required, integer)]
    min: String,
    #[validate(required, gt("min", &self.min), multiple_of(5))]
    max: String,
    #[validate(decimal(0, 2), prohibits("max", &self.max))]
    exact: Option<String>,
}

#[renox::test]
async fn the_derive_takes_the_new_rules_too() {
    let app = renox::testing::TestApp::new(App::new()).await;
    let range = Range {
        min: "20".into(),
        max: "15".into(),
        exact: Some("1.234".into()),
    };
    let errors = Validator::rules_of(&range).finish(app.db()).await.unwrap();
    assert_eq!(errors.first("min"), None);
    // Two texts that read as numbers compare as numbers.
    assert_eq!(
        errors.first("max"),
        Some("The max must be greater than min.")
    );
    assert_eq!(
        errors.first("exact"),
        Some("The exact must have 0-2 decimal places.")
    );
}
