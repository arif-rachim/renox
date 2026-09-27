//! Renox's own test helpers, used the way an app would use them.

use renox::prelude::*;
use renox::testing::TestApp;
use serde::{Deserialize, Serialize};

#[derive(Model, Serialize, Deserialize, Default)]
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

#[derive(Deserialize, Serialize)]
struct ProdukForm {
    nama: String,
    harga: i64,
}

impl Validate for ProdukForm {
    fn rules(&self, v: &mut Validator) {
        v.field("nama", &self.nama).required();
        v.field("harga", &self.harga).min(1000);
    }
}

#[derive(Serialize, Deserialize)]
struct HitungStok;

impl Job for HitungStok {
    const NAME: &'static str = "hitung-stok";

    async fn handle(self, _: JobContext) -> Result {
        Ok(())
    }
}

struct Toko;

impl Module for Toko {
    fn name(&self) -> &'static str {
        "toko"
    }

    fn routes(&self) -> Routes {
        Routes::new()
            .get("/", || async { view("shop/home.html", ()) })
            .name("home")
            .post("/produk", simpan)
            .post("/stok", |State(state): State<AppState>| async move {
                state.dispatch(HitungStok).await?;
                state
                    .queue_mail(renox::mail::Mail::new(
                        "gudang@toko.id",
                        "Stok dihitung",
                        "ok",
                    ))
                    .await?;
                Ok::<_, Error>("queued")
            })
            .merge(
                Routes::new()
                    .get("/dashboard", |auth: AuthUser| async move {
                        format!("dashboard {}", auth.name)
                    })
                    .require_auth(),
            )
    }

    fn register(&self, app: &mut Registry) {
        app.job::<HitungStok>();
    }
}

async fn simpan(State(db): State<Db>, Valid(form): Valid<ProdukForm>) -> Result<Redirect> {
    Produk::create(
        &db,
        Produk {
            nama: form.nama,
            harga: form.harga,
            ..Default::default()
        },
    )
    .await?;
    Ok(Redirect::to("/"))
}

fn app() -> App {
    App::new()
        .migrations(renox::migrations!("tests/migrations"))
        .module(Auth::new())
        .module(Toko)
}

async fn test_app() -> TestApp {
    TestApp::with_config(app(), |c| c.views_path = "tests/views".into()).await
}

#[renox::test]
async fn pages_forms_and_the_database() {
    let app = test_app().await;
    app.get("/")
        .await
        .assert_ok()
        .assert_see("<h1>Toko</h1>")
        .assert_dont_see("Halo");

    app.post("/produk", &[("nama", "Kopi"), ("harga", "18000")])
        .await
        .assert_redirect("/");
    app.assert_database_has("produk", &[("nama", &"Kopi"), ("harga", &18000)])
        .await;
    app.assert_database_missing("produk", &[("nama", &"Teh")])
        .await;
    app.assert_database_count("produk", 1).await;
    app.assert_database_has("produk", &[("kategori", &None::<String>)])
        .await;
}

#[renox::test]
async fn csrf_and_validation() {
    let app = test_app().await;
    app.request()
        .without_csrf()
        .post("/produk", &[("nama", "Kopi"), ("harga", "18000")])
        .await
        .assert_status(419);

    app.htmx()
        .post("/produk", &[("nama", ""), ("harga", "500")])
        .await
        .assert_invalid("nama")
        .assert_invalid("harga");
    app.request()
        .json()
        .post("/produk", &[("harga", "10")])
        .await
        .assert_invalid("harga");
    app.post_json(
        "/produk",
        &serde_json::json!({ "nama": "Teh", "harga": 5000 }),
    )
    .await
    .assert_redirect("/");
    app.assert_database_count("produk", 1).await;
}

#[renox::test]
async fn acting_as_a_user() {
    let app = test_app().await;
    let user = User::register(app.db(), "Arif", "arif@example.com", "rahasia123")
        .await
        .unwrap();

    app.get("/dashboard").await.assert_redirect("/login");
    app.request()
        .json()
        .get("/dashboard")
        .await
        .assert_unauthorized();

    app.acting_as(&user)
        .get("/dashboard")
        .await
        .assert_ok()
        .assert_see("dashboard Arif");
    app.get("/").await.assert_see("Halo Arif");
    app.logout()
        .get("/dashboard")
        .await
        .assert_redirect("/login");
}

#[renox::test]
async fn queue_and_mail_fakes() {
    let app = test_app().await;
    app.post("/stok", &[])
        .await
        .assert_ok()
        .assert_see("queued");
    assert_eq!(app.queued_jobs().await, ["hitung-stok", "renox.send-mail"]);
    assert!(app.sent_mail().is_empty());
    assert_eq!(app.run_jobs().await, 2);
    app.assert_mail_sent("gudang@toko.id", "Stok");
}

#[renox::test]
#[should_panic(expected = "expected to see \"Harga\" in:")]
async fn failed_assertions_explain_themselves() {
    let app = test_app().await;
    app.get("/").await.assert_see("Harga");
}
