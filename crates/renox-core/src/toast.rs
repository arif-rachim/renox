//! Toasts: short messages that confirm what just happened ("Saved"),
//! shown over the page and announced to screen readers.
//!
//! ```
//! # use renox::prelude::*;
//! use renox::Toast;
//!
//! async fn save() -> (Toast, Redirect) {
//!     // … save …
//!     (Toast::success("Product saved"), Redirect::to("/products"))
//! }
//! ```
//!
//! After a redirect the toast waits in the session for the next page; an
//! htmx request gets it at once (in `HX-Trigger`). Put `{{ toasts() }}` in
//! the layout, once, and include `renox_ui_styles()`.

use axum::response::{IntoResponse, IntoResponseParts, Response, ResponseParts};
use serde::{Deserialize, Serialize};

/// Session key of the toasts waiting for the next page.
pub(crate) const SESSION_KEY: &str = "_toasts";
/// The htmx event that carries toasts.
pub(crate) const EVENT: &str = "renox:toast";

/// What a toast says about the outcome: its color, icon and whether it
/// stays until dismissed (errors do: they need reading).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum ToastKind {
    Success,
    Info,
    Warning,
    Error,
}

/// A toast; return it with the response. See the [module docs](self).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Toast {
    pub kind: ToastKind,
    pub message: String,
}

impl Toast {
    pub fn new(kind: ToastKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    pub fn success(message: impl Into<String>) -> Self {
        Self::new(ToastKind::Success, message)
    }

    pub fn info(message: impl Into<String>) -> Self {
        Self::new(ToastKind::Info, message)
    }

    pub fn warning(message: impl Into<String>) -> Self {
        Self::new(ToastKind::Warning, message)
    }

    /// Stays until dismissed.
    pub fn error(message: impl Into<String>) -> Self {
        Self::new(ToastKind::Error, message)
    }
}

/// Toasts attached to a response, delivered by the view middleware.
#[derive(Debug, Clone, Default)]
pub(crate) struct PendingToasts(pub Vec<Toast>);

impl IntoResponseParts for Toast {
    type Error = std::convert::Infallible;

    fn into_response_parts(self, mut res: ResponseParts) -> Result<ResponseParts, Self::Error> {
        let pending = res.extensions_mut().get_mut::<PendingToasts>();
        match pending {
            Some(pending) => pending.0.push(self),
            None => {
                res.extensions_mut().insert(PendingToasts(vec![self]));
            }
        }
        Ok(res)
    }
}

impl IntoResponse for Toast {
    fn into_response(self) -> Response {
        (self, axum::http::StatusCode::NO_CONTENT).into_response()
    }
}

/// The toast region's markup, with the toasts waiting in `waiting`. The
/// bundled renox.js adds the ones htmx responses bring, and dismisses.
pub(crate) fn region(waiting: &[Toast], dismiss: &str) -> String {
    let mut out = format!(
        r#"<div class="rx-toasts" data-renox-toasts data-dismiss-label="{}" aria-live="polite" aria-relevant="additions">"#,
        escape(dismiss)
    );
    for toast in waiting {
        out.push_str(&toast_html(toast, dismiss));
    }
    out.push_str("</div>");
    out
}

pub(crate) fn toast_html(toast: &Toast, dismiss: &str) -> String {
    let (kind, role, icon) = match toast.kind {
        ToastKind::Success => ("success", "status", ICON_CHECK),
        ToastKind::Info => ("info", "status", ICON_INFO),
        ToastKind::Warning => ("warning", "status", ICON_WARNING),
        ToastKind::Error => ("error", "alert", ICON_ERROR),
    };
    format!(
        r#"<div class="rx-toast rx-toast--{kind}" role="{role}" data-renox-toast{sticky}><span class="rx-toast__icon" aria-hidden="true">{icon}</span><p class="rx-toast__message">{message}</p><button type="button" class="rx-toast__close" data-renox-dismiss aria-label="{dismiss}">{ICON_CLOSE}</button></div>"#,
        sticky = if toast.kind == ToastKind::Error {
            " data-sticky"
        } else {
            ""
        },
        message = escape(&toast.message),
        dismiss = escape(dismiss),
    )
}

pub(crate) fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#x27;"),
            c => out.push(c),
        }
    }
    out
}

const ICON_CHECK: &str = r##"<svg viewBox="0 0 20 20" width="20" height="20"><circle cx="10" cy="10" r="9" fill="currentColor"/><path d="M6 10.5l2.5 2.5L14 7.5" stroke="#fff" stroke-width="2" fill="none" stroke-linecap="round" stroke-linejoin="round"/></svg>"##;
const ICON_INFO: &str = r##"<svg viewBox="0 0 20 20" width="20" height="20"><circle cx="10" cy="10" r="9" fill="currentColor"/><path d="M10 9v5M10 6h.01" stroke="#fff" stroke-width="2" stroke-linecap="round"/></svg>"##;
const ICON_WARNING: &str = r##"<svg viewBox="0 0 20 20" width="20" height="20"><path d="M10 2l8.5 15h-17z" fill="currentColor" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round"/><path d="M10 8v4M10 14.5h.01" stroke="#fff" stroke-width="2" stroke-linecap="round"/></svg>"##;
const ICON_ERROR: &str = r##"<svg viewBox="0 0 20 20" width="20" height="20"><circle cx="10" cy="10" r="9" fill="currentColor"/><path d="M7 7l6 6M13 7l-6 6" stroke="#fff" stroke-width="2" stroke-linecap="round"/></svg>"##;
const ICON_CLOSE: &str = r##"<svg viewBox="0 0 20 20" width="16" height="16" aria-hidden="true"><path d="M5 5l10 10M15 5L5 15" stroke="currentColor" stroke-width="2" stroke-linecap="round"/></svg>"##;
