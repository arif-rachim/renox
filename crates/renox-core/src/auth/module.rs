use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use axum::extract::{ConnectInfo, Extension, FromRequestParts, State};
use axum::http::request::Parts;
use axum::response::{IntoResponse, Redirect, Response};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::user::dummy_hash;
use super::{User, intended, login, logout, passwords, verification, verify_password};
use crate::db::Migration;
use crate::validation::{Errors, Locale, Valid, Validate, ValidationError, Validator};
use crate::{AppState, Htmx, HxRedirect, Module, Result, Routes, Session, View, context, view};

macro_rules! migration {
    ($name:literal) => {
        Migration {
            name: $name,
            up: include_str!(concat!("../../migrations/auth/", $name, ".up.sql")),
            down: Some(include_str!(concat!(
                "../../migrations/auth/",
                $name,
                ".down.sql"
            ))),
        }
    };
}

const MIGRATIONS: &[Migration] = &[
    migration!("00010101000000_create_users_table"),
    migration!("00010101000001_create_password_reset_tokens_table"),
    migration!("00010101000002_create_personal_access_tokens_table"),
    migration!("00010101000003_create_notifications_table"),
];

struct Settings {
    registration: bool,
    redirect_to: Option<String>,
    verify_email: bool,
}

/// Login, registration, password reset and email verification pages, and
/// the `users`, `password_reset_tokens` and `personal_access_tokens` tables.
///
/// Routes: `login`, `register`, `logout`, `password.request`,
/// `password.email`, `password.reset`, `password.update`,
/// `verification.notice`, `verification.verify` and `verification.send`.
/// The pages live in `renox/auth/*.html`, inside `renox/auth/layout.html`;
/// create a file with the same name under your views to replace one.
#[derive(Clone)]
pub struct Auth {
    registration: bool,
    redirect_to: Option<String>,
    verify_email: bool,
}

impl Auth {
    pub fn new() -> Self {
        Self {
            registration: true,
            redirect_to: None,
            verify_email: false,
        }
    }

    /// Emails new users a verification link. Guard routes that need a
    /// verified address with `Routes::require_verified()`.
    pub fn verify_email(mut self) -> Self {
        self.verify_email = true;
        self
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
            verify_email: self.verify_email,
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
        let guest = guest
            .get("/forgot-password", passwords::show_forgot)
            .name("password.request")
            .post("/forgot-password", passwords::send_link)
            .name("password.email")
            .get("/reset-password/{token}", passwords::show_reset)
            .name("password.reset")
            .post("/reset-password", passwords::reset)
            .name("password.update")
            .guest_only();
        let verification = Routes::new()
            .get("/verify-email", verification::notice)
            .name("verification.notice")
            .get("/verify-email/{id}/{hash}", verification::verify)
            .name("verification.verify")
            .post("/email/verification-notification", verification::resend)
            .name("verification.send")
            .require_auth();
        guest
            .merge(verification)
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

pub(super) fn locale(state: &AppState) -> Locale {
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
pub(super) fn go(htmx: &Htmx, to: String) -> Response {
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
pub(super) fn label(v: &Validator, field: &'static str) -> &'static str {
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
    if settings.verify_email {
        verification::send_verification(&state, &user).await?;
    }
    login(&session, &user, None)?;
    Ok(go(&htmx, after_login(&state, &settings, &session)))
}

async fn destroy(State(state): State<AppState>, session: Session, htmx: Htmx) -> Response {
    logout(&session);
    go(&htmx, state.url("home", &[]).unwrap_or_else(|_| "/".into()))
}

/// Words on the built-in pages.
pub(super) fn text(locale: Locale) -> Value {
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
            "forgot_link": "Forgot your password?",
            "forgot_title": "Forgot your password?",
            "forgot_intro": "Enter your email and we'll send you a link to choose a new password.",
            "send_link": "Email me a reset link",
            "back_to_login": "Back to log in",
            "reset_title": "Choose a new password",
            "reset_button": "Reset password",
            "reset_link_sent": "If that email has an account, a reset link is on its way.",
            "reset_invalid": "This password reset link is invalid or has expired.",
            "password_reset_done": "Your password has been reset. You can log in now.",
            "verify_title": "Verify your email",
            "verify_intro": "We've emailed you a link to verify your address. Didn't get it?",
            "resend_button": "Send another link",
            "logout": "Log out",
            "verification_sent": "A new verification link has been sent.",
            "verified": "Your email address is verified.",
            "mail_reset_subject": "Reset your password",
            "mail_reset_intro": "You asked to reset your password. Choose a new one with the button below.",
            "mail_reset_outro": "The link works for 60 minutes. If you didn't ask for this, ignore this email.",
            "mail_verify_subject": "Verify your email address",
            "mail_verify_intro": "Please confirm that this is your email address.",
            "mail_verify_outro": "The link works for 60 minutes.",
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
            "forgot_link": "Lupa kata sandi?",
            "forgot_title": "Lupa kata sandi?",
            "forgot_intro": "Masukkan email kamu, kami kirimkan link untuk membuat kata sandi baru.",
            "send_link": "Kirim link atur ulang",
            "back_to_login": "Kembali ke halaman masuk",
            "reset_title": "Buat kata sandi baru",
            "reset_button": "Simpan kata sandi",
            "reset_link_sent": "Kalau email itu terdaftar, link atur ulang sedang dikirim.",
            "reset_invalid": "Link atur ulang kata sandi tidak valid atau sudah kedaluwarsa.",
            "password_reset_done": "Kata sandi sudah diganti. Silakan masuk.",
            "verify_title": "Verifikasi email",
            "verify_intro": "Kami sudah mengirim link verifikasi ke email kamu. Belum menerima?",
            "resend_button": "Kirim ulang link",
            "logout": "Keluar",
            "verification_sent": "Link verifikasi baru sudah dikirim.",
            "verified": "Alamat email kamu sudah terverifikasi.",
            "mail_reset_subject": "Atur ulang kata sandi",
            "mail_reset_intro": "Kamu meminta atur ulang kata sandi. Buat kata sandi baru lewat tombol di bawah.",
            "mail_reset_outro": "Link berlaku 60 menit. Kalau kamu tidak memintanya, abaikan email ini.",
            "mail_verify_subject": "Verifikasi alamat email",
            "mail_verify_intro": "Silakan konfirmasi bahwa ini alamat email kamu.",
            "mail_verify_outro": "Link berlaku 60 menit.",
        }),
    }
}
