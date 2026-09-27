//! Renox's integration tests, compiled into one binary: linking one test
//! executable against sqlx, axum and friends is much faster than linking ten.
//! Fixtures (`tests/migrations*`) are read relative to the crate root.

mod auth;
mod auth_email;
mod database;
mod dx;
mod embed;
mod i18n;
mod infra;
mod mail;
mod queue;
mod testing;
mod uploads;
mod validation;
