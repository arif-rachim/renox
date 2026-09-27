//! Security headers on every response: a Content-Security-Policy (see
//! [`CspMode`], `CSP` in `.env`),
//! `X-Content-Type-Options`, `Referrer-Policy`, `X-Frame-Options`, and HSTS
//! in production over https. A header the handler already set is kept.
//!
//! Allow another site's scripts, images or frames with [`App::csp`](crate::App::csp):
//!
//! ```ignore
//! App::new().csp(|csp| {
//!     csp.allow("script-src", "https://www.googletagmanager.com")
//!        .allow("frame-src", "https://www.youtube.com");
//! })
//! ```
//!
//! With `CSP=strict`, inline scripts need the request's nonce:
//! `<script nonce="{{ csp_nonce() }}">…</script>`.

use std::collections::{BTreeMap, HashSet};
use std::convert::Infallible;

use axum::extract::{FromRequestParts, MatchedPath, Request, State};
use axum::http::header::{
    CONTENT_SECURITY_POLICY, REFERRER_POLICY, STRICT_TRANSPORT_SECURITY, X_CONTENT_TYPE_OPTIONS,
    X_FRAME_OPTIONS,
};
use axum::http::request::Parts;
use axum::http::{HeaderName, HeaderValue, Method};
use axum::middleware::Next;
use axum::response::Response;

use crate::AppState;
use crate::config::{Config, CspMode, Environment};
use crate::routing::RouteInfo;

/// Sources an app adds to the Content-Security-Policy, per directive.
#[derive(Debug, Clone, Default)]
pub struct Csp {
    extra: BTreeMap<String, Vec<String>>,
}

impl Csp {
    /// Allows `source` (e.g. `https://www.googletagmanager.com`) for
    /// `directive` (e.g. `script-src`, `img-src`, `connect-src`, `frame-src`).
    pub fn allow(&mut self, directive: &str, source: &str) -> &mut Self {
        self.extra
            .entry(directive.trim().to_ascii_lowercase())
            .or_default()
            .push(source.trim().to_owned());
        self
    }
}

/// The request's CSP nonce, also `csp_nonce()` in templates.
#[derive(Debug, Clone)]
pub struct CspNonce(pub String);

impl<S: Send + Sync> FromRequestParts<S> for CspNonce {
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Infallible> {
        Ok(parts
            .extensions
            .get::<CspNonce>()
            .cloned()
            .unwrap_or(CspNonce(String::new())))
    }
}

const NONCE: &str = "{nonce}";

/// What the middleware adds, worked out once at boot.
#[derive(Debug)]
pub(crate) struct Security {
    pub mode: CspMode,
    /// The policy, with `{nonce}` where each request's nonce goes.
    policy: Option<String>,
    hsts: bool,
    /// (method, path pattern) of routes marked `without_csrf()`; method `*` is any.
    csrf_exempt: HashSet<(String, String)>,
    /// Path patterns of webhook routes, which work in maintenance mode.
    webhook_paths: HashSet<String>,
}

impl Security {
    pub fn new(config: &Config, csp: &Csp, routes: &[RouteInfo]) -> Self {
        let script = match config.csp {
            CspMode::Strict => format!("'self' 'nonce-{NONCE}'"),
            _ => "'self' 'unsafe-inline' 'unsafe-eval'".to_owned(),
        };
        let mut directives: Vec<(String, String)> = [
            ("default-src", "'self'".to_owned()),
            ("script-src", script),
            ("style-src", "'self' 'unsafe-inline'".to_owned()),
            ("img-src", "'self' data: blob: https:".to_owned()),
            ("font-src", "'self' data: https:".to_owned()),
            ("connect-src", "'self'".to_owned()),
            ("frame-ancestors", "'self'".to_owned()),
            ("base-uri", "'self'".to_owned()),
            ("object-src", "'none'".to_owned()),
        ]
        .into_iter()
        .map(|(name, value)| (name.to_owned(), value))
        .collect();
        for (name, sources) in &csp.extra {
            let sources = sources.join(" ");
            match directives.iter_mut().find(|(n, _)| n == name) {
                Some((_, value)) => {
                    value.push(' ');
                    value.push_str(&sources);
                }
                // A new directive would otherwise stop falling back to
                // default-src, so it keeps this site too.
                None => directives.push((name.clone(), format!("'self' {sources}"))),
            }
        }
        let policy = (config.csp != CspMode::Off).then(|| {
            directives
                .iter()
                .map(|(name, value)| format!("{name} {value}"))
                .collect::<Vec<_>>()
                .join("; ")
        });
        let csrf_exempt = routes
            .iter()
            .filter(|route| route.middleware.iter().any(|m| m == "no-csrf"))
            .map(|route| (route.method.clone(), route.path.clone()))
            .collect();
        let webhook_paths = routes
            .iter()
            .filter(|route| route.middleware.iter().any(|m| m.starts_with("webhook:")))
            .map(|route| route.path.clone())
            .collect();
        Self {
            webhook_paths,
            mode: config.csp,
            policy,
            hsts: config.env == Environment::Production && config.url.starts_with("https://"),
            csrf_exempt,
        }
    }

    /// Whether the matched route receives webhooks.
    pub fn is_webhook(&self, path: Option<&MatchedPath>) -> bool {
        path.is_some_and(|path| self.webhook_paths.contains(path.as_str()))
    }

    /// Whether the route `req` matched was marked `without_csrf()`.
    pub fn skips_csrf(&self, method: &Method, path: Option<&MatchedPath>) -> bool {
        let Some(path) = path else {
            return false;
        };
        let path = path.as_str().to_owned();
        self.csrf_exempt
            .contains(&(method.as_str().to_owned(), path.clone()))
            || self.csrf_exempt.contains(&("*".to_owned(), path))
    }
}

pub(crate) async fn middleware(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Response {
    let security = &state.security;
    let nonce = crate::crypto::random_token();
    req.extensions_mut().insert(CspNonce(nonce.clone()));
    let mut res = next.run(req).await;

    let headers = res.headers_mut();
    let mut set = |name: HeaderName, value: &str| {
        if !headers.contains_key(&name)
            && let Ok(value) = HeaderValue::from_str(value)
        {
            headers.insert(name, value);
        }
    };
    set(X_CONTENT_TYPE_OPTIONS, "nosniff");
    set(REFERRER_POLICY, "strict-origin-when-cross-origin");
    set(X_FRAME_OPTIONS, "SAMEORIGIN");
    if security.hsts {
        set(STRICT_TRANSPORT_SECURITY, "max-age=31536000");
    }
    if let Some(policy) = &security.policy {
        set(CONTENT_SECURITY_POLICY, &policy.replace(NONCE, &nonce));
    }
    res
}
