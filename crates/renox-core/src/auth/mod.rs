//! Authentication and authorization: the `users` table, password hashing,
//! session login with "remember me", route guards, policies and gates.
//!
//! ```
//! # #[derive(Model, serde::Serialize, Default)]
//! # #[model(table = "produk")]
//! # struct Produk { id: i64, nama: String, harga: i64, kategori: Option<String>, user_id: i64 }
//! # use renox::prelude::*;
//! # impl Policy for Produk { fn allows(&self, user: &User, _: &str) -> bool { self.user_id == user.id } }
//! # let _ =
//! App::new()
//!     .module(Auth::new())                       // /login, /register, /logout
//!     .gate("admin", |user| user.email.ends_with("@toko.id"))
//!     .module(Toko)
//! # ;
//!
//! struct Toko;
//!
//! impl Module for Toko {
//! #   fn name(&self) -> &'static str { "toko" }
//!     fn routes(&self) -> Routes {
//!         Routes::new()
//!             .get("/produk/{id}/edit", edit)
//!             .require_auth()                    // everything above needs a login
//!     }
//! }
//!
//! async fn edit(auth: AuthUser, State(db): State<Db>, Path(id): Path<i64>) -> Result<View> {
//!     let produk = Produk::find_or_404(&db, id).await?;
//!     auth.authorize("update", &produk)?;        // 403 unless the policy allows it
//!     Ok(view("produk/edit.html", context! { produk }))
//! }
//! ```

mod module;
pub mod notifications;
mod passwords;
mod throttle;
mod tokens;
mod user;
mod verification;

use std::collections::HashMap;
use std::convert::Infallible;
use std::ops::Deref;
use std::sync::Arc;

use axum::extract::{FromRequestParts, OptionalFromRequestParts, Request};
use axum::http::header::{ACCEPT, AUTHORIZATION};
use axum::http::request::Parts;
use axum::http::{Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Redirect, Response};

pub use module::Auth;
pub use notifications::{Channel, DatabaseNotification, Notification};
pub(crate) use throttle::Throttle;
pub use tokens::{AccessToken, NewToken};
pub use user::{User, hash_password, verify_password};
pub use verification::send_verification;

use crate::crypto::constant_time_eq;
use crate::db::Model;
use crate::{AppState, Error, Htmx, HxRedirect, Result, Session};

const AUTH_ID: &str = "_auth_user_id";
const AUTH_HASH: &str = "_auth_password_hash";
const INTENDED: &str = "_intended";

/// Decides whether a user may perform an ability on a model.
///
/// ```
/// # #[derive(Model, serde::Serialize, Default)]
/// # #[model(table = "produk")]
/// # struct Produk { id: i64, nama: String, harga: i64, kategori: Option<String>, user_id: i64 }
/// # use renox::prelude::*;
/// impl Policy for Produk {
///     fn allows(&self, user: &User, ability: &str) -> bool {
///         match ability {
///             "update" | "delete" => self.user_id == user.id,
///             _ => true,
///         }
///     }
/// }
/// ```
pub trait Policy {
    fn allows(&self, user: &User, ability: &str) -> bool;
}

/// A model together with what the current user may do with it, so templates
/// can ask the policy: `{% if can('update', product) %}`.
///
/// ```
/// # use renox::prelude::*;
/// # #[derive(Model, serde::Serialize, Default)] struct Product { id: i64, user_id: i64 }
/// # impl Policy for Product { fn allows(&self, user: &User, _: &str) -> bool { self.user_id == user.id } }
/// async fn index(State(db): State<Db>, user: Option<AuthUser>, Page(page): Page) -> Result<View> {
///     let products = Product::query().paginate(&db, page, 20).await?
///         .map(|p| Can::new(p, user.as_deref(), &["update", "delete"]));
///     Ok(view("products/index.html", context! { products }))
/// }
/// ```
///
/// It serializes as the model's own fields plus `_can` (`{"update": true, …}`);
/// guests get `false` for every ability.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Can<T> {
    #[serde(flatten)]
    pub item: T,
    #[serde(rename = "_can")]
    pub abilities: std::collections::BTreeMap<String, bool>,
}

impl<T: Policy> Can<T> {
    pub fn new(item: T, user: Option<&User>, abilities: &[&str]) -> Self {
        let abilities = abilities
            .iter()
            .map(|ability| {
                let allowed = user.is_some_and(|user| item.allows(user, ability));
                ((*ability).to_owned(), allowed)
            })
            .collect();
        Self { item, abilities }
    }
}

