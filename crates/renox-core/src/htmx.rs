use std::convert::Infallible;

use axum::extract::FromRequestParts;
use axum::http::header::REFERER;
use axum::http::request::Parts;
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, IntoResponseParts, Redirect, Response, ResponseParts};

/// What HTMX told us about the current request.
///
/// ```
/// # use renox::prelude::*;
/// async fn index(htmx: Htmx) -> View {
///     let page = view("produk/index.html", context! {});
///     if htmx.request { page.fragment("list") } else { page }
/// }
/// ```
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct Htmx {
    /// The request was made by HTMX (`HX-Request`).
    pub request: bool,
    /// The request comes from an `hx-boost` link or form, which expects a full page.
    pub boosted: bool,
    /// The id of the target element (`HX-Target`).
    pub target: Option<String>,
    /// The id of the element that triggered the request (`HX-Trigger`).
    pub trigger: Option<String>,
    /// The browser's current URL (`HX-Current-URL`).
    pub current_url: Option<String>,
}

impl Htmx {
    pub fn from_headers(headers: &HeaderMap) -> Self {
        let text = |name: &str| {
            headers
                .get(name)
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned)
        };
        Self {
            request: text("hx-request").as_deref() == Some("true"),
            boosted: text("hx-boosted").as_deref() == Some("true"),
            target: text("hx-target"),
            trigger: text("hx-trigger"),
            current_url: text("hx-current-url"),
        }
    }

    /// Goes to `to` after a form post: `HX-Redirect` for htmx requests (a
    /// full page load in the browser), a `303 See Other` otherwise.
    pub fn redirect(&self, to: &str) -> axum::response::Response {
        use axum::response::IntoResponse;
        if self.request {
            HxRedirect(to.to_owned()).into_response()
        } else {
            axum::response::Redirect::to(to).into_response()
        }
    }

    /// An HTMX request that wants a fragment rather than a full page.
    pub fn wants_fragment(&self) -> bool {
        self.request && !self.boosted
    }
}

impl<S: Send + Sync> FromRequestParts<S> for Htmx {
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Infallible> {
        Ok(Self::from_headers(&parts.headers))
    }
}

fn set(res: &mut ResponseParts, name: &'static str, value: &str) {
    match HeaderValue::from_str(value) {
        Ok(value) => {
            res.headers_mut()
                .insert(HeaderName::from_static(name), value);
        }
        Err(_) => tracing::warn!(header = name, "invalid header value dropped"),
    }
}

/// Makes HTMX do a full page load of the given URL (`HX-Redirect`).
pub struct HxRedirect(pub String);

impl IntoResponseParts for HxRedirect {
    type Error = Infallible;

    fn into_response_parts(self, mut res: ResponseParts) -> Result<ResponseParts, Infallible> {
        set(&mut res, "hx-redirect", &self.0);
        Ok(res)
    }
}

impl IntoResponse for HxRedirect {
    fn into_response(self) -> Response {
        (self, StatusCode::OK).into_response()
    }
}

/// Makes HTMX reload the whole page (`HX-Refresh`).
pub struct HxRefresh;

impl IntoResponseParts for HxRefresh {
    type Error = Infallible;

    fn into_response_parts(self, mut res: ResponseParts) -> Result<ResponseParts, Infallible> {
        set(&mut res, "hx-refresh", "true");
        Ok(res)
    }
}

impl IntoResponse for HxRefresh {
    fn into_response(self) -> Response {
        (self, StatusCode::OK).into_response()
    }
}

/// Triggers client-side events after the swap (`HX-Trigger`), e.g.
/// `(HxTrigger("produk-saved".into()), view(...))`.
pub struct HxTrigger(pub String);

impl IntoResponseParts for HxTrigger {
    type Error = Infallible;

    fn into_response_parts(self, mut res: ResponseParts) -> Result<ResponseParts, Infallible> {
        set(&mut res, "hx-trigger", &self.0);
        Ok(res)
    }
}

