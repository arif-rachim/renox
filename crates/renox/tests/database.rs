use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use renox::db::{DbValue, Migration};
use renox::prelude::*;
use renox::{Kernel, fake::Fake};
use serde::Serialize;
use tower::ServiceExt;

#[derive(Model, Serialize, Default, Debug, Clone, PartialEq)]
#[model(table = "produk", soft_deletes)]
struct Produk {
    id: i64,
    nama: String,
    harga: i64,
    kategori: Option<String>,
    #[model(skip)]
    label: String,
    created_at: Option<DateTime>,
    updated_at: Option<DateTime>,
    deleted_at: Option<DateTime>,
}

impl Factory for Produk {
    fn definition() -> Self {
        Produk {
            nama: (5..12).fake::<String>(),
            harga: (1_000..50_000).fake(),
            ..Default::default()
        }
    }
}

#[derive(Model, Serialize, Default, Debug)]
struct Catatan {
    id: i64,
    produk_id: Option<i64>,
    isi: String,
}

fn produk(nama: &str, harga: i64, kategori: Option<&str>) -> Produk {
    Produk {
        nama: nama.into(),
        harga,
        kategori: kategori.map(Into::into),
        ..Default::default()
    }
}

fn config(views: &std::path::Path) -> Config {
    Config {
        env: Environment::Testing,
        key: Some(renox::generate_key()),
        views_path: views.to_path_buf(),
        ..Config::default()
    }
}

async fn kernel_with(app: impl FnOnce(App) -> App) -> (Kernel, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let kernel = app(
        App::with_config(config(dir.path())).migrations(renox::migrations!("tests/migrations"))
    )
    .boot()
    .await
    .unwrap();
    kernel.migrate().await.unwrap();
    (kernel, dir)
}

async fn kernel() -> (Kernel, tempfile::TempDir) {
    kernel_with(|app| app).await
}

#[test]
fn migrations_macro_embeds_files_in_order() {
    let migrations: &[Migration] = renox::migrations!("tests/migrations");
    let names: Vec<_> = migrations.iter().map(|m| m.name).collect();
    assert_eq!(
        names,
        [
            "20260101000000_create_produk",
            "20260102000000_create_catatan"
        ]
    );
    assert!(migrations[0].up.contains("CREATE TABLE produk"));
    // Trimmed: Git may check files out with CRLF line endings on Windows.
    assert_eq!(
        migrations[0].down.map(str::trim),
        Some("DROP TABLE produk;")
    );
    assert!(
        renox::migrations!("tests/migrations_plain")[0]
            .down
            .is_none()
    );
    assert!(renox::migrations!("tests/does_not_exist").is_empty());
}

#[tokio::test]
async fn migrations_run_in_batches_and_roll_back() {
    let (kernel, _dir) = kernel().await;
    assert!(
        kernel.migrate().await.unwrap().is_empty(),
        "nothing left to run"
    );
    let status = kernel.migration_status().await.unwrap();
    assert!(status.iter().all(|m| m.batch == Some(1)));

    let rolled = kernel.rollback(1).await.unwrap();
    // Newest first; every app also gets the framework's jobs tables.
    assert_eq!(
        rolled,
        [
            "20260102000000_create_catatan",
            "20260101000000_create_produk",
            "00010101000100_create_jobs_table"
        ]
    );
    assert!(Produk::all(kernel.db()).await.is_err(), "table is gone");

    assert_eq!(kernel.migrate().await.unwrap().len(), 3);
    Produk::create(kernel.db(), produk("Kopi", 1, None))
        .await
        .unwrap();
    assert_eq!(kernel.fresh().await.unwrap().len(), 3);
    assert!(
        Produk::all(kernel.db()).await.unwrap().is_empty(),
        "fresh drops data"
    );
}

