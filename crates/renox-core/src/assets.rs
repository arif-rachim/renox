use std::sync::LazyLock;

use axum::Router;
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use axum::response::IntoResponse;
use axum::routing::get;

use crate::AppState;

pub const HTMX_VERSION: &str = "2.0.11";
pub const ALPINE_VERSION: &str = "3.17.4";

const HTMX: &str = include_str!("../assets/htmx.min.js");
const ALPINE: &str = include_str!("../assets/alpine.min.js");
const RENOX: &str = r#"document.addEventListener("htmx:configRequest", function (event) {
  var meta = document.querySelector('meta[name="csrf-token"]');
  if (meta) event.detail.headers["X-CSRF-Token"] = meta.content;
});
"#;

/// Content-hashed URLs, so browsers can cache the files forever.
static URLS: LazyLock<[String; 3]> = LazyLock::new(|| {
    [
        format!("/_renox/htmx-{HTMX_VERSION}.min.js"),
        format!("/_renox/alpine-{ALPINE_VERSION}.min.js"),
        format!("/_renox/renox-{:016x}.js", fnv1a(RENOX)),
    ]
});

pub(crate) fn router() -> Router<AppState> {
    let [htmx, alpine, renox] = &*URLS;
    Router::new()
        .route(htmx, get(|| async { js(HTMX) }))
        .route(alpine, get(|| async { js(ALPINE) }))
        .route(renox, get(|| async { js(RENOX) }))
}

fn js(body: &'static str) -> impl IntoResponse {
    (
        [
            (CONTENT_TYPE, "text/javascript; charset=utf-8"),
            (CACHE_CONTROL, "public, max-age=31536000, immutable"),
        ],
        body,
    )
}

/// The `<head>` tags every Renox page needs: the CSRF token, htmx, Alpine.js
/// and the script that sends the token with HTMX requests.
pub(crate) fn head_tags(csrf_token: &str) -> String {
    let [htmx, alpine, renox] = &*URLS;
    format!(
        "<meta name=\"csrf-token\" content=\"{csrf_token}\">\n\
         <script src=\"{htmx}\" defer></script>\n\
         <script src=\"{renox}\" defer></script>\n\
         <script src=\"{alpine}\" defer></script>"
    )
}

fn fnv1a(s: &str) -> u64 {
    s.bytes().fold(0xcbf29ce484222325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
    })
}
