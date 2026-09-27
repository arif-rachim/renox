//! Maintenance mode: `my-app down` makes the site answer 503 (with
//! `errors/503.html` or the built-in error page) until `my-app up`.
//!
//! `my-app down --secret s3cr3t` lets you in anyway: visiting `/s3cr3t` sets a
//! cookie that bypasses the 503. `/health` and Renox's assets keep working.
//! The state lives in `storage/framework/down`, so every process sees it.

use std::path::{Path, PathBuf};

use axum::extract::{Request, State};
use axum::http::HeaderValue;
use axum::http::header::{COOKIE, SET_COOKIE};
use axum::middleware::Next;
use axum::response::{IntoResponse, Redirect, Response};
use serde::{Deserialize, Serialize};

use crate::crypto::constant_time_eq;
use crate::queue::unix_now;
use crate::{AppState, Error, Result};

const BYPASS_COOKIE: &str = "renox_maintenance";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Down {
    pub since: i64,
    /// Seconds to suggest in `Retry-After`.
    pub retry: Option<u64>,
    pub secret: Option<String>,
}

pub(crate) fn file(storage: &Path) -> PathBuf {
    storage.join("framework").join("down")
}

/// Takes the app down.
pub fn down(storage: &Path, secret: Option<String>, retry: Option<u64>) -> Result {
    let path = file(storage);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let state = Down {
        since: unix_now(),
        retry,
        secret,
    };
    std::fs::write(path, serde_json::to_string(&state)?)?;
    Ok(())
}

/// Brings the app back; returns whether it was down.
pub fn up(storage: &Path) -> Result<bool> {
    match std::fs::remove_file(file(storage)) {
        Ok(()) => Ok(true),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(err) => Err(err.into()),
    }
}

pub fn status(storage: &Path) -> Option<Down> {
    let text = std::fs::read_to_string(file(storage)).ok()?;
    serde_json::from_str(&text).ok()
}

fn has_bypass(req: &Request, secret: &str) -> bool {
    req.headers()
        .get_all(COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .any(|(name, value)| name == BYPASS_COOKIE && constant_time_eq(value, secret))
}

pub(crate) async fn middleware(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Response {
    let Some(down) = status(&state.config.storage_path) else {
        return next.run(req).await;
    };
    // Providers would retry, but may give up; store the calls instead.
    if state
        .security
        .is_webhook(req.extensions().get::<axum::extract::MatchedPath>())
    {
        return next.run(req).await;
    }
    if let Some(secret) = &down.secret {
        if has_bypass(&req, secret) {
            return next.run(req).await;
        }
        if req.uri().path().trim_start_matches('/') == secret {
            let mut res = Redirect::to("/").into_response();
            let cookie = format!("{BYPASS_COOKIE}={secret}; Path=/; HttpOnly; SameSite=Lax");
            if let Ok(value) = HeaderValue::from_str(&cookie) {
                res.headers_mut().append(SET_COOKIE, value);
            }
            return res;
        }
    }
    let mut res = Error::ServiceUnavailable.into_response();
    if let Some(retry) = down.retry {
        res.headers_mut().insert("retry-after", retry.into());
    }
    res
}