pub(crate) type Gate = Arc<dyn Fn(&User) -> bool + Send + Sync>;
pub(crate) type Gates = Arc<HashMap<String, Gate>>;

/// The logged-in user of the current request, if any; set by Renox's auth
/// middleware for every request.
#[derive(Clone)]
pub(crate) struct CurrentUser {
    pub user: Option<Arc<User>>,
    pub gates: Gates,
    /// Authenticated with `Authorization: Bearer`, so CSRF doesn't apply.
    pub via_token: bool,
}

/// The logged-in user. Requests without one are sent to the `login` route
/// (HTMX requests via `HX-Redirect`) or get 401 when they want JSON. Use
/// `Option<AuthUser>` where logging in is optional.
#[derive(Clone)]
pub struct AuthUser {
    user: Arc<User>,
    gates: Gates,
}

impl Deref for AuthUser {
    type Target = User;

    fn deref(&self) -> &User {
        &self.user
    }
}

impl AuthUser {
    /// Whether the policy of `target` allows `ability`.
    pub fn can(&self, ability: &str, target: &impl Policy) -> bool {
        target.allows(&self.user, ability)
    }

    /// Like `can`, but a refusal becomes a 403 response.
    pub fn authorize(&self, ability: &str, target: &impl Policy) -> Result {
        if self.can(ability, target) {
            Ok(())
        } else {
            Err(Error::Forbidden)
        }
    }

    /// Whether the gate named `gate` lets this user through. Unknown gates deny.
    pub fn allows(&self, gate: &str) -> bool {
        self.gates.get(gate).is_some_and(|check| check(&self.user))
    }

    /// Like `allows`, but a refusal becomes a 403 response.
    pub fn gate(&self, gate: &str) -> Result {
        if self.allows(gate) {
            Ok(())
        } else {
            Err(Error::Forbidden)
        }
    }

    pub fn user(&self) -> &User {
        &self.user
    }
}

impl<S: Send + Sync> FromRequestParts<S> for AuthUser {
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> std::result::Result<Self, Response> {
        match current(&parts.extensions) {
            Some(user) => Ok(user),
            None => Err(unauthenticated(parts)),
        }
    }
}

impl<S: Send + Sync> OptionalFromRequestParts<S> for AuthUser {
    type Rejection = Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        _: &S,
    ) -> std::result::Result<Option<Self>, Infallible> {
        Ok(current(&parts.extensions))
    }
}

fn current(extensions: &axum::http::Extensions) -> Option<AuthUser> {
    let current = extensions.get::<CurrentUser>()?;
    Some(AuthUser {
        user: current.user.clone()?,
        gates: current.gates.clone(),
    })
}

/// Logs `user` in for this session. With `remember`, the session lasts
/// `REMEMBER_LIFETIME` instead of `SESSION_LIFETIME`.
pub fn login(session: &Session, user: &User, remember: Option<u64>) -> Result {
    session.regenerate_token();
    session.put(AUTH_ID, user.id)?;
    session.put(AUTH_HASH, fingerprint(&user.password))?;
    if let Some(minutes) = remember {
        session.set_lifetime(minutes);
    }
    Ok(())
}

/// Ends the session entirely: user, data and CSRF token.
pub fn logout(session: &Session) {
    session.flush();
}

/// Ties a session to the password it was logged in with, so changing the
/// password logs out every other session.
fn fingerprint(password_hash: &str) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(password_hash.as_bytes());
    digest.iter().take(16).map(|b| format!("{b:02x}")).collect()
}

/// Loads the session's user once per request for extractors, guards and templates.
pub(crate) async fn middleware(
    axum::extract::State(state): axum::extract::State<AppState>,
    mut req: Request,
    next: Next,
) -> Response {
    let bearer = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(str::to_owned);
    let session = req.extensions().get::<Session>().cloned();
    let (user, via_token) = match (&bearer, &session) {
        (Some(bearer), _) => match tokens::authenticate(&state.db, bearer.trim()).await {
            Ok(user) => (user, true),
            Err(err) => {
                tracing::error!(error = ?err, "could not check the API token");
                (None, true)
            }
        },
        (None, Some(session)) => (resolve(&state, session).await, false),
        (None, None) => (None, false),
    };
    req.extensions_mut().insert(CurrentUser {
        user: user.map(Arc::new),
        gates: state.gates.clone(),
        via_token,
    });
    req.extensions_mut().insert(state);
    next.run(req).await
}

