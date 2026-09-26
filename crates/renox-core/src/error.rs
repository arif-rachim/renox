use std::sync::atomic::{AtomicBool, Ordering};

use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};

/// Whether error responses include details. Set once by `App` from `Config::debug`.
static DEBUG: AtomicBool = AtomicBool::new(false);

pub(crate) fn set_debug(debug: bool) {
    DEBUG.store(debug, Ordering::Relaxed);
}

pub type Result<T = (), E = Error> = std::result::Result<T, E>;

/// The error type handlers return. Any `anyhow`-compatible error converts into
/// `Error::Internal` with `?`.
#[derive(Debug)]
pub enum Error {
    BadRequest(String),
    Unauthorized,
    Forbidden,
    NotFound,
    /// The CSRF token was missing or wrong, usually because the session expired.
    PageExpired,
    Internal(anyhow::Error),
}

impl Error {
    pub fn status(&self) -> StatusCode {
        match self {
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::Forbidden => StatusCode::FORBIDDEN,
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::PageExpired => StatusCode::from_u16(419).expect("valid status code"),
            Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl<E: Into<anyhow::Error>> From<E> for Error {
    fn from(err: E) -> Self {
        Self::Internal(err.into())
    }
}

/// Marks an error response so the view middleware can render it with
/// `errors/{status}.html` or the built-in `renox/error.html`.
#[derive(Debug, Clone)]
pub(crate) struct ErrorPage {
    pub status: StatusCode,
    pub detail: Option<String>,
}

pub(crate) fn reason(status: StatusCode) -> &'static str {
    match status.as_u16() {
        419 => "Page Expired",
        _ => status.canonical_reason().unwrap_or("Error"),
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let status = self.status();
        let detail = match &self {
            Self::Internal(err) => {
                tracing::error!(error = ?err, "internal server error");
                DEBUG.load(Ordering::Relaxed).then(|| format!("{err:?}"))
            }
            Self::BadRequest(msg) => Some(msg.clone()),
            _ => None,
        };
        let mut res = (status, Html(error_page(status, detail.as_deref()))).into_response();
        res.extensions_mut().insert(ErrorPage { status, detail });
        res
    }
}

/// Plain fallback used when the error page template can't be rendered.
fn error_page(status: StatusCode, detail: Option<&str>) -> String {
    let code = status.as_u16();
    let reason = reason(status);
    let detail = detail
        .map(|d| format!("<pre>{}</pre>", escape(d)))
        .unwrap_or_default();
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
         <title>{code} {reason}</title></head>\
         <body><h1>{code} · {reason}</h1>{detail}</body></html>"
    )
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
