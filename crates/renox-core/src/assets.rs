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
const RENOX: &str = r#"(function () {
  // Send the CSRF token with every HTMX request.
  document.addEventListener("htmx:configRequest", function (event) {
    var meta = document.querySelector('meta[name="csrf-token"]');
    if (meta) event.detail.headers["X-CSRF-Token"] = meta.content;
  });

  function formOf(elt) {
    return (elt && elt.closest && elt.closest("form")) || elt;
  }

  function clearErrors(form) {
    if (!form || !form.querySelectorAll) return;
    form.querySelectorAll("[data-renox-error]").forEach(function (el) { el.remove(); });
    form.querySelectorAll("[data-error-for]").forEach(function (el) { el.textContent = ""; });
    form.querySelectorAll("[aria-invalid]").forEach(function (el) { el.removeAttribute("aria-invalid"); });
  }

  // Show 422 validation errors next to the inputs that caused them: in an
  // element with data-error-for="field" if the form has one, otherwise in a
  // <p class="error"> inserted after the input.
  function showErrors(form, errors) {
    clearErrors(form);
    Object.keys(errors).forEach(function (field) {
      var message = (errors[field] || [])[0];
      if (!message) return;
      var name = CSS.escape(field);
      var input = form.querySelector('[name="' + name + '"]');
      if (input) input.setAttribute("aria-invalid", "true");
      var slot = form.querySelector('[data-error-for="' + name + '"]');
      if (slot) {
        slot.textContent = message;
        return;
      }
      var p = document.createElement("p");
      p.className = "error";
      p.setAttribute("data-renox-error", field);
      p.textContent = message;
      if (input) input.insertAdjacentElement("afterend", p);
      else form.prepend(p);
    });
    // First invalid input in page order, not in the (alphabetical) error order.
    var first = form.querySelector('[aria-invalid="true"]');
    if (first && first.focus) first.focus();
  }

  document.addEventListener("htmx:beforeRequest", function (event) {
    clearErrors(formOf(event.detail.elt));
  });

  document.addEventListener("htmx:beforeSwap", function (event) {
    var xhr = event.detail.xhr;
    var type = xhr.getResponseHeader("Content-Type") || "";
    if (xhr.status !== 422 || type.indexOf("application/json") !== 0) return;
    // Not swapped, and still an error: forms that reset themselves after a
    // successful request keep what the user typed.
    event.detail.shouldSwap = false;
    try {
      var body = JSON.parse(xhr.responseText);
      showErrors(formOf(event.detail.requestConfig.elt), body.errors || {});
    } catch (_) {}
  });
})();
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
