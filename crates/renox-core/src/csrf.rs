use axum::body::{Body, to_bytes};
use axum::extract::Request;
use axum::http::{Method, header::CONTENT_TYPE};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::crypto::constant_time_eq;
use crate::{Error, Session};

pub const CSRF_HEADER: &str = "x-csrf-token";
pub const CSRF_FIELD: &str = "_token";

/// Largest urlencoded body the CSRF check buffers to look for `_token`.
const FORM_LIMIT: usize = 2 * 1024 * 1024;

/// Rejects state-changing requests that don't carry the session's CSRF token,
/// either in the `X-CSRF-Token` header (sent automatically for HTMX requests)
/// or in a `_token` form field (`{{ csrf_field() }}`).
pub(crate) async fn middleware(req: Request, next: Next) -> Response {
    if matches!(
        *req.method(),
        Method::GET | Method::HEAD | Method::OPTIONS | Method::TRACE
    ) {
        return next.run(req).await;
    }

    // API tokens are sent explicitly, not by the browser, so they can't be forged cross-site.
    if crate::auth::user_via_token(req.extensions()) {
        return next.run(req).await;
    }

    let Some(session) = req.extensions().get::<Session>().cloned() else {
        return Error::from(anyhow::anyhow!(
            "CSRF protection requires the session middleware"
        ))
        .into_response();
    };
    let expected = session.token();

    if let Some(token) = req.headers().get(CSRF_HEADER) {
        return match token.to_str() {
            Ok(token) if constant_time_eq(token, &expected) => next.run(req).await,
            _ => Error::PageExpired.into_response(),
        };
    }

    let is_form = req
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("application/x-www-form-urlencoded"));
    if !is_form {
        return Error::PageExpired.into_response();
    }

    let (parts, body) = req.into_parts();
    let bytes = match to_bytes(body, FORM_LIMIT).await {
        Ok(bytes) => bytes,
        Err(_) => return Error::BadRequest("The form is too large.".into()).into_response(),
    };
    let valid = form_urlencoded::parse(&bytes)
        .find(|(name, _)| name == CSRF_FIELD)
        .is_some_and(|(_, token)| constant_time_eq(&token, &expected));
    if !valid {
        return Error::PageExpired.into_response();
    }
    next.run(Request::from_parts(parts, Body::from(bytes)))
        .await
}
