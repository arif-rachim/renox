//! The code editor (#528): CodeMirror 6, vendored as one pinned bundle made by
//! `editor/bundle.sh`, plus `public/editor.js` (`window.LearnEditor`). `GET
//! /editor` is its playground (noindex, in no sitemap), one Rust editor and one
//! HTML editor with coffee-shop samples.

use renox::prelude::*;

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
