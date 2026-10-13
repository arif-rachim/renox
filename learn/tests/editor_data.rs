//! The editor's autocomplete data (#529): Renox's public API read from the
//! source (`editor/data/renox-api.json`) and each course stage's models and
//! tags (`editor/data/stage-NN.json`). Both are snapshots: the test compares
//! what it extracts with the committed files, and `RENOX_UPDATE_SNAPSHOT=1`
//! rewrites them. The tags come from `editor/stage-data.sh` (`view:data`).

#[path = "editor_data/api.rs"]
mod api;
#[path = "editor_data/models.rs"]
mod models;
#[path = "editor_data/tidy.rs"]
mod tidy;

use renox::testing::TestApp;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

fn learn_dir() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

static API: LazyLock<api::Api> = LazyLock::new(|| api::extract(learn_dir().parent().unwrap()));

/// The editor's data for one stage.
#[derive(Serialize)]
struct Stage {
    version: u32,
    stage: u8,
    slug: String,
    models: Vec<models::Model>,
    tags: Vec<Tag>,
    global_attributes: Vec<GlobalAttribute>,
}

#[derive(Serialize)]
struct Tag {
    name: String,
    doc: String,
    attributes: Vec<Attribute>,
}

#[derive(Serialize)]
struct Attribute {
    name: String,
    doc: String,
    values: Vec<String>,
}

#[derive(Serialize)]
struct GlobalAttribute {
    name: String,
    doc: String,
}

/// A customData `description`: a string or `{ kind, value }`.
fn description(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Object(o) => o
            .get("value")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_owned(),
        _ => String::new(),
    }
}

fn stage(number: u8) -> Stage {
    let dir = learn_dir().join(format!("course/stage-{number:02}"));
    let tags_file = learn_dir().join(format!("editor/data/tags/stage-{number:02}.json"));
    let text = std::fs::read_to_string(&tags_file).unwrap_or_else(|e| {
        panic!(
            "{}: {e} (run learn/editor/stage-data.sh)",
            tags_file.display()
        )
    });
    let custom: serde_json::Value = serde_json::from_str(&text).unwrap();
    let list = |v: &serde_json::Value| v.as_array().cloned().unwrap_or_default();
    let tags = list(&custom["tags"])
        .iter()
        .map(|t| Tag {
            name: t["name"].as_str().unwrap_or("").to_owned(),
            doc: description(&t["description"]),
            attributes: list(&t["attributes"])
                .iter()
                .map(|a| Attribute {
                    name: a["name"].as_str().unwrap_or("").to_owned(),
                    doc: description(&a["description"]),
                    values: list(&a["values"])
                        .iter()
                        .filter_map(|v| v["name"].as_str().map(str::to_owned))
                        .collect(),
                })
                .collect(),
        })
        .collect();
    let global_attributes = list(&custom["globalAttributes"])
        .iter()
        .map(|a| GlobalAttribute {
            name: a["name"].as_str().unwrap_or("").to_owned(),
            doc: description(&a["description"]),
        })
        .collect();
    Stage {
        version: 1,
        stage: number,
        slug: renox_learn::course::STAGES[usize::from(number)]
            .slug
            .to_owned(),
        models: models::models(&dir),
        tags,
        global_attributes,
    }
}

/// Compares `value`'s pretty JSON with the committed `file`, or rewrites it
/// with `RENOX_UPDATE_SNAPSHOT=1`.
fn snapshot<T: Serialize>(file: &str, value: &T) {
    let path: PathBuf = learn_dir().join("editor/data").join(file);
    let mut text = serde_json::to_string_pretty(value).unwrap();
    text.push('\n');
    if std::env::var("RENOX_UPDATE_SNAPSHOT").is_ok_and(|v| v == "1") {
        std::fs::write(&path, &text).unwrap();
        return;
    }
    // A Windows checkout may have CRLF line ends.
    let committed = std::fs::read_to_string(&path)
        .unwrap_or_default()
        .replace("\r\n", "\n");
    assert!(
        committed == text,
        "learn/editor/data/{file} is out of date: run \
         `RENOX_UPDATE_SNAPSHOT=1 cargo test -p renox-learn --test editor_data` and commit it"
    );
}

fn item<'a>(path: &str, kind: &str) -> &'a api::ApiItem {
    API.items
        .iter()
        .find(|i| i.path == path && i.kind == kind)
        .unwrap_or_else(|| panic!("no {kind} {path}"))
}

#[test]
fn the_api_matches_the_snapshot() {
    snapshot("renox-api.json", &*API);
    // 420 items and 1 200 members when this was written (#529 guessed 500
    // items; the members are where the bulk is).
    let members: usize = API.items.iter().map(|i| i.members.len()).sum();
    assert!(
        API.items.len() > 400 && members > 1000,
        "only {} items and {members} members in Renox's API",
        API.items.len()
    );
    // A setting enum written by setting_enum!, with its variants.
    let driver = item("renox::SessionDriver", "enum");
    assert!(
        driver
            .members
            .iter()
            .any(|m| m.name == "Database" && m.kind == "variant")
    );
}

