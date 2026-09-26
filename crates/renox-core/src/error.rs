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
    Internal(anyhow::Error),
}

impl Error {
    pub fn status(&self) -> StatusCode {
        match self {
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::Forbidden => StatusCode::FORBIDDEN,
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl<E: Into<anyhow::Error>> From<E> for Error {
    fn from(err: E) -> Self {
        Self::Internal(err.into())
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
        (status, Html(error_page(status, detail.as_deref()))).into_response()
    }
}

fn error_page(status: StatusCode, detail: Option<&str>) -> String {
    let code = status.as_u16();
    let reason = status.canonical_reason().unwrap_or("Error");
    let detail = detail
        .map(|d| format!("<pre>{}</pre>", escape(d)))
        .unwrap_or_default();
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
         <title>{code} {reason}</title>\
         <style>body{{font-family:system-ui,sans-serif;max-width:48rem;margin:4rem auto;padding:0 1rem;color:#222}}\
         h1{{font-weight:600}}pre{{background:#f4f4f4;padding:1rem;overflow:auto;white-space:pre-wrap}}</style>\
         </head><body><h1>{code} · {reason}</h1>{detail}</body></html>"
    )
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
