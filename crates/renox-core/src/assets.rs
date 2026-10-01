use std::sync::LazyLock;

use axum::Router;
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use axum::response::IntoResponse;
use axum::routing::get;

use crate::AppState;

/// The version of the bundled htmx.
pub const HTMX_VERSION: &str = "2.0.11";
/// The version of the bundled Alpine.js.
pub const ALPINE_VERSION: &str = "3.17.4";

const HTMX: &str = include_str!("../assets/htmx.min.js");
const ALPINE: &str = include_str!("../assets/alpine.min.js");
/// Alpine's build for `CSP=strict`: no `eval`, simpler expressions.
const ALPINE_CSP: &str = include_str!("../assets/alpine-csp.min.js");
/// The UI kit's styles and behavior (renox/ui.html).
pub(crate) const UI_CSS: &str = include_str!("../assets/renox-ui.css");
pub(crate) const UI_JS: &str = include_str!("../assets/renox-ui.js");

/// The data grid's styles and behavior (renox/grid.html), and the date
/// range calendar its date filters use.
pub(crate) const GRID_CSS: &str = include_str!("../assets/renox-grid.css");
pub(crate) const GRID_JS: &str = include_str!("../assets/renox-grid.js");
/// The version of the bundled Cally (calendar web components, MIT).
pub const CALLY_VERSION: &str = "0.9.2";
const CALLY: &str = include_str!("../assets/cally.js");

static GRID_URLS: LazyLock<[String; 3]> = LazyLock::new(|| {
    [
        format!("/_renox/grid-{:016x}.css", fnv1a(GRID_CSS)),
        format!("/_renox/grid-{:016x}.js", fnv1a(GRID_JS)),
        format!("/_renox/cally-{:016x}.js", fnv1a(CALLY)),
    ]
});

/// `{{ renox_grid() }}` (the `grid` macro calls it): the grid's stylesheet
/// and scripts. Twice on a page is harmless.
pub(crate) fn grid_tags() -> String {
    let [css, js, cally] = &*GRID_URLS;
    format!(
        "<link rel=\"stylesheet\" href=\"{css}\">\n<script src=\"{js}\" defer></script>\n<script type=\"module\" src=\"{cally}\"></script>"
    )
}

static UI_URLS: LazyLock<[String; 2]> = LazyLock::new(|| {
    [
        format!("/_renox/ui-{:016x}.css", fnv1a(UI_CSS)),
        format!("/_renox/ui-{:016x}.js", fnv1a(UI_JS)),
    ]
});

/// `{{ renox_ui() }}`: the kit's stylesheet and script, for the `<head>`;
/// `renox_ui(styles=false)` only the script, for an app with its own copy
/// of the styles (`ui:publish`).
pub(crate) fn ui_tags(styles: bool) -> String {
    let [css, js] = &*UI_URLS;
    let script = format!("<script src=\"{js}\" defer></script>");
    if styles {
        format!("<link rel=\"stylesheet\" href=\"{css}\">\n{script}")
    } else {
        script
    }
}

/// `my-app ui:publish`: copies the kit into the app, to change it there.
pub(crate) fn publish_ui(config: &crate::Config, force: bool) -> anyhow::Result<()> {
    let view = config.views_path.join("components/ui.html");
    let css = config.public_path.join("css/renox-ui.css");
    for path in [&view, &css] {
        if path.exists() && !force {
            anyhow::bail!("{} exists; add --force to replace it", path.display());
        }
    }
    let template = include_str!("../views/ui.html").replace(
        "{% from \"renox/ui.html\" import",
        "{% from \"components/ui.html\" import",
    );
    for (path, body) in [(&view, template.as_str()), (&css, UI_CSS)] {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, body)?;
        println!("Wrote {}", path.display());
    }
    println!(
        "Import from \"components/ui.html\", and in the layout replace {{{{ renox_ui() }}}} with\n  \
         <link rel=\"stylesheet\" href=\"{{{{ asset('css/renox-ui.css') }}}}\">{{{{ renox_ui(styles=false) }}}}"
    );
    Ok(())
}

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
      var slot = form.querySelector('[data-error-for="' + name + '"]');
      // `tags.1` / `photos.0`: an item of a list. Use the list's own input
      // (the item's one when there is one per item) and slot.
      var parts = field.split(".");
      if (!input && parts.length > 1) {
        var inputs = form.querySelectorAll('[name="' + CSS.escape(parts[0]) + '"]');
        input = inputs[parseInt(parts[1], 10)] || inputs[0] || null;
      }
      if (!slot && parts.length > 1) {
        slot = form.querySelector('[data-error-for="' + CSS.escape(parts[0]) + '"]');
      }
      if (input) input.setAttribute("aria-invalid", "true");
      if (slot) {
        slot.textContent = slot.textContent ? slot.textContent + " " + message : message;
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

  // Live reload while developing: reload when a view, public or lang file
  // changes, or when the app restarts (a new boot id after reconnecting).
  // Analytics events from the server (renox::analytics::event): with an htmx
  // swap in its HX-Trigger, with a page in a <meta>.
  function track(events) {
    (events || []).forEach(function (e) {
      var params = e.params || {};
      if (typeof window.gtag === "function") window.gtag("event", e.name, params);
      if (window.dataLayer && document.querySelector('script[src*="googletagmanager.com/gtm.js"]')) {
        var entry = { event: e.name };
        Object.keys(params).forEach(function (key) { entry[key] = params[key]; });
        window.dataLayer.push(entry);
      }
      document.dispatchEvent(new CustomEvent("renox:tracked", { detail: e }));
    });
  }
  document.addEventListener("renox:analytics", function (event) {
    track(event.detail && event.detail.events);
  });
  var pending = document.querySelector('meta[name="renox-analytics"]');
  if (pending) {
    try { track(JSON.parse(pending.content)); } catch (_) {}
  }

  var live = document.querySelector('meta[name="renox-live"]');
  if (live && window.EventSource) {
    var boot = null;
    var source = new EventSource("/_renox/live");
    source.addEventListener("boot", function (event) {
      if (boot !== null && boot !== event.data) location.reload();
      boot = event.data;
    });
    source.addEventListener("reload", function () { location.reload(); });
  }
})();
"#;

