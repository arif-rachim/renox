//! Security headers on every response: a Content-Security-Policy (see
//! [`CspMode`], `CSP` in `.env`),
//! `X-Content-Type-Options`, `Referrer-Policy`, `X-Frame-Options`, and HSTS
//! in production over https. A header the handler already set is kept.
//!
//! Allow another site's scripts, images or frames with [`App::csp`](crate::App::csp):
//!
//! ```
//! # use renox::prelude::*;
//! # let _ =
//! App::new().csp(|csp| {
//!     csp.allow("script-src", "https://www.googletagmanager.com")
//!        .allow("frame-src", "https://www.youtube.com");
//! })
//! # ;
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
use axum::response::{IntoResponse, Response};

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
    /// `App::xsrf_cookie`: the CSRF token also goes out as `XSRF-TOKEN`.
    pub xsrf_cookie: bool,
    /// `TRUSTED_HOSTS` plus `APP_URL`'s host; empty answers any host.
    trusted_hosts: Vec<String>,
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
        let mut extra = csp.extra.clone();
        for (directive, source) in crate::seo::csp_sources(config) {
            extra
                .entry(directive.to_owned())
                .or_default()
                .push(source.to_owned());
        }
        for (name, sources) in &extra {
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
            // A resource's update route is listed as `PUT|PATCH`.
            .flat_map(|route| {
                route
                    .method
                    .split('|')
                    .map(|method| (method.to_owned(), route.path.clone()))
            })
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
            xsrf_cookie: false,
            trusted_hosts: trusted_hosts(config),
        }
    }

    /// Whether the app answers requests for `host` (`TRUSTED_HOSTS`).
    pub fn allows_host(&self, host: Option<&str>) -> bool {
        if self.trusted_hosts.is_empty() {
            return true;
        }
        let Some(host) = host.map(str::to_ascii_lowercase) else {
            return false;
        };
        self.trusted_hosts
            .iter()
            .any(|allowed| match allowed.strip_prefix("*.") {
                Some(parent) => host
                    .strip_suffix(parent)
                    .is_some_and(|sub| sub.len() > 1 && sub.ends_with('.')),
                None => *allowed == host,
            })
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

/// Marks a response from a `Routes::etag()` route.
#[derive(Debug, Clone, Copy)]
pub(crate) struct WantsEtag;

/// The largest body `Routes::etag()` hashes.
const ETAG_LIMIT: u64 = 2 * 1024 * 1024;

/// Adds an `ETag` to a rendered page, or turns it into a 304 when the
/// browser's `If-None-Match` names it.
async fn etag(res: Response, method: &Method, if_none_match: Option<HeaderValue>) -> Response {
    use axum::body::HttpBody as _;
    use axum::http::StatusCode;
    let wanted = res.extensions().get::<WantsEtag>().is_some()
        && matches!(*method, Method::GET | Method::HEAD)
        && res.status() == StatusCode::OK
        && !res.headers().contains_key(axum::http::header::ETAG)
        && res
            .body()
            .size_hint()
            .exact()
            .is_some_and(|size| size <= ETAG_LIMIT);
    if !wanted {
        return res;
    }
    let (mut parts, body) = res.into_parts();
    let Ok(bytes) = axum::body::to_bytes(body, ETAG_LIMIT as usize).await else {
        return (StatusCode::INTERNAL_SERVER_ERROR, "could not read the page").into_response();
    };
    let hash = crate::webhook::sha256_hex(&bytes);
    let tag = format!("\"{}\"", &hash[..32]);
    let matches = if_none_match
        .as_ref()
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| {
            v.split(',')
                .map(|t| t.trim().trim_start_matches("W/"))
                .any(|t| t == tag || t == "*")
        });
    if let Ok(value) = HeaderValue::from_str(&tag) {
        parts.headers.insert(axum::http::header::ETAG, value);
    }
    if matches {
        parts.status = StatusCode::NOT_MODIFIED;
        parts.headers.remove(axum::http::header::CONTENT_LENGTH);
        parts.headers.remove(axum::http::header::CONTENT_TYPE);
        return Response::from_parts(parts, axum::body::Body::empty());
    }
    Response::from_parts(parts, axum::body::Body::from(bytes))
}

fn trusted_hosts(config: &Config) -> Vec<String> {
    let mut hosts = config.trusted_hosts.clone();
    if !hosts.is_empty()
        && let Ok(url) = config.url.parse::<axum::http::Uri>()
        && let Some(host) = url.host()
    {
        hosts.push(host.to_ascii_lowercase());
    }
    hosts
}

pub(crate) async fn middleware(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Response {
    // A request for a host the app doesn't serve (`TRUSTED_HOSTS`): links
    // built from it, such as password reset mails, would point elsewhere.
    // Load balancers' health checks often use an IP, so `/health` answers.
    if req.uri().path() != "/health"
        && !state
            .security
            .allows_host(crate::domain::host(&req).as_deref())
    {
        return (
            axum::http::StatusCode::BAD_REQUEST,
            "This host is not served here.",
        )
            .into_response();
    }
    let client = crate::client_ip::resolve(&req, &state.config.trusted_proxies);
    req.extensions_mut().insert(client);
    let security = &state.security;
    let nonce = crate::crypto::random_token();
    req.extensions_mut().insert(CspNonce(nonce.clone()));
    let wants_json = crate::error::wants_json(req.headers());
    let method = req.method().clone();
    let if_none_match = req
        .headers()
        .get(axum::http::header::IF_NONE_MATCH)
        .cloned();
    let mut res = next.run(req).await;
    res = etag(res, &method, if_none_match).await;
    // Errors from outside the view layer (e.g. CSRF's 419) for API clients.
    if wants_json && let Some(page) = res.extensions_mut().remove::<crate::error::ErrorPage>() {
        res = page.json(state.config.debug);
    }

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

#[cfg(test)]
mod tests {
    use std::pin::Pin;
    use std::task::{Context, Poll};

    use axum::body::{Body, Bytes, HttpBody};
    use axum::http::StatusCode;
    use axum::response::IntoResponse;

    use super::*;

    /// A page body that claims a size, then fails while being read.
    struct Broken;

    impl HttpBody for Broken {
        type Data = Bytes;
        type Error = std::io::Error;

        fn poll_frame(
            self: Pin<&mut Self>,
            _: &mut Context<'_>,
        ) -> Poll<Option<Result<http_body::Frame<Bytes>, std::io::Error>>> {
            Poll::Ready(Some(Err(std::io::Error::other("disk went away"))))
        }

        fn size_hint(&self) -> http_body::SizeHint {
            http_body::SizeHint::with_exact(10)
        }
    }

    #[tokio::test]
    async fn an_etag_page_that_cant_be_read_is_a_500() {
        let mut res = (StatusCode::OK, Body::new(Broken)).into_response();
        res.extensions_mut().insert(WantsEtag);
        let res = etag(res, &Method::GET, None).await;
        assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }
}
