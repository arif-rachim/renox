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
#[model(table = "produk", soft_deletes)]
struct Produk {
    id: i64,
    nama: String,
    harga: i64,
    kategori: Option<String>,
    created_at: Option<DateTime>,
    updated_at: Option<DateTime>,
    deleted_at: Option<DateTime>,
}

#[derive(Deserialize, Serialize, Default)]
struct ProdukForm {
    nama: String,
    harga: i64,
    kategori: Option<String>,
    email: Option<String>,
    website: Option<String>,
    password: Option<String>,
    password_confirmation: Option<String>,
    setuju: Option<bool>,
    #[serde(skip)]
    ignore_id: Option<i64>,
}

impl Validate for ProdukForm {
    fn rules(&self, v: &mut Validator) {
        let nama = v
            .field("nama", &self.nama)
            .required()
            .between(3, 20)
            .unique("produk", "nama");
        if let Some(id) = self.ignore_id {
            nama.ignore(id);
        }
        v.field("harga", &self.harga)
            .label("harga jual")
            .min(1_000)
            .max(1_000_000);
        v.field("kategori", &self.kategori)
            .one_of(&["kopi", "teh"])
            .exists("produk", "kategori");
        v.field("email", &self.email).email();
        v.field("website", &self.website).url();
        v.field("password", &self.password)
            .min(8)
            .confirmed(&self.password_confirmation);
        v.field("setuju", &self.setuju)
            .accepted()
            .message("Centang dulu persetujuannya.");
    }
}

fn valid_form() -> ProdukForm {
    ProdukForm {
        nama: "Kopi Susu".into(),
        harga: 18_000,
        setuju: Some(true),
        ..Default::default()
    }
}

