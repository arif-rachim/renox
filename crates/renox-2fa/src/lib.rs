//! Two-factor authentication for Renox apps: a code from an authenticator
//! app (TOTP, RFC 6238) after the password, and one-time recovery codes.
//!
//! ```
//! use renox::prelude::*;
//! use renox_2fa::TwoFactor;
//!
//! # let _ =
//! App::new()
//!     .module(Auth::new().account()) // the account page, where users turn it on
//!     .module(TwoFactor::new())
//! # ;
//! ```
//!
//! This crate is being built in steps (#146): the table, the codes and the QR
//! code are here; turning it on from the account page, the login challenge
//! and recovery codes follow.

use renox::db::Migration;
use renox::prelude::*;

mod model;
pub mod qr;
pub mod totp;

pub use model::TwoFactorCredential;

const MIGRATIONS: &[Migration] = &[Migration::new(
    "00010101000700_create_two_factor_table",
    "",
    Some(include_str!(
        "../migrations/00010101000700_create_two_factor_table.down.sql"
    )),
)
.sqlite(
    include_str!("../migrations/00010101000700_create_two_factor_table.up.sql"),
    None,
)
.postgres(
    include_str!("../migrations/00010101000700_create_two_factor_table.postgres.up.sql"),
    None,
)];

/// The two-factor authentication module: add it next to `Auth`.
#[derive(Debug, Clone, Default)]
pub struct TwoFactor {}

impl TwoFactor {
    /// The module with its defaults.
    pub fn new() -> Self {
        Self::default()
    }
}

impl Module for TwoFactor {
    fn name(&self) -> &'static str {
        "two-factor"
    }

    fn migrations(&self) -> &'static [Migration] {
        MIGRATIONS
    }
}
