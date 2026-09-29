use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use renox::Kernel;
use renox::prelude::*;
use serde::Deserialize;
use tower::ServiceExt;

#[derive(Deserialize)]
struct Signup {
    email: String,
}

impl Validate for Signup {
    fn rules(&self, v: &mut Validator) {
        v.field("email", &self.email).required();
    }
}

struct Site;

impl Module for Site {
    fn name(&self) -> &'static str {
        "site"
    }

    fn routes(&self) -> Routes {
        Routes::new()
            .get("/", || async { view("page.html", ()) })
            .name("home")
            .get(
                "/lang/{locale}",
                |session: Session, Path(locale): Path<String>| async move {
                    renox::i18n::set_locale(&session, &locale)?;
                    Ok::<_, Error>("ok")
                },
            )
            .get("/greet", |lang: Lang| async move {
                format!(
                    "{} / {}",
                    lang.t("welcome", &[("name", &"budi")]),
                    lang.choice("items", 2, &[])
                )
            })
            .get("/signup", |Valid(_): Valid<Signup>| async { "ok" })
    }
}

fn write_lang(dir: &std::path::Path) {
    let lang = dir.join("lang");
    std::fs::create_dir_all(&lang).unwrap();
    std::fs::write(
        lang.join("en.json"),
        r#"{ "welcome": "Welcome, :name!", "items": "One item|:count items", "only_en": "fallback works",
             "renox": { "validation": { "attributes": { "email": "email address" } } } }"#,
    )
    .unwrap();
    std::fs::write(
        lang.join("id.json"),
        r#"{ "welcome": "Selamat datang, :Name!", "items": "Satu barang|:count barang",
             "renox": { "validation": { "required": ":Attribute harus ada.", "attributes": { "email": "alamat email" } } } }"#,
    )
    .unwrap();
    std::fs::write(
        lang.join("ms.json"),
        r#"{ "welcome": "Selamat datang ke kedai, :name!",
             "renox": { "auth": { "login_title": "Log masuk" }, "validation": { "required": ":Attribute diperlukan." } } }"#,
    )
    .unwrap();
}

async fn kernel(dir: &std::path::Path, debug: bool) -> Kernel {
    std::fs::create_dir_all(dir.join("views")).unwrap();
    std::fs::write(
        dir.join("views/page.html"),
        "{{ t('welcome', name='arif') }}|{{ t('items', count=1) }}|{{ t('items', count=3) }}|{{ t('only_en') }}|{{ t('missing.key') }}|{{ app.locale }}",
    )
    .unwrap();
    let config = {
        let mut c = Config::default();
        c.env = Environment::Testing;
        c.debug = debug;
        c.key = Some(renox::generate_key());
        c.views_path = dir.join("views");
        c.lang_path = dir.join("lang");
        c
    };
    let kernel = App::with_config(config)
        .module(Auth::new())
        .module(Site)
        .boot()
        .await
        .unwrap();
    kernel.migrate().await.unwrap();
    kernel
}

struct Visitor {
    router: axum::Router,
    cookie: Option<String>,
}

impl Visitor {
    async fn get(&mut self, uri: &str) -> (StatusCode, String) {
        let mut req = Request::get(uri).header("accept", "application/json");
        if let Some(cookie) = &self.cookie {
            req = req.header("cookie", cookie);
        }
        let res = self
            .router
            .clone()
            .oneshot(req.body(Body::empty()).unwrap())
            .await
            .unwrap();
        if let Some(set) = res.headers().get("set-cookie") {
            self.cookie = Some(set.to_str().unwrap().split(';').next().unwrap().to_owned());
        }
        let status = res.status();
        let body = res.into_body().collect().await.unwrap().to_bytes();
        (status, String::from_utf8(body.to_vec()).unwrap())
    }
}

#[tokio::test]
async fn templates_translate_for_each_visitor() {
    let dir = tempfile::tempdir().unwrap();
    write_lang(dir.path());
    let kernel = kernel(dir.path(), false).await;
    let mut visitor = Visitor {
        router: kernel.router(),
        cookie: None,
    };

    assert_eq!(
        visitor.get("/").await.1,
        "Welcome, arif!|One item|3 items|fallback works|missing.key|en"
    );
    visitor.get("/lang/id").await;
    assert_eq!(
        visitor.get("/").await.1,
        "Selamat datang, Arif!|Satu barang|3 barang|fallback works|missing.key|id",
        "missing Indonesian keys fall back to English"
    );
    assert_eq!(
        visitor.get("/greet").await.1,
        "Selamat datang, Budi! / 2 barang"
    );

    visitor.get("/lang/xx").await;
    assert!(
        visitor.get("/").await.1.ends_with("|en"),
        "unknown locales are ignored"
    );

    let mut other = Visitor {
        router: kernel.router(),
        cookie: None,
    };
    assert!(
        other.get("/").await.1.ends_with("|en"),
        "each visitor has their own language"
    );
}

#[tokio::test]
async fn built_in_texts_can_be_translated() {
    let dir = tempfile::tempdir().unwrap();
    write_lang(dir.path());
    let kernel = kernel(dir.path(), false).await;
    let mut visitor = Visitor {
        router: kernel.router(),
        cookie: None,
    };
    let first_error = |body: String| -> String {
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        json["errors"]["email"][0].as_str().unwrap().to_owned()
    };

    assert_eq!(
        first_error(visitor.get("/signup?email=").await.1),
        "The email address field is required."
    );
    visitor.get("/lang/id").await;
    assert_eq!(
        first_error(visitor.get("/signup?email=").await.1),
        "Alamat email harus ada."
    );
    visitor.get("/lang/ms").await;
    assert_eq!(
        first_error(visitor.get("/signup?email=").await.1),
        "Email diperlukan."
    );

    let login = visitor.get("/login").await.1;
    assert!(
        login.contains(">Log masuk</h2>"),
        "renox.auth.* overrides the built-in page"
    );
    assert!(login.contains(r#"<html lang="ms">"#));
    visitor.get("/lang/id").await;
    assert!(
        visitor.get("/login").await.1.contains(">Masuk</h2>"),
        "built-in Indonesian"
    );
}

#[tokio::test]
async fn broken_lang_files_fail_at_boot() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("lang")).unwrap();
    std::fs::write(dir.path().join("lang/id.json"), "{ not json").unwrap();
    let config = {
        let mut c = Config::default();
        c.lang_path = dir.path().join("lang");
        c
    };
    let err = App::with_config(config).boot().await.err().unwrap();
    assert!(
        format!("{err:?}").contains("id.json is not valid JSON"),
        "{err:?}"
    );

    let missing = {
        let mut c = Config::default();
        c.lang_path = dir.path().join("nowhere");
        c
    };
    assert!(
        App::with_config(missing).boot().await.is_ok(),
        "no lang directory is fine"
    );
}

#[tokio::test]
async fn debug_mode_picks_up_edited_lang_files() {
    let dir = tempfile::tempdir().unwrap();
    write_lang(dir.path());
    let kernel = kernel(dir.path(), true).await;
    let mut visitor = Visitor {
        router: kernel.router(),
        cookie: None,
    };
    assert!(visitor.get("/").await.1.starts_with("Welcome, arif!"));

    tokio::time::sleep(Duration::from_millis(1100)).await;
    std::fs::write(
        dir.path().join("lang/en.json"),
        r#"{ "welcome": "Hi :name" }"#,
    )
    .unwrap();
    tokio::time::sleep(Duration::from_millis(1100)).await;
    assert!(visitor.get("/").await.1.starts_with("Hi arif|"));
}
