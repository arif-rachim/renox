use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use axum::extract::{ConnectInfo, Extension, FromRequestParts, State};
use axum::http::request::Parts;
use axum::response::{IntoResponse, Redirect, Response};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::user::dummy_hash;
use super::{User, intended, login, logout, verify_password};
use crate::db::Migration;
use crate::validation::{Errors, Locale, Valid, Validate, ValidationError, Validator};
use crate::{AppState, Htmx, HxRedirect, Module, Result, Routes, Session, View, context, view};

const MIGRATIONS: &[Migration] = &[Migration {
    name: "00010101000000_create_users_table",
    up: include_str!("../../migrations/auth/00010101000000_create_users_table.up.sql"),
    down: Some(include_str!(
        "../../migrations/auth/00010101000000_create_users_table.down.sql"
    )),
}];

struct Settings {
    registration: bool,
    redirect_to: Option<String>,
}

/// Login, registration and logout pages, and the `users` table.
///
/// Routes: `login` (GET/POST `/login`), `register` (GET/POST `/register`) and
/// `logout` (POST `/logout`). The pages are `renox/auth/login.html` and
/// `renox/auth/register.html`, inside `renox/auth/layout.html`; create a file
/// with the same name under your views to replace one.
#[derive(Clone)]
pub struct Auth {
    registration: bool,
    redirect_to: Option<String>,
}

impl Auth {
    pub fn new() -> Self {
        Self {
            registration: true,
            redirect_to: None,
        }
    }

    /// Hides `/register`, e.g. for back-office apps where an admin adds users.
    pub fn without_registration(mut self) -> Self {
        self.registration = false;
        self
    }

    /// Where to go after logging in or registering when no page asked for
    /// the login. Defaults to the `home` route, or `/`.
    pub fn redirect_to(mut self, path: &str) -> Self {
        self.redirect_to = Some(path.to_owned());
        self
    }
}

impl Default for Auth {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for Auth {
    fn name(&self) -> &'static str {
        "auth"
    }

    fn migrations(&self) -> &'static [Migration] {
        MIGRATIONS
    }

    fn routes(&self) -> Routes {
        let settings = Arc::new(Settings {
            registration: self.registration,
            redirect_to: self.redirect_to.clone(),
        });

        let mut guest = Routes::new()
            .get("/login", show_login)
            .post("/login", store_login)
            .name("login");
        if self.registration {
            guest = guest
                .get("/register", show_register)
                .post("/register", store_register)
                .name("register");
        }
        guest
            .guest_only()
            .merge(Routes::new().post("/logout", destroy).name("logout"))
            .route_layer(Extension(settings))
    }
}

/// The client's IP address, when the server knows it.
struct ClientIp(Option<IpAddr>);

impl<S: Send + Sync> FromRequestParts<S> for ClientIp {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        _: &S,
    ) -> std::result::Result<Self, Self::Rejection> {
        Ok(Self(
            parts
                .extensions
                .get::<ConnectInfo<SocketAddr>>()
                .map(|info| info.0.ip()),
        ))
    }
}

fn locale(state: &AppState) -> Locale {
    Locale::parse(&state.config.locale)
}

fn after_login(state: &AppState, settings: &Settings, session: &Session) -> String {
    let fallback = settings
        .redirect_to
        .clone()
        .unwrap_or_else(|| state.url("home", &[]).unwrap_or_else(|_| "/".into()));
    intended(session, fallback)
}

/// A full page load after logging in or out, since the layout changes.
fn go(htmx: &Htmx, to: String) -> Response {
    if htmx.request {
        HxRedirect(to).into_response()
    } else {
        Redirect::to(&to).into_response()
    }
}

async fn show_login(
    Extension(settings): Extension<Arc<Settings>>,
    State(state): State<AppState>,
) -> View {
    view(
        "renox/auth/login.html",
        context! { registration => settings.registration, text => text(locale(&state)) },
    )
}

#[derive(Deserialize, Serialize)]
struct LoginForm {
    email: String,
    password: String,
    remember: Option<String>,
}

