//! The code editor (#528): CodeMirror 6, vendored as one pinned bundle made by
//! `editor/bundle.sh`, plus `public/editor.js` (`window.LearnEditor`). `GET
//! /editor` is its playground (noindex, in no sitemap), one Rust editor and one
//! HTML editor with coffee-shop samples.
//!
//! The autocomplete's data (#529) is served by `GET /editor/data/{file}`:
//! `renox-api.json` (Renox's public API) and `stage-NN.json` (a stage's
//! models and tags), written by `tests/editor_data.rs` and compiled in.

use renox::axum::http::header::CONTENT_TYPE;
use renox::axum::response::{IntoResponse, Response};
use renox::prelude::*;
use std::collections::HashMap;
use std::sync::LazyLock;

include!(concat!(env!("OUT_DIR"), "/editor_data.rs"));

/// The data files, minified once.
static MINIFIED: LazyLock<HashMap<&'static str, String>> = LazyLock::new(|| {
    EDITOR_DATA
        .iter()
        .map(|(name, text)| {
            let value: serde_json::Value = serde_json::from_str(text)
                .unwrap_or_else(|e| panic!("editor/data/{name}.json: {e}"));
            (*name, value.to_string())
        })
        .collect()
});

/// `GET /editor/data/{file}`: `renox-api.json` or `stage-NN.json`, minified.
pub async fn data(Path(file): Path<String>) -> Result<Response> {
    let name = file.strip_suffix(".json").ok_or(Error::NotFound)?;
    let text = MINIFIED.get(name).ok_or(Error::NotFound)?;
    Ok(([(CONTENT_TYPE, "application/json")], text.clone()).into_response())
}

/// The vendored bundle, under `public/`. `editor/bundle.sh` names it after
/// `@codemirror/view`'s version; a version bump changes this line too
/// (`tests/editor.rs` checks the file is there).
pub const CODEMIRROR: &str = "vendor/codemirror-6.43.14.js";

/// The playground's Rust sample: a `MenuItem` model.
pub const RUST_SAMPLE: &str = include_str!("../editor/samples/menu_item.rs");
/// The playground's HTML sample: the menu page, written with tags.
pub const HTML_SAMPLE: &str = include_str!("../editor/samples/index.html");

/// `GET /editor`.
pub async fn page() -> Result<View> {
    Ok(view(
        "editor.html",
        context! {
            codemirror => CODEMIRROR,
            rust_sample => RUST_SAMPLE,
            html_sample => HTML_SAMPLE,
        },
    ))
}