#[tokio::test]
async fn migrations_without_down_cannot_roll_back() {
    let dir = tempfile::tempdir().unwrap();
    let kernel = App::with_config(config(dir.path()))
        .migrations(renox::migrations!("tests/migrations_plain"))
        .boot()
        .await
        .unwrap();
    kernel.migrate().await.unwrap();
    let err = kernel.rollback(1).await.unwrap_err();
    assert!(format!("{err:?}").contains("has no .down.sql"));
}

#[tokio::test]
async fn duplicate_migrations_fail_at_boot() {
    let dir = tempfile::tempdir().unwrap();
    let result = App::with_config(config(dir.path()))
        .migrations(renox::migrations!("tests/migrations"))
        .migrations(renox::migrations!("tests/migrations"))
        .boot()
        .await;
    assert!(format!("{:?}", result.err().unwrap()).contains("registered twice"));
}

#[tokio::test]
async fn file_databases_use_wal_and_create_their_directory() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("storage/app.db");
    let config = Config {
        database_url: format!("sqlite://{}", path.display()),
        ..config(dir.path())
    };
    let kernel = App::with_config(config).boot().await.unwrap();
    let mode: String = renox::sqlx::query_scalar("PRAGMA journal_mode")
        .fetch_one(kernel.db())
        .await
        .unwrap();
    assert_eq!(mode, "wal");
    assert!(path.exists());
}

#[tokio::test]
async fn save_inserts_then_updates_with_timestamps() {
    let (kernel, _dir) = kernel().await;
    let db = kernel.db();

    let mut kopi = produk("Kopi", 18_000, Some("minuman"));
    kopi.save(db).await.unwrap();
    assert!(kopi.id > 0);
    let created = kopi.created_at.unwrap();
    assert_eq!(kopi.updated_at, Some(created));

    kopi.harga = 20_000;
    kopi.label = "not stored".into();
    kopi.save(db).await.unwrap();
    assert_eq!(kopi.created_at, Some(created), "created_at is kept");

    let found = Produk::find(db, kopi.id).await.unwrap().unwrap();
    assert_eq!(found.harga, 20_000);
    assert_eq!(found.label, "", "skipped fields load as Default");
    assert_eq!(found.created_at, Some(created));
    assert!(Produk::find(db, 999).await.unwrap().is_none());
}

#[tokio::test]
async fn soft_deletes_hide_rows_until_restored() {
    let (kernel, _dir) = kernel().await;
    let db = kernel.db();
    let mut kopi = Produk::create(db, produk("Kopi", 1, None)).await.unwrap();
    Produk::create(db, produk("Teh", 2, None)).await.unwrap();

    kopi.delete(db).await.unwrap();
    assert!(kopi.deleted_at.is_some());
    assert_eq!(Produk::query().count(db).await.unwrap(), 1);
    assert_eq!(Produk::query().with_trashed().count(db).await.unwrap(), 2);
    assert_eq!(
        Produk::query().only_trashed().get(db).await.unwrap()[0].nama,
        "Kopi"
    );
    assert!(Produk::find(db, kopi.id).await.unwrap().is_none());

    kopi.restore(db).await.unwrap();
    assert_eq!(Produk::query().count(db).await.unwrap(), 2);

    kopi.force_delete(db).await.unwrap();
    assert_eq!(Produk::query().with_trashed().count(db).await.unwrap(), 1);
}