async fn resolve(state: &AppState, session: &Session) -> Option<User> {
    let id: i64 = session.get(AUTH_ID)?;
    let hash: String = session.get(AUTH_HASH).unwrap_or_default();
    match User::find(&state.db, id).await {
        Ok(Some(user)) if constant_time_eq(&fingerprint(&user.password), &hash) => Some(user),
        Ok(_) => {
            // Deleted user or changed password: this session is no longer theirs.
            session.remove(AUTH_ID);
            session.remove(AUTH_HASH);
            None
        }
        Err(err) => {
            tracing::error!(error = ?err, "could not load the logged-in user");
            None
        }
    }
}

fn wants_json(headers: &axum::http::HeaderMap) -> bool {
    headers
        .get(ACCEPT)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.contains("application/json"))
}

fn path_or(state: Option<&AppState>, route: &str, fallback: &str) -> String {
    state
        .and_then(|s| s.url(route, &[]).ok())
        .unwrap_or_else(|| fallback.to_owned())
}

/// Sends a guest to the login page, remembering where they were going.
fn unauthenticated(parts: &Parts) -> Response {
    if wants_json(&parts.headers) || parts.headers.contains_key(AUTHORIZATION) {
        let body = serde_json::json!({ "message": "Unauthenticated." });
        return (StatusCode::UNAUTHORIZED, axum::Json(body)).into_response();
    }
    let login = path_or(parts.extensions.get::<AppState>(), "login", "/login");
    if let (Some(session), &Method::GET) = (parts.extensions.get::<Session>(), &parts.method) {
        let intended = parts
            .uri
            .path_and_query()
            .map(|p| p.as_str().to_owned())
            .unwrap_or_else(|| "/".into());
        let _ = session.put(INTENDED, intended);
    }
    if Htmx::from_headers(&parts.headers).request {
        return HxRedirect(login).into_response();
    }
    Redirect::to(&login).into_response()
}

/// Route guard: only logged-in users. See `Routes::require_auth`.
pub(crate) async fn require_auth(req: Request, next: Next) -> Response {
    if current(req.extensions()).is_some() {
        return next.run(req).await;
    }
    let (parts, _) = req.into_parts();
    unauthenticated(&parts)
}

/// Route guard: only users who verified their email. See `Routes::require_verified`.
pub(crate) async fn require_verified(req: Request, next: Next) -> Response {
    let Some(user) = current(req.extensions()) else {
        let (parts, _) = req.into_parts();
        return unauthenticated(&parts);
    };
    if user.email_verified_at.is_some() {
        return next.run(req).await;
    }
    if wants_json(req.headers()) || user_via_token(req.extensions()) {
        let body = serde_json::json!({ "message": "Your email address is not verified." });
        return (StatusCode::FORBIDDEN, axum::Json(body)).into_response();
    }
    let notice = path_or(
        req.extensions().get::<AppState>(),
        "verification.notice",
        "/verify-email",
    );
    if Htmx::from_headers(req.headers()).request {
        return HxRedirect(notice).into_response();
    }
    Redirect::to(&notice).into_response()
}

pub(crate) fn user_via_token(extensions: &axum::http::Extensions) -> bool {
    extensions.get::<CurrentUser>().is_some_and(|c| c.via_token)
}

/// Route guard: only guests; logged-in users go to the `home` route.
pub(crate) async fn guest_only(req: Request, next: Next) -> Response {
    if current(req.extensions()).is_none() {
        return next.run(req).await;
    }
    Redirect::to(&path_or(req.extensions().get::<AppState>(), "home", "/")).into_response()
}

/// Where to go after logging in: the page that asked for a login, else `fallback`.
pub(crate) fn intended(session: &Session, fallback: String) -> String {
    session
        .pull::<String>(INTENDED)
        .filter(|path| path.starts_with('/') && !path.starts_with("//"))
        .unwrap_or(fallback)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprints_are_short_and_stable() {
        let a = fingerprint("$argon2id$v=19$abc");
        assert_eq!(a.len(), 32);
        assert_eq!(a, fingerprint("$argon2id$v=19$abc"));
        assert_ne!(a, fingerprint("$argon2id$v=19$abd"));
    }
}
