use axum::body::Body;
use axum::http::header::{CONTENT_TYPE, COOKIE, LOCATION, SET_COOKIE};
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use renox::Kernel;
use renox::prelude::*;
use renox::validation::Locale;
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

async fn errors_for(form: &ProductForm, locale: Locale) -> Errors {
    let (kernel, _dir) = kernel("en").await;
    Validator::rules_of(form, locale)
        .finish(kernel.db())
        .await
        .unwrap()
}

#[tokio::test]
async fn valid_input_has_no_errors() {
    assert!(errors_for(&valid_form(), Locale::En).await.is_empty());
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
    let errors = errors_for(&form, Locale::En).await;
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
        errors_for(&taken, Locale::En).await.first("name"),
        Some("The name has already been taken.")
    );

    let editing_itself = ProductForm {
        name: "Black Coffee".into(),
        ignore_id: Some(1),
        ..valid_form()
    };
    assert!(errors_for(&editing_itself, Locale::En).await.is_empty());

    let no_tea_yet = ProductForm {
        category: Some("tea".into()),
        ..valid_form()
    };
    assert_eq!(
        errors_for(&no_tea_yet, Locale::En).await.first("category"),
        Some("The selected category is invalid.")
    );
    let has_coffee = ProductForm {
        category: Some("coffee".into()),
        ..valid_form()
    };
    assert!(errors_for(&has_coffee, Locale::En).await.is_empty());
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
