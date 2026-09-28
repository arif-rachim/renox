//! Events the built-in auth pages emit, e.g. for a welcome mail or an audit
//! log (`renox::audit` records them all). Listen like to any event:
//!
//! ```
//! # use renox::prelude::*;
//! use renox::auth::events::{LoggedIn, Registered};
//!
//! # let _ =
//! App::new()
//!     .module(Auth::new())
//!     .listen(|e: Registered, state| async move {
//!         tracing::info!(user = e.user_id, "welcome!"); // e.g. queue a welcome mail
//!         let _ = state;
//!         Ok(())
//!     })
//!     .listen(|e: LoggedIn, _state| async move {
//!         tracing::info!(user = e.user_id, ip = ?e.ip, "logged in");
//!         Ok(())
//!     })
//! # ;
//! ```
//!
//! A listener's error is logged; it never fails the login itself. Your own
//! login code (`auth::login`) emits nothing: emit these yourself if you need
//! them.

use crate::AppState;
use crate::events::Event;

/// A new account, after `on_registered` succeeded.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Registered {
    pub user_id: i64,
    pub email: String,
}

/// A successful login on the login page.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct LoggedIn {
    pub user_id: i64,
    pub ip: Option<String>,
}

/// A wrong email or password on the login page.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct LoginFailed {
    pub email: String,
    pub ip: Option<String>,
}

/// Too many failed logins: this email (or address) must wait.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct LockedOut {
    pub email: String,
    pub ip: Option<String>,
    pub seconds: u64,
}

/// A logout of one device.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct LoggedOut {
    pub user_id: i64,
}

/// A password chosen through a reset link.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct PasswordReset {
    pub user_id: i64,
}

/// A password changed on the account page.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct PasswordChanged {
    pub user_id: i64,
}

/// An email address confirmed through its link.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct EmailVerified {
    pub user_id: i64,
}

/// Name or email changed on the account page.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct ProfileUpdated {
    pub user_id: i64,
    pub email_changed: bool,
}

/// "Log out other devices" on the account page.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct OtherDevicesLoggedOut {
    pub user_id: i64,
}

/// An account deleted on the account page (the row is already gone).
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct AccountDeleted {
    pub user_id: i64,
    pub email: String,
}

impl Event for Registered {}
impl Event for LoggedIn {}
impl Event for LoginFailed {}
impl Event for LockedOut {}
impl Event for LoggedOut {}
impl Event for PasswordReset {}
impl Event for PasswordChanged {}
impl Event for EmailVerified {}
impl Event for ProfileUpdated {}
impl Event for OtherDevicesLoggedOut {}
impl Event for AccountDeleted {}

/// Emits `event`; a listener's failure is logged, not returned.
pub(crate) async fn announce<E: Event>(state: &AppState, event: E) {
    if let Err(err) = state.emit(event).await {
        tracing::error!(event = std::any::type_name::<E>(), error = ?err, "auth listener failed");
    }
}
