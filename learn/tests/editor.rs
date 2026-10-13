//! The code editor (#528): the `/editor` playground, the vendored CodeMirror
//! bundle (one file, within its budget, made by `editor/bundle.sh`) and its
//! NOTICE (every locked package, version and licence).

use renox::testing::TestApp;
use std::path::Path;

async fn site() -> TestApp {
    TestApp::with_config(renox_learn::app(), |c| {
        c.url = "https://learn.renox.rs".into()
    })
    .await
}

fn learn_dir() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// The bundle's size budget (the owner, 2026-10-11).
const BUDGET: u64 = 900_000;

#[renox::test]
async fn the_playground_answers_with_both_editors_and_is_not_indexed() {
    let app = site().await;
    let page = app.get("/editor").await;
    page.assert_ok()
        .assert_see("<meta name=\"robots\" content=\"noindex\">")
        .assert_see("data-learn-editor=\"rust\"")
        .assert_see("data-learn-editor=\"html\"")
        .assert_see("they are not type inference")
        // The samples are printed as text, not run as templates.
        .assert_see("{{ item.name }}")
        .assert_see("pub struct MenuItem");
    let html = page.text();
    assert_eq!(html.matches("data-learn-editor=").count(), 2);
    for script in [renox_learn::editor::CODEMIRROR, "editor.js"] {
        assert!(
            html.contains(&format!("src=\"/{script}?v=")),
            "{script} is loaded with ?v="
        );
    }
}

#[test]
fn exactly_one_codemirror_bundle_is_vendored_and_within_budget() {
    let vendor = learn_dir().join("public/vendor");
    let bundles: Vec<_> = std::fs::read_dir(&vendor)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("codemirror-") && n.ends_with(".js"))
        .collect();
    assert_eq!(
        bundles,
        [renox_learn::editor::CODEMIRROR.trim_start_matches("vendor/")],
        "one bundle, the one editor::CODEMIRROR names (run learn/editor/bundle.sh)"
    );
    let path = learn_dir()
        .join("public")
        .join(renox_learn::editor::CODEMIRROR);
    let size = std::fs::metadata(&path).unwrap().len();
    assert!(size <= BUDGET, "the bundle is {size} bytes, over {BUDGET}");
    let text = std::fs::read_to_string(&path).unwrap();
    assert_eq!(
        text.lines().next(),
        Some("/* CodeMirror 6 for learn.renox.rs. MIT licence; see vendor/NOTICE. */")
    );
    // The bundle is named after @codemirror/view's locked version.
    let lock: serde_json::Value =
        serde_json::from_str(include_str!("../editor/package-lock.json")).unwrap();
    let view = lock["packages"]["node_modules/@codemirror/view"]["version"]
        .as_str()
        .unwrap();
    assert_eq!(
        renox_learn::editor::CODEMIRROR,
        format!("vendor/codemirror-{view}.js")
    );
}

#[test]
fn the_notice_names_every_locked_package_and_version() {
    let lock: serde_json::Value =
        serde_json::from_str(include_str!("../editor/package-lock.json")).unwrap();
    // A Windows checkout may have CRLF line ends.
    let notice = include_str!("../public/vendor/NOTICE").replace("\r\n", "\n");
    let mut checked = 0;
    for (path, entry) in lock["packages"].as_object().unwrap() {
        if path.is_empty() || entry["dev"].as_bool() == Some(true) {
            continue;
        }
        let name = path.trim_start_matches("node_modules/");
        let version = entry["version"].as_str().unwrap();
        assert!(
            notice.contains(&format!("{name} {version}\n")),
            "NOTICE doesn't name {name} {version}"
        );
        assert_eq!(entry["license"].as_str(), Some("MIT"), "{name}'s licence");
        let licence = format!(
            "public/vendor/licenses/{}-LICENSE",
            name.trim_start_matches('@').replace('/', "-")
        );
        assert!(learn_dir().join(&licence).is_file(), "{licence} is missing");
        checked += 1;
    }
    assert!(checked >= 10, "checked {checked} packages");
    let package: serde_json::Value =
        serde_json::from_str(include_str!("../editor/package.json")).unwrap();
    for group in ["dependencies", "devDependencies"] {
        for (name, version) in package[group].as_object().unwrap() {
            let version = version.as_str().unwrap();
            assert!(
                version.chars().all(|c| c.is_ascii_digit() || c == '.'),
                "{name} is not pinned exactly: {version}"
            );
        }
    }
}

#[renox::test]
async fn the_sitemap_leaves_the_playground_out() {
    let app = site().await;
    for path in ["/sitemap.xml", "/llms.txt"] {
        let text = app.get(path).await.text();
        assert!(!text.contains("/editor"), "{path} mentions /editor");
    }
}