#[tokio::test]
async fn models_without_soft_deletes_delete_rows() {
    let (kernel, _dir) = kernel().await;
    let db = kernel.db();
    let mut catatan = Catatan::create(
        db,
        Catatan {
            isi: "halo".into(),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert!(catatan.restore(db).await.is_err());
    catatan.delete(db).await.unwrap();
    assert_eq!(Catatan::query().count(db).await.unwrap(), 0);
}

#[tokio::test]
async fn queries_filter_order_and_limit() {
    let (kernel, _dir) = kernel().await;
    let db = kernel.db();
    for (nama, harga, kategori) in [
        ("Kopi", 18_000, Some("minuman")),
        ("Teh", 8_000, Some("minuman")),
        ("Roti", 12_000, Some("makanan")),
        ("Air", 5_000, None),
    ] {
        Produk::create(db, produk(nama, harga, kategori))
            .await
            .unwrap();
    }
    let names = |list: Vec<Produk>| list.into_iter().map(|p| p.nama).collect::<Vec<_>>();

    let minuman = Produk::where_eq("kategori", "minuman")
        .order_by("nama")
        .get(db)
        .await
        .unwrap();
    assert_eq!(names(minuman), ["Kopi", "Teh"]);

    let murah = Produk::query()
        .where_op("harga", "<", 10_000)
        .order_by_desc("harga")
        .get(db)
        .await
        .unwrap();
    assert_eq!(names(murah), ["Teh", "Air"]);

    let dipilih = Produk::query()
        .where_in("nama", ["Air", "Roti"])
        .order_by("id")
        .get(db)
        .await
        .unwrap();
    assert_eq!(names(dipilih), ["Roti", "Air"]);
    assert!(
        Produk::query()
            .where_in("nama", Vec::<String>::new())
            .get(db)
            .await
            .unwrap()
            .is_empty()
    );

    assert_eq!(
        names(
            Produk::query()
                .where_null("kategori")
                .get(db)
                .await
                .unwrap()
        ),
        ["Air"]
    );
    assert_eq!(
        Produk::query()
            .where_not_null("kategori")
            .count(db)
            .await
            .unwrap(),
        3
    );
    assert_eq!(
        names(
            Produk::query()
                .where_like("nama", "%o%")
                .order_by("nama")
                .get(db)
                .await
                .unwrap()
        ),
        ["Kopi", "Roti"]
    );

    let halaman = Produk::query()
        .order_by("harga")
        .limit(2)
        .offset(1)
        .get(db)
        .await
        .unwrap();
    assert_eq!(names(halaman), ["Teh", "Roti"]);
    assert_eq!(
        Produk::query()
            .latest()
            .first(db)
            .await
            .unwrap()
            .unwrap()
            .nama,
        "Air"
    );
    assert!(Produk::where_eq("nama", "Kopi").exists(db).await.unwrap());

    assert_eq!(
        Produk::where_eq("kategori", "minuman")
            .delete(db)
            .await
            .unwrap(),
        2
    );
    assert_eq!(Produk::query().count(db).await.unwrap(), 2);
}

#[tokio::test]
async fn invalid_columns_and_operators_are_errors_not_sql() {
    let (kernel, _dir) = kernel().await;
    let db = kernel.db();
    let err = Produk::where_eq("nama; DROP TABLE produk", 1)
        .get(db)
        .await
        .unwrap_err();
    assert!(format!("{err:?}").contains("has no column"));
    let err = Produk::query()
        .where_op("harga", "OR 1=1 --", 1)
        .get(db)
        .await
        .unwrap_err();
    assert!(format!("{err:?}").contains("unsupported operator"));
    assert!(Produk::all(db).await.is_ok(), "table still exists");
}

#[tokio::test]
async fn transactions_roll_back_on_drop() {
    let (kernel, _dir) = kernel().await;
    let db = kernel.db();
    {
        let mut tx = db.begin().await.unwrap();
        produk("Kopi", 1, None).save(&mut *tx).await.unwrap();
        assert_eq!(Produk::query().count(&mut *tx).await.unwrap(), 1);
    }
    assert_eq!(Produk::query().count(db).await.unwrap(), 0);
}

#[tokio::test]
async fn foreign_keys_are_enforced() {
    let (kernel, _dir) = kernel().await;
    let db = kernel.db();
    let result = Catatan::create(
        db,
        Catatan {
            produk_id: Some(42),
            isi: "x".into(),
            ..Default::default()
        },
    )
    .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn paginate_counts_pages() {
    let (kernel, _dir) = kernel().await;
    let db = kernel.db();
    Produk::create_many(db, 23).await.unwrap();
    let page = Produk::query()
        .order_by("id")
        .paginate(db, 3, 10)
        .await
        .unwrap();
    assert_eq!(page.items.len(), 3);
    assert_eq!(
        (page.total, page.last_page, page.from, page.to),
        (23, 3, 21, 23)
    );
    assert!(page.has_prev && !page.has_next);
}

#[tokio::test]
async fn seeders_run_in_order() {
    let (kernel, _dir) = kernel_with(|app| {
        app.seeder(|db| async move {
            Produk::create_many(&db, 3).await?;
            Ok(())
        })
        .seeder(|db| async move {
            let first = Produk::query()
                .order_by("id")
                .first(&db)
                .await?
                .ok_or(Error::NotFound)?;
            Catatan::create(
                &db,
                Catatan {
                    produk_id: Some(first.id),
                    isi: "pertama".into(),
                    ..Default::default()
                },
            )
            .await?;
            Ok(())
        })
    })
    .await;
    kernel.seed().await.unwrap();
    assert_eq!(Produk::query().count(kernel.db()).await.unwrap(), 3);
    assert_eq!(Catatan::query().count(kernel.db()).await.unwrap(), 1);
}

#[test]
fn values_skip_id_and_skipped_fields() {
    let p = produk("Kopi", 5, None);
    assert_eq!(
        <Produk as Model>::COLUMNS,
        &[
            "id",
            "nama",
            "harga",
            "kategori",
            "created_at",
            "updated_at",
            "deleted_at"
        ]
    );
    assert_eq!(
        p.values()[..3],
        [
            DbValue::Text("Kopi".into()),
            DbValue::Integer(5),
            DbValue::Null
        ]
    );
}

struct Shop;

impl Module for Shop {
    fn name(&self) -> &'static str {
        "shop"
    }

    fn routes(&self) -> Routes {
        Routes::new()
            .get("/produk", index)
            .get("/produk/{id}", show)
    }
}

async fn index(State(db): State<Db>, Page(page): Page) -> Result<View> {
    let produk = Produk::query()
        .order_by("id")
        .paginate(&db, page, 2)
        .await?;
    Ok(view("index.html", context! { produk }))
}

async fn show(State(db): State<Db>, Path(id): Path<i64>) -> Result<String> {
    Ok(Produk::find_or_404(&db, id).await?.nama)
}

#[tokio::test]
async fn handlers_use_the_pool_and_render_pagination() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("index.html"),
        r#"{% from "renox/pagination.html" import pagination %}{% for p in produk.items %}[{{ p.nama }}]{% endfor %}{{ pagination(produk) }}"#,
    )
    .unwrap();
    let kernel = App::with_config(config(dir.path()))
        .migrations(renox::migrations!("tests/migrations"))
        .module(Shop)
        .boot()
        .await
        .unwrap();
    kernel.migrate().await.unwrap();
    for nama in ["A", "B", "C", "D", "E"] {
        Produk::create(kernel.db(), produk(nama, 1, None))
            .await
            .unwrap();
    }

    let get = |uri: &str| {
        let router = kernel.router();
        let uri = uri.to_owned();
        async move {
            let res = router
                .oneshot(Request::get(uri).body(Body::empty()).unwrap())
                .await
                .unwrap();
            let status = res.status();
            let body = res.into_body().collect().await.unwrap().to_bytes();
            (status, String::from_utf8(body.to_vec()).unwrap())
        }
    };

    assert_eq!(get("/produk/2").await, (StatusCode::OK, "B".into()));
    assert_eq!(get("/produk/99").await.0, StatusCode::NOT_FOUND);

    let (status, body) = get("/produk?page=2").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.starts_with("[C][D]"), "{body}");
    assert!(
        body.contains(r#"<span aria-current="page">2</span>"#),
        "{body}"
    );
    assert!(body.contains(r#"href="?page=1" rel="prev""#), "{body}");
    assert!(body.contains(r#"href="?page=3" rel="next""#), "{body}");
    assert!(get("/produk?page=abc").await.1.starts_with("[A][B]"));
}