#[test]
fn every_stage_matches_its_snapshot() {
    for s in renox_learn::course::STAGES {
        snapshot(&format!("stage-{:02}.json", s.number), &stage(s.number));
    }
}

#[test]
fn reexports_reach_the_facade_and_the_prelude() {
    // From the private `routing` module, through `pub use` in lib.rs.
    let routes = item("renox::Routes", "struct");
    assert!(routes.prelude);
    let model = item("renox::db::Model", "trait");
    assert!(model.prelude);
    assert!(model.members.iter().any(|m| m.name == "find_or_404"));
    let state = item("renox::prelude::State", "reexport");
    assert_eq!(state.from.as_deref(), Some("axum::extract::State"));
    assert!(state.prelude);
    // The derive from renox-macros, with its doc.
    let derive = item("renox::Model", "derive");
    assert!(derive.prelude && !derive.doc.is_empty());
    // A module's own path, for `renox::db::` completions.
    item("renox::db", "mod");
}

#[test]
fn macro_generated_methods_are_listed() {
    let routes = item("renox::Routes", "struct");
    for name in ["get", "post", "put", "patch", "delete"] {
        let m = routes
            .members
            .iter()
            .find(|m| m.name == name)
            .unwrap_or_else(|| panic!("Routes::{name}"));
        assert_eq!(m.kind, "method");
        assert_eq!(
            m.sig,
            format!("pub fn {name}<H, T>(self, path: &str, handler: H) -> Self")
        );
        assert!(m.doc.starts_with("Adds a route for this HTTP method"));
        assert_eq!(m.returns.as_deref(), Some("Routes"));
    }
    let col = API
        .items
        .iter()
        .find(|i| i.name == "Col" && i.kind == "struct")
        .expect("Col");
    let lt = col
        .members
        .iter()
        .find(|m| m.name == "lt")
        .expect("Col::lt");
    assert_eq!(lt.doc, "`column < value`.");
    assert!(col.members.iter().any(|m| m.name == "eq"));
}

#[test]
fn hidden_and_feature_gated_items_are_left_out() {
    let model = item("renox::db::Model", "trait");
    assert!(
        !model.members.iter().any(|m| m.name == "set_id"),
        "Model::set_id is #[doc(hidden)]"
    );
    assert!(
        model.members.iter().any(|m| m.name == "id"),
        "Model::id is documented"
    );
    for i in &API.items {
        assert!(
            !i.name.starts_with("__") && i.name != "bounds",
            "{} starts with __",
            i.path
        );
        assert!(
            !i.path.to_lowercase().contains("postgres"),
            "{} is behind the postgres feature",
            i.path
        );
    }
}

#[test]
fn tidy_formats_signatures() {
    use tidy::tidy;
    assert_eq!(tidy("pub fn new () -> Self"), "pub fn new() -> Self");
    assert_eq!(
        tidy("pub fn get < H , T > (self , path : & str , handler : H ,) -> Self"),
        "pub fn get<H, T>(self, path: &str, handler: H) -> Self"
    );
    assert_eq!(
        tidy(
            "fn find_or_404 < 'c , E : Executor < 'c > > (db : E , id : i64) -> impl Future < Output = Result < Self > > + Send"
        ),
        "fn find_or_404<'c, E: Executor<'c>>(db: E, id: i64) -> impl Future<Output = Result<Self>> + Send"
    );
    assert_eq!(
        tidy("const COLUMNS : & 'static [& 'static str]"),
        "const COLUMNS: &'static [&'static str]"
    );
}

#[test]
fn stage_02_has_menu_item_with_typed_columns() {
    let s = stage(2);
    assert_eq!(s.slug, renox_learn::course::STAGES[2].slug);
    let item = s
        .models
        .iter()
        .find(|m| m.name == "MenuItem")
        .expect("MenuItem in stage 2");
    assert_eq!(item.table, "menu_items");
    assert_eq!(item.key, "i64");
    let price = item
        .columns
        .iter()
        .find(|c| c.konst == "PRICE")
        .expect("PRICE");
    assert_eq!((price.column.as_str(), price.ty.as_str()), ("price", "i64"));
    assert!(s.tags.iter().any(|t| t.name == "rx-table"));
    assert!(stage(0).models.iter().all(|m| m.name != "MenuItem"));
}

#[renox::test]
async fn the_data_routes_serve_json_and_404_otherwise() {
    let app = TestApp::new(renox_learn::app()).await;
    let api = app.get("/editor/data/renox-api.json").await;
    api.assert_ok();
    let text = api.text();
    assert!(!text.contains('\n'), "served minified");
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(value["version"], 1);
    let stage = app.get("/editor/data/stage-02.json").await;
    stage.assert_ok();
    let value: serde_json::Value = serde_json::from_str(&stage.text()).unwrap();
    assert_eq!(value["stage"], 2);
    for missing in [
        "/editor/data/stage-99.json",
        "/editor/data/tags.json",
        "/editor/data/renox-api",
    ] {
        app.get(missing).await.assert_status(404);
    }
}
