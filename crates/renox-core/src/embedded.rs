//! Views, translations and public files compiled into the binary with
//! `renox::embedded!()`, so a release build is a single file to deploy.

use std::collections::HashMap;
use std::sync::Arc;

use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use axum::http::{StatusCode, Uri};
use axum::response::{IntoResponse, Response};

use crate::Error;

/// Files from `resources/views`, `resources/lang` and `public`, as
/// `(relative path, contents)`. Built by `renox::embedded!()`.
#[derive(Debug, Clone, Copy, Default)]
pub struct Embedded {
    pub views: &'static [(&'static str, &'static str)],
    pub lang: &'static [(&'static str, &'static str)],
    pub public: &'static [(&'static str, &'static [u8])],
}

/// A content type for a public file, from its extension.
pub(crate) fn content_type(path: &str) -> &'static str {
    let ext = path.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase());
    match ext.as_deref() {
        Some("css") => "text/css; charset=utf-8",
        Some("js" | "mjs") => "text/javascript; charset=utf-8",
        Some("html" | "htm") => "text/html; charset=utf-8",
        Some("json" | "map") => "application/json",
        Some("txt") => "text/plain; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("ico") => "image/x-icon",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("pdf") => "application/pdf",
        Some("xml") => "application/xml",
        _ => "application/octet-stream",
    }
}

pub(crate) type PublicFiles = Arc<HashMap<&'static str, &'static [u8]>>;

pub(crate) fn public_map(files: &'static [(&'static str, &'static [u8])]) -> PublicFiles {
    Arc::new(files.iter().copied().collect())
}

/// Serves an embedded public file for the request path, or 404.
pub(crate) fn serve(files: &PublicFiles, uri: &Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };
    match files.get(path) {
        Some(bytes) => (
            StatusCode::OK,
            [
                (CONTENT_TYPE, content_type(path)),
                (CACHE_CONTROL, "public, max-age=3600"),
            ],
            *bytes,
        )
            .into_response(),
        None => Error::NotFound.into_response(),
    }
}
