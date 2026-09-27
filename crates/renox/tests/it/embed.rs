//! Views, translations and public files compiled into the binary.

use renox::Embedded;
use renox::prelude::*;
use renox::testing::TestApp;

struct Site;

impl Module for Site {
    fn name(&self) -> &'static str {
        "site"
    }

    fn routes(&self) -> Routes {
        Routes::new()
            .get("/", || async { view("home.html", ()) })
            .name("home")
    }
}

const EMBEDDED: Embedded = Embedded {
    views: &[("home.html", "embedded: {{ t('hi') }}")],
    lang: &[("en.json", r#"{ "hi": "hello from the binary" }"#)],
    public: &[
        ("app.css", b"body { color: teal; }"),
        ("img/logo.svg", b"<svg/>"),
    ],
};

async fn app(debug: bool, dir: &std::path::Path) -> TestApp {
    std::fs::create_dir_all(dir.join("views")).unwrap();
    std::fs::write(dir.join("views/home.html"), "from disk").unwrap();
    TestApp::with_config(App::new().embed(EMBEDDED).module(Site), |c| {
        c.debug = debug;
        c.views_path = dir.join("views");
        c.public_path = dir.join("public");
        c.lang_path = dir.join("lang");
    })
    .await
}

#[renox::test]
async fn release_builds_serve_what_was_compiled_in() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(false, dir.path()).await;
    app.get("/")
        .await
        .assert_ok()
        .assert_see("embedded: hello from the binary");
    app.get("/app.css")
        .await
        .assert_ok()
        .assert_header("content-type", "text/css; charset=utf-8")
        .assert_header("cache-control", "public, max-age=3600")
        .assert_see("teal");
    app.get("/img/logo.svg")
        .await
        .assert_header("content-type", "image/svg+xml");
    app.get("/missing.css").await.assert_not_found();
    app.get("/login-page-that-does-not-exist")
        .await
        .assert_not_found();
}

#[renox::test]
async fn debug_builds_read_the_disk() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(true, dir.path()).await;
    app.get("/").await.assert_ok().assert_see("from disk");
    app.get("/app.css").await.assert_not_found();
}

#[test]
fn the_macro_embeds_nothing_when_there_is_nothing() {
    // crates/renox has no resources/ or public/ directory.
    let embedded = renox::embedded!();
    assert!(embedded.views.is_empty() && embedded.lang.is_empty() && embedded.public.is_empty());
}