/// Field labels in messages, matching the words on the built-in pages.
fn label(v: &Validator, field: &'static str) -> &'static str {
    match (v.locale(), field) {
        (Locale::Id, "name") => "nama",
        (Locale::Id, "password") => "kata sandi",
        _ => field,
    }
}

impl Validate for LoginForm {
    fn rules(&self, v: &mut Validator) {
        v.field("email", &self.email).required().email();
        let password = label(v, "password");
        v.field("password", &self.password)
            .label(password)
            .required();
    }
}

async fn store_login(
    Extension(settings): Extension<Arc<Settings>>,
    State(state): State<AppState>,
    session: Session,
    htmx: Htmx,
    ClientIp(ip): ClientIp,
    Valid(form): Valid<LoginForm>,
) -> Result<Response> {
    let key = format!(
        "{}|{}",
        form.email.trim().to_lowercase(),
        ip.map(|ip| ip.to_string()).unwrap_or_default()
    );
    let locale = locale(&state);
    let failed = |key: &str, seconds: Option<u64>| {
        let mut errors = Errors::new();
        let seconds = seconds.map(|s| s.to_string()).unwrap_or_default();
        errors.add(
            "email",
            crate::validation::message(locale, key, "", &[("seconds", seconds)]),
        );
        ValidationError::new(errors)
            .with_input(&json!({ "email": form.email, "remember": form.remember }))
    };

    if let Some(seconds) = state.throttle.blocked_for(&key) {
        return Err(failed("auth.throttle", Some(seconds)).into());
    }

    let user = User::find_by_email(&state.db, &form.email).await?;
    // Hash even when the email is unknown, so timing doesn't reveal which emails exist.
    let hash = user
        .as_ref()
        .map_or_else(dummy_hash, |u| u.password.clone());
    let valid = verify_password(&form.password, &hash).await;
    let Some(user) = user.filter(|_| valid) else {
        state.throttle.fail(&key);
        return Err(failed("auth.failed", None).into());
    };

    state.throttle.clear(&key);
    let remember = form
        .remember
        .is_some()
        .then_some(state.config.remember_lifetime);
    login(&session, &user, remember)?;
    Ok(go(&htmx, after_login(&state, &settings, &session)))
}

async fn show_register(State(state): State<AppState>) -> View {
    view(
        "renox/auth/register.html",
        context! { text => text(locale(&state)) },
    )
}

#[derive(Deserialize)]
struct RegisterForm {
    name: String,
    email: String,
    password: String,
    password_confirmation: Option<String>,
}

impl Validate for RegisterForm {
    fn rules(&self, v: &mut Validator) {
        let (name, password) = (label(v, "name"), label(v, "password"));
        v.field("name", &self.name).label(name).required().max(255);
        v.field("email", &self.email)
            .required()
            .email()
            .max(255)
            .unique("users", "email");
        v.field("password", &self.password)
            .label(password)
            .required()
            .min(8)
            .confirmed(&self.password_confirmation);
    }
}

async fn store_register(
    Extension(settings): Extension<Arc<Settings>>,
    State(state): State<AppState>,
    session: Session,
    htmx: Htmx,
    Valid(form): Valid<RegisterForm>,
) -> Result<Response> {
    let user = User::register(&state.db, &form.name, &form.email, &form.password).await?;
    login(&session, &user, None)?;
    Ok(go(&htmx, after_login(&state, &settings, &session)))
}

async fn destroy(State(state): State<AppState>, session: Session, htmx: Htmx) -> Response {
    logout(&session);
    go(&htmx, state.url("home", &[]).unwrap_or_else(|_| "/".into()))
}

/// Words on the built-in pages.
fn text(locale: Locale) -> Value {
    match locale {
        Locale::En => json!({
            "login_title": "Log in",
            "register_title": "Create an account",
            "name": "Name",
            "email": "Email",
            "password": "Password",
            "password_confirmation": "Confirm password",
            "remember": "Remember me",
            "login_button": "Log in",
            "register_button": "Register",
            "no_account": "No account yet?",
            "have_account": "Already registered?",
        }),
        Locale::Id => json!({
            "login_title": "Masuk",
            "register_title": "Buat akun",
            "name": "Nama",
            "email": "Email",
            "password": "Kata sandi",
            "password_confirmation": "Ulangi kata sandi",
            "remember": "Ingat saya",
            "login_button": "Masuk",
            "register_button": "Daftar",
            "no_account": "Belum punya akun?",
            "have_account": "Sudah punya akun?",
        }),
    }
}
