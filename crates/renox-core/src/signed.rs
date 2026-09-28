//! Signed URLs: links that can't be altered and may expire, e.g. for email
//! verification. `state.signed_url(...)` builds one; the `ValidSignature`
//! extractor rejects tampered or expired ones with 403.

use std::fmt::Display;
use std::time::Duration;

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;

use crate::crypto::constant_time_eq;
use crate::{AppState, Error, Result};

fn now() -> u64 {
    crate::clock::unix_secs().max(0) as u64
}

pub(crate) fn signature(state: &AppState, payload: &str) -> String {
    let mut mac = <Hmac<Sha256> as KeyInit>::new_from_slice(state.key.signing())
        .expect("HMAC accepts keys of any length");
    mac.update(payload.as_bytes());
    mac.finalize()
        .into_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

impl AppState {
    /// An absolute URL to a named route, valid for `ttl` and signed with `APP_KEY`.
    pub fn signed_url(&self, name: &str, params: &[&dyn Display], ttl: Duration) -> Result<String> {
        let path = self.url(name, params)?;
        self.sign_path(&path, ttl)
    }

    /// Like `signed_url`, for an already-encoded path such as `/_renox/files/a.pdf`.
    pub fn sign_path(&self, path: &str, ttl: Duration) -> Result<String> {
        let unsigned = format!("{path}?expires={}", now().saturating_add(ttl.as_secs()));
        let signature = signature(self, &unsigned);
        Ok(format!(
            "{}{unsigned}&signature={signature}",
            self.config.url.trim_end_matches('/')
        ))
    }

    /// An absolute URL to a named route.
    pub fn absolute_url(&self, name: &str, params: &[&dyn Display]) -> Result<String> {
        Ok(format!(
            "{}{}",
            self.config.url.trim_end_matches('/'),
            self.url(name, params)?
        ))
    }
}

/// Proof that the request's URL was signed by `signed_url` and hasn't expired.
pub struct ValidSignature;

impl<S: Send + Sync> FromRequestParts<S> for ValidSignature {
    type Rejection = Error;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self> {
        let state = parts
            .extensions
            .get::<AppState>()
            .ok_or_else(|| anyhow::anyhow!("the auth middleware is not installed"))?;
        let query = parts.uri.query().unwrap_or_default();
        let (unsigned_query, given) = match query.rsplit_once("&signature=") {
            Some((rest, signature)) => (rest, signature),
            None => return Err(Error::Forbidden),
        };
        let expires: u64 = unsigned_query
            .strip_prefix("expires=")
            .and_then(|v| v.parse().ok())
            .ok_or(Error::Forbidden)?;
        let unsigned = format!("{}?{unsigned_query}", parts.uri.path());
        if expires < now() || !constant_time_eq(&signature(state, &unsigned), given) {
            return Err(Error::Forbidden);
        }
        Ok(Self)
    }
}
