use std::time::Duration;

use axum::extract::{Path, State};
use axum::response::{IntoResponse, Response};

use super::module::{go, locale, text};
use super::tokens::sha256_hex;
use super::{AuthUser, User};
use crate::crypto::constant_time_eq;
use crate::db::{Model, now};
use crate::mail::Mail;
use crate::signed::ValidSignature;
use crate::{AppState, Error, Htmx, Result, Session, context, view};

/// How long a verification link works.
const EXPIRES: Duration = Duration::from_secs(60 * 60);

/// Emails `user` a signed link that marks their address as verified.
pub async fn send_verification(state: &AppState, user: &User) -> Result {
    let text = text(locale(state));
    let link = state.signed_url(
        "verification.verify",
        &[&user.id, &sha256_hex(&user.email)],
        EXPIRES,
    )?;
    let body = text["mail_verify_body"]
        .as_str()
        .unwrap_or_default()
        .replace("{link}", &link);
    let subject = text["mail_verify_subject"].as_str().unwrap_or_default();
    state
        .mailer
        .send(Mail::new(&user.email, subject, body))
        .await
}

fn home(state: &AppState) -> String {
    state.url("home", &[]).unwrap_or_else(|_| "/".into())
}

pub(super) async fn notice(auth: AuthUser, State(state): State<AppState>) -> Response {
    if auth.email_verified_at.is_some() {
        return axum::response::Redirect::to(&home(&state)).into_response();
    }
    view(
        "renox/auth/verify-email.html",
        context! { text => text(locale(&state)) },
    )
    .into_response()
}

pub(super) async fn verify(
    auth: AuthUser,
    _: ValidSignature,
    State(state): State<AppState>,
    session: Session,
    Path((id, hash)): Path<(i64, String)>,
) -> Result<Response> {
    if id != auth.id || !constant_time_eq(&hash, &sha256_hex(&auth.email)) {
        return Err(Error::Forbidden);
    }
    if auth.email_verified_at.is_none() {
        let mut user = auth.user().clone();
        user.email_verified_at = Some(now());
        user.save(&state.db).await?;
    }
    session.flash("status", &text(locale(&state))["verified"])?;
    Ok(axum::response::Redirect::to(&home(&state)).into_response())
}

pub(super) async fn resend(
    auth: AuthUser,
    State(state): State<AppState>,
    session: Session,
    htmx: Htmx,
) -> Result<Response> {
    if auth.email_verified_at.is_some() {
        return Ok(go(&htmx, home(&state)));
    }
    send_verification(&state, auth.user()).await?;
    session.flash("status", &text(locale(&state))["verification_sent"])?;
    Ok(go(&htmx, state.url("verification.notice", &[])?))
}
