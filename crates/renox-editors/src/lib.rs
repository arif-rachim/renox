//! Rich text, Markdown and code editor fields, and a code entry for detail
//! pages, for Renox apps. They bring JavaScript the UI kit doesn't carry
//! (Trix, CodeJar and Prism, compiled into this crate), loaded only on pages
//! that use them.
//!
//! ```
//! use renox::prelude::*;
//! use renox_editors::Editors;
//!
//! # let _ =
//! App::new().module(Editors::new())
//! # ;
//! ```
//!
//! In a form, next to the kit's fields:
//!
//! ```html
//! {% from "renox-editors/editors.html" import rich_editor, markdown_editor, code_editor %}
//! <form method="post" action="/posts" data-live-validate>
//!   {{ csrf_field() }}
//!   {{ rich_editor("body", "Body", value=post.body, required=true) }}
//!   {{ markdown_editor("notes", "Notes", value=post.notes) }}
//!   {{ code_editor("settings", "Settings", value=post.settings, language="json") }}
//! </form>
//! ```
//!
//! Each sends a plain form field of its name, so `Valid<T>`, old input,
//! errors and live validation work as for the kit's own fields. Rich text
//! arrives as HTML: read it as a [`RichText`], which cleans it, and show it
//! with the `rich_text` filter, which cleans it again. On a detail page,
//! `code_entry` shows code highlighted, with a copy button. The guide is
//! docs/editors.md in the Renox repository.

use renox::minijinja;
use renox::prelude::*;
use serde::Deserialize;

mod assets;
pub mod rich_text;

pub use assets::{CODEJAR_VERSION, PRISM_VERSION, TRIX_VERSION};
pub use rich_text::{RichText, sanitize};

/// The macros and the preview, compiled in. An app replaces one with a file
/// of the same name under its views directory
/// (`resources/views/renox-editors/editors.html`).
const VIEWS: &[(&str, &str)] = &[
    (
        "renox-editors/editors.html",
        include_str!("../views/editors.html"),
    ),
    (
        "renox-editors/preview.html",
        include_str!("../views/preview.html"),
    ),
];

/// The longest Markdown the preview renders, in bytes.
const PREVIEW_LIMIT: usize = 200_000;

/// The editors module: add it to the app, then import the macros from
/// `renox-editors/editors.html`.
#[derive(Debug, Clone, Default)]
pub struct Editors {}

impl Editors {
    /// The module with its defaults.
    pub fn new() -> Self {
        Self::default()
    }
}

impl Module for Editors {
    fn name(&self) -> &'static str {
        "editors"
    }

    fn routes(&self) -> Routes {
        // The Markdown editor's Preview tab: the text rendered as the
        // `markdown` filter renders it on the page.
        Routes::new()
            .post("/_renox/editors/preview", preview)
            .name("editors.preview")
            .throttle(120, std::time::Duration::from_secs(60))
    }

    fn register(&self, app: &mut Registry) {
        assets::register(app);
        app.templates(|env| {
            for (name, source) in VIEWS {
                // The app's own file of that name wins.
                if env.get_template(name).is_err() {
                    let _ = env.add_template(name, source);
                }
            }
            env.add_function("renox_editors", || {
                minijinja::Value::from_safe_string(assets::tags())
            });
            // `{{ post.body | rich_text }}`: stored rich text, cleaned again
            // and shown as HTML.
            env.add_filter("rich_text", |html: Option<String>| {
                minijinja::Value::from_safe_string(sanitize(html.as_deref().unwrap_or("")))
            });
            // The rich editor's starting value: cleaned, and escaped by the
            // template as an attribute.
            env.add_function("renox_editors_html", |html: Option<String>| {
                sanitize(html.as_deref().unwrap_or(""))
            });
            // A code entry's text: strings as they are, other values as
            // indented JSON.
            env.add_function("renox_editors_code", code_text);
        });
    }
}

/// What the Preview tab sends.
#[derive(Deserialize)]
struct Preview {
    #[serde(default)]
    text: String,
}

async fn preview(Form(form): Form<Preview>) -> Result<Response> {
    if form.text.len() > PREVIEW_LIMIT {
        return Ok(StatusCode::PAYLOAD_TOO_LARGE.into_response());
    }
    Ok(view("renox-editors/preview.html", context! { text => form.text }).into_response())
}

/// The text a code entry shows for `value`.
fn code_text(value: minijinja::Value) -> String {
    if value.is_undefined() || value.is_none() {
        return String::new();
    }
    if let Some(text) = value.as_str() {
        return text.to_owned();
    }
    renox::serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string())
}

/// Compiles the Rust in docs/editors.md (the guide) as doctests.
#[cfg(doctest)]
#[doc = include_str!("../../../docs/editors.md")]
pub struct Guide;