/// Content-hashed URLs, so browsers can cache the files forever.
static URLS: LazyLock<[String; 4]> = LazyLock::new(|| {
    [
        format!("/_renox/htmx-{HTMX_VERSION}.min.js"),
        format!("/_renox/alpine-{ALPINE_VERSION}.min.js"),
        format!("/_renox/renox-{:016x}.js", fnv1a(RENOX)),
        format!("/_renox/alpine-csp-{ALPINE_VERSION}.min.js"),
    ]
});

pub(crate) fn router() -> Router<AppState> {
    let [htmx, alpine, renox, alpine_csp] = &*URLS;
    let [ui_css, ui_js] = &*UI_URLS;
    Router::new()
        .route(
            ui_css,
            get(|| async { asset("text/css; charset=utf-8", UI_CSS) }),
        )
        .route(ui_js, get(|| async { js(UI_JS) }))
        .route(
            &GRID_URLS[0],
            get(|| async { asset("text/css; charset=utf-8", GRID_CSS) }),
        )
        .route(&GRID_URLS[1], get(|| async { js(GRID_JS) }))
        .route(&GRID_URLS[2], get(|| async { js(CALLY) }))
        .route(htmx, get(|| async { js(HTMX) }))
        .route(alpine, get(|| async { js(ALPINE) }))
        .route(renox, get(|| async { js(RENOX) }))
        .route(alpine_csp, get(|| async { js(ALPINE_CSP) }))
}

fn asset(content_type: &'static str, body: &'static str) -> impl IntoResponse {
    (
        [
            (CONTENT_TYPE, content_type),
            (CACHE_CONTROL, "public, max-age=31536000, immutable"),
        ],
        body,
    )
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
pub(crate) fn head_tags(csrf_token: &str, live: bool, strict_csp: bool) -> String {
    let [htmx, alpine, renox, alpine_csp] = &*URLS;
    // Under a strict CSP: Alpine's CSP build, and htmx without `eval`.
    let (alpine, htmx_config) = if strict_csp {
        (
            alpine_csp,
            "<meta name=\"htmx-config\" content='{\"allowEval\":false}'>\n",
        )
    } else {
        (alpine, "")
    };
    let live = if live {
        "<meta name=\"renox-live\" content=\"1\">\n"
    } else {
        ""
    };
    format!(
        "{live}{htmx_config}<meta name=\"csrf-token\" content=\"{csrf_token}\">\n\
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

#[cfg(test)]
mod ui_tests {
    #[test]
    fn publishing_the_kit_copies_it_once() {
        let dir = tempfile::tempdir().unwrap();
        let config = crate::Config {
            views_path: dir.path().join("views"),
            public_path: dir.path().join("public"),
            ..crate::Config::default()
        };
        super::publish_ui(&config, false).unwrap();
        let view = std::fs::read_to_string(dir.path().join("views/components/ui.html")).unwrap();
        assert!(
            view.contains("{% macro input(") && !view.contains("from \"renox/ui.html\" import")
        );
        assert!(
            std::fs::read_to_string(dir.path().join("public/css/renox-ui.css"))
                .unwrap()
                .contains("--rx-accent")
        );
        assert!(
            super::publish_ui(&config, false).is_err(),
            "no overwrite without --force"
        );
        super::publish_ui(&config, true).unwrap();
        assert!(super::ui_tags(false).starts_with("<script"));
        assert!(super::ui_tags(true).starts_with("<link rel=\"stylesheet\""));
    }
}