/// Makes HTMX swap the response into another element than the request's
/// `hx-target` (`HX-Retarget`), e.g. a form's errors into a summary box.
pub struct HxRetarget(pub String);

/// How HTMX swaps the response (`HX-Reswap`: `innerHTML`, `outerHTML`,
/// `beforeend`, `none`…), overriding the request's `hx-swap`.
pub struct HxReswap(pub String);

/// Puts a URL in the browser's address bar and history (`HX-Push-Url`), e.g.
/// the filters of a list; `HxPushUrl("false")` keeps it as it is.
pub struct HxPushUrl(pub String);

macro_rules! hx_header {
    ($type:ty, $header:literal) => {
        impl IntoResponseParts for $type {
            type Error = Infallible;

            fn into_response_parts(
                self,
                mut res: ResponseParts,
            ) -> Result<ResponseParts, Infallible> {
                set(&mut res, $header, &self.0);
                Ok(res)
            }
        }
    };
}

hx_header!(HxRetarget, "hx-retarget");
hx_header!(HxReswap, "hx-reswap");
hx_header!(HxPushUrl, "hx-push-url");

/// Adds the event `name` with `detail` to the response's `HX-Trigger`,
/// keeping the events already there.
pub(crate) fn add_trigger<B>(
    res: &mut axum::http::Response<B>,
    name: &str,
    detail: serde_json::Value,
) {
    use serde_json::{Map, Value};
    let mut triggers = match res
        .headers()
        .get("hx-trigger")
        .and_then(|v| v.to_str().ok())
    {
        Some(existing) if existing.trim_start().starts_with('{') => {
            serde_json::from_str::<Map<String, Value>>(existing).unwrap_or_default()
        }
        Some(existing) => existing
            .split(',')
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(|name| (name.to_owned(), Value::Null))
            .collect(),
        None => Map::new(),
    };
    triggers.insert(name.to_owned(), detail);
    if let Ok(value) = axum::http::HeaderValue::from_str(&Value::Object(triggers).to_string()) {
        res.headers_mut().insert("hx-trigger", value);
    }
}

/// Redirects to the previous page (the `Referer`), or `/` when it's unknown
/// or on another site (so a link from elsewhere can't use it as an open
/// redirect).
///
/// ```
/// # use renox::prelude::*;
/// async fn store(back: Back, session: Session) -> Result<Back> {
///     session.flash("status", "Tersimpan")?;
///     Ok(back)
/// }
/// ```
pub struct Back(Option<String>);

impl<S: Send + Sync> FromRequestParts<S> for Back {
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Infallible> {
        Ok(Self(same_site_referer(&parts.headers)))
    }
}

/// The `Referer` as a path on this site (`/produk?page=2`), or `None` when
/// it's missing or points anywhere else. The request's `Host` decides what
/// "this site" is.
pub(crate) fn same_site_referer(headers: &axum::http::HeaderMap) -> Option<String> {
    let referer = headers.get(REFERER)?.to_str().ok()?;
    if referer.starts_with('/') {
        return is_local_path(referer).then(|| referer.to_owned());
    }
    let host = headers.get(axum::http::header::HOST)?.to_str().ok()?;
    let rest = referer
        .strip_prefix("https://")
        .or_else(|| referer.strip_prefix("http://"))?;
    let (authority, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    (authority.eq_ignore_ascii_case(host) && is_local_path(path)).then(|| path.to_owned())
}

/// A path that stays on this site: starts with one `/`, and no `//` or `/\`
/// that browsers would read as another host.
pub(crate) fn is_local_path(path: &str) -> bool {
    path.starts_with('/')
        && !path.starts_with("//")
        && !path.starts_with("/\\")
        && !path.contains(['\\', '\r', '\n'])
}

impl IntoResponse for Back {
    fn into_response(self) -> Response {
        Redirect::to(self.0.as_deref().unwrap_or("/")).into_response()
    }
}
