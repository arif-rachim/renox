use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::{FromRequestParts, Request, State};
use axum::http::header::{COOKIE, SET_COOKIE};
use axum::http::request::Parts;
use axum::http::{HeaderMap, HeaderValue};
use axum::middleware::Next;
use axum::response::Response;
use cookie::{Cookie, CookieJar, Key, SameSite};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::crypto::random_token;
use crate::{AppState, Error, Result};

const OLD_INPUT: &str = "_old_input";
const ERRORS: &str = "_errors";

/// The current visitor's session, stored in an encrypted cookie.
///
/// Values put in the session last until it expires. Flashed values are
/// readable during the next request only, which is how "saved!" messages and
/// old form input survive a redirect.
#[derive(Clone)]
pub struct Session(Arc<Mutex<Inner>>);

#[derive(Default)]
struct Inner {
    data: Map<String, Value>,
    /// Flashed by the previous request, readable now.
    flashed: Map<String, Value>,
    /// Flashed by this request, readable during the next one.
    flash_next: Map<String, Value>,
    token: String,
}

#[derive(Serialize, Deserialize)]
struct Payload {
    #[serde(default)]
    data: Map<String, Value>,
    #[serde(default)]
    flash: Map<String, Value>,
    token: String,
    expires: u64,
}

impl Session {
    fn new(payload: Option<Payload>) -> Self {
        let inner = match payload {
            Some(p) => Inner {
                data: p.data,
                flashed: p.flash,
                flash_next: Map::new(),
                token: p.token,
            },
            None => Inner {
                token: random_token(),
                ..Inner::default()
            },
        };
        Self(Arc::new(Mutex::new(inner)))
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Reads a value, including values flashed by the previous request.
    pub fn get<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        let inner = self.lock();
        let value = inner
            .data
            .get(key)
            .or_else(|| inner.flash_next.get(key))
            .or_else(|| inner.flashed.get(key))?;
        serde_json::from_value(value.clone()).ok()
    }

    pub fn has(&self, key: &str) -> bool {
        let inner = self.lock();
        inner.data.contains_key(key)
            || inner.flash_next.contains_key(key)
            || inner.flashed.contains_key(key)
    }

    pub fn put(&self, key: &str, value: impl Serialize) -> Result {
        let value = serde_json::to_value(value)?;
        self.lock().data.insert(key.to_owned(), value);
        Ok(())
    }

    pub fn remove(&self, key: &str) -> Option<Value> {
        self.lock().data.remove(key)
    }

    /// Stores a value for the next request only.
    pub fn flash(&self, key: &str, value: impl Serialize) -> Result {
        let value = serde_json::to_value(value)?;
        self.lock().flash_next.insert(key.to_owned(), value);
        Ok(())
    }

    /// Keeps the values flashed by the previous request for one more request.
    pub fn reflash(&self) {
        let mut inner = self.lock();
        let flashed = inner.flashed.clone();
        for (key, value) in flashed {
            inner.flash_next.entry(key).or_insert(value);
        }
    }

    /// Flashes the submitted form so the next page can refill it with `old()`.
    pub fn flash_input(&self, input: &impl Serialize) -> Result {
        self.flash(OLD_INPUT, input)
    }

    /// A field from the input flashed by the previous request.
    pub fn old(&self, field: &str) -> Option<Value> {
        self.lock()
            .flashed
            .get(OLD_INPUT)
            .and_then(|input| input.get(field))
            .cloned()
    }

    /// Flashes validation errors, keyed by field, for the next request.
    pub fn flash_errors(&self, errors: &impl Serialize) -> Result {
        self.flash(ERRORS, errors)
    }

    /// Validation errors flashed by the previous request.
    pub fn errors(&self) -> Map<String, Value> {
        match self.lock().flashed.get(ERRORS) {
            Some(Value::Object(errors)) => errors.clone(),
            _ => Map::new(),
        }
    }

    /// Values flashed by the previous request, except old input and errors.
    pub fn flashed(&self) -> Map<String, Value> {
        let mut flashed = self.lock().flashed.clone();
        flashed.remove(OLD_INPUT);
        flashed.remove(ERRORS);
        flashed
    }

    /// The CSRF token forms and HTMX requests must send back.
    pub fn token(&self) -> String {
        self.lock().token.clone()
    }

    /// Issues a new CSRF token, e.g. after logging in.
    pub fn regenerate_token(&self) {
        self.lock().token = random_token();
    }

    /// Removes all data and flashed values and issues a new CSRF token.
    pub fn flush(&self) {
        let mut inner = self.lock();
        *inner = Inner {
            token: random_token(),
            ..Inner::default()
        };
    }

    fn to_payload(&self, expires: u64) -> Payload {
        let inner = self.lock();
        Payload {
            data: inner.data.clone(),
            flash: inner.flash_next.clone(),
            token: inner.token.clone(),
            expires,
        }
    }
}

impl<S: Send + Sync> FromRequestParts<S> for Session {
    type Rejection = Error;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self> {
        parts
            .extensions
            .get::<Session>()
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("the session middleware is not installed").into())
    }
}

pub(crate) async fn middleware(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Response {
    let config = &state.config;
    let now = unix_now();
    let payload = read_cookie(req.headers(), &config.session_cookie, &state.key)
        .filter(|payload| payload.expires > now);
    let session = Session::new(payload);
    req.extensions_mut().insert(session.clone());

    let mut res = next.run(req).await;

    let lifetime = config.session_lifetime * 60;
    let payload = session.to_payload(now + lifetime);
    let value = match serde_json::to_string(&payload) {
        Ok(value) => value,
        Err(err) => {
            tracing::error!(error = %err, "could not serialize the session");
            return res;
        }
    };
    let cookie = Cookie::build((config.session_cookie.clone(), value))
        .path("/")
        .http_only(true)
        .same_site(SameSite::Lax)
        .secure(config.url.starts_with("https://"))
        .max_age(cookie::time::Duration::seconds(lifetime as i64))
        .build();
    let mut jar = CookieJar::new();
    jar.private_mut(&state.key).add(cookie);
    for cookie in jar.delta() {
        let encoded = cookie.encoded().to_string();
        if encoded.len() > 4000 {
            tracing::warn!(
                bytes = encoded.len(),
                "the session cookie is larger than browsers reliably store"
            );
        }
        if let Ok(value) = HeaderValue::from_str(&encoded) {
            res.headers_mut().append(SET_COOKIE, value);
        }
    }
    res
}

fn read_cookie(headers: &HeaderMap, name: &str, key: &Key) -> Option<Payload> {
    let mut jar = CookieJar::new();
    for header in headers.get_all(COOKIE) {
        let Ok(header) = header.to_str() else {
            continue;
        };
        for cookie in Cookie::split_parse_encoded(header.to_owned()).flatten() {
            if cookie.name() == name {
                jar.add_original(cookie);
            }
        }
    }
    let cookie = jar.private(key).get(name)?;
    serde_json::from_str(cookie.value()).ok()
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default()
}
