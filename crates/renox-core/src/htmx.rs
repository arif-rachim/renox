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

/// Redirects to the previous page (the `Referer`), or `/` when unknown.
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
        Ok(Self(
            parts
                .headers
                .get(REFERER)
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned),
        ))
    }
}

impl IntoResponse for Back {
    fn into_response(self) -> Response {
        Redirect::to(self.0.as_deref().unwrap_or("/")).into_response()
    }
}