async fn kernel(locale: &str) -> (Kernel, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("form.html"),
        r#"<input name="nama" value="{{ old('nama') }}"><input name="password" value="{{ old('password') }}"><p data-error-for="nama">{{ error('nama') }}</p><p>{{ error('harga') }}</p>{{ csrf_token }}"#,
    )
    .unwrap();
    let config = Config {
        env: Environment::Testing,
        key: Some(renox::generate_key()),
        views_path: dir.path().to_path_buf(),
        locale: locale.into(),
        ..Config::default()
    };
    let kernel = App::with_config(config)
        .migrations(renox::migrations!("tests/migrations"))
        .module(Shop)
        .boot()
        .await
        .unwrap();
    kernel.migrate().await.unwrap();
    Produk::create(
        kernel.db(),
        Produk {
            nama: "Kopi Hitam".into(),
            harga: 15_000,
            kategori: Some("kopi".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    (kernel, dir)
}

async fn errors_for(form: &ProdukForm, locale: Locale) -> Errors {
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
    let form = ProdukForm {
        nama: "Ko".into(),
        harga: 500,
        kategori: Some("susu".into()),
        email: Some("bukan-email".into()),
        website: Some("renox.dev".into()),
        password: Some("rahasia123".into()),
        password_confirmation: Some("beda".into()),
        setuju: Some(false),
        ..Default::default()
    };
    let errors = errors_for(&form, Locale::En).await;
    assert_eq!(
        errors.first("nama"),
        Some("The nama must be between 3 and 20 characters.")
    );
    assert_eq!(
        errors.first("harga"),
        Some("The harga jual must be at least 1000.")
    );
    assert_eq!(
        errors.first("kategori"),
        Some("The selected kategori is invalid.")
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
    assert_eq!(errors.first("setuju"), Some("Centang dulu persetujuannya."));
    assert!(errors.iter().all(|(_, messages)| messages.len() == 1));
}

#[tokio::test]
async fn messages_come_in_indonesian() {
    let form = ProdukForm {
        nama: " ".into(),
        harga: 2_000_000,
        setuju: Some(true),
        ..Default::default()
    };
    let errors = errors_for(&form, Locale::Id).await;
    assert_eq!(errors.first("nama"), Some("Nama wajib diisi."));
    assert_eq!(errors.first("harga"), Some("Harga jual maksimal 1000000."));
}

#[tokio::test]
async fn unique_and_exists_query_the_database() {
    let taken = ProdukForm {
        nama: "Kopi Hitam".into(),
        ..valid_form()
    };
    assert_eq!(
        errors_for(&taken, Locale::Id).await.first("nama"),
        Some("Nama sudah digunakan.")
    );

    let editing_itself = ProdukForm {
        nama: "Kopi Hitam".into(),
        ignore_id: Some(1),
        ..valid_form()
    };
    assert!(errors_for(&editing_itself, Locale::Id).await.is_empty());

    let no_teh_yet = ProdukForm {
        kategori: Some("teh".into()),
        ..valid_form()
    };
    assert_eq!(
        errors_for(&no_teh_yet, Locale::En).await.first("kategori"),
        Some("The selected kategori is invalid.")
    );
    let has_kopi = ProdukForm {
        kategori: Some("kopi".into()),
        ..valid_form()
    };
    assert!(errors_for(&has_kopi, Locale::En).await.is_empty());
}

struct Shop;

impl Module for Shop {
    fn name(&self) -> &'static str {
        "shop"
    }

    fn routes(&self) -> Routes {
        Routes::new()
            .get("/produk/create", || async { view("form.html", ()) })
            .post("/produk", store)
            .get("/cari", search)
            .post("/stok", stok)
    }
}

async fn store(State(db): State<Db>, Valid(form): Valid<ProdukForm>) -> Result<String> {
    let produk = Produk::create(
        &db,
        Produk {
            nama: form.nama,
            harga: form.harga,
            ..Default::default()
        },
    )
    .await?;
    Ok(format!("created {}", produk.id))
}

#[derive(Deserialize)]
struct Cari {
    q: String,
}

impl Validate for Cari {
    fn rules(&self, v: &mut Validator) {
        v.field("q", &self.q).min(3);
    }
}

async fn search(Valid(cari): Valid<Cari>) -> String {
    format!("mencari {}", cari.q)
}

async fn stok(Form(form): Form<serde_json::Value>) -> Result<String> {
    let mut errors = Errors::new();
    errors.add("jumlah", "Stok tidak cukup.");
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
        let page = client.get("/produk/create").await.body;
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
            .header("referer", "/produk/create");
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
            "/produk",
            "nama=Teh+Tarik&harga=12000&setuju=true&kategori=",
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
    let (mut client, _dir) = Client::new("id").await;
    let reply = client
        .post(
            "/produk",
            "nama=Kopi+Hitam&harga=abc&setuju=true&password=rahasia",
            false,
        )
        .await;
    assert_eq!(reply.status, StatusCode::SEE_OTHER);
    assert_eq!(reply.location.as_deref(), Some("/produk/create"));

    let page = client.get("/produk/create").await.body;
    assert!(
        page.contains(r#"<input name="nama" value="Kopi Hitam">"#),
        "{page}"
    );
    assert!(
        page.contains(r#"<input name="password" value="">"#),
        "passwords are never flashed"
    );
    assert!(page.contains("<p>Harga harus berupa angka.</p>"), "{page}");

    let reply = client
        .post("/produk", "nama=Kopi+Hitam&harga=5000&setuju=true", false)
        .await;
    assert_eq!(reply.status, StatusCode::SEE_OTHER);
    let page = client.get("/produk/create").await.body;
    assert!(
        page.contains(r#"<p data-error-for="nama">Nama sudah digunakan.</p>"#),
        "{page}"
    );

    let page = client.get("/produk/create").await.body;
    assert!(!page.contains("sudah digunakan"), "errors last one request");
}

#[tokio::test]
async fn htmx_and_json_requests_get_422_json() {
    let (mut client, _dir) = Client::new("en").await;
    let reply = client
        .post("/produk", "nama=&harga=5000&setuju=true", true)
        .await;
    assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(reply.content_type.as_deref(), Some("application/json"));
    let body: serde_json::Value = serde_json::from_str(&reply.body).unwrap();
    assert_eq!(body["errors"]["nama"][0], "The nama field is required.");
    assert_eq!(body["message"], "The nama field is required.");

    let token = client.token.clone();
    let reply = client
        .send(
            Request::post("/produk")
                .header(CONTENT_TYPE, "application/json")
                .header("x-csrf-token", token)
                .body(Body::from(r#"{"nama": "Es Teh", "harga": "murah"}"#))
                .unwrap(),
        )
        .await;
    assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY);
    let body: serde_json::Value = serde_json::from_str(&reply.body).unwrap();
    assert_eq!(body["errors"]["harga"][0], "The harga must be a number.");
}

#[tokio::test]
async fn query_strings_are_validated_for_get() {
    let (mut client, _dir) = Client::new("en").await;
    assert_eq!(client.get("/cari?q=kopi").await.body, "mencari kopi");
    let reply = client
        .send(
            Request::get("/cari?q=ko")
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
    let reply = client.post("/stok", "jumlah=99", true).await;
    assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(reply.body.contains("Stok tidak cukup."));
    let reply = client.post("/stok", "jumlah=99", false).await;
    assert_eq!(reply.status, StatusCode::SEE_OTHER);
}
