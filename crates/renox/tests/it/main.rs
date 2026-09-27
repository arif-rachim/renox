//! Renox's integration tests, compiled into one binary: linking one test
//! executable against sqlx, axum and friends is much faster than linking ten.
//! Fixtures (`tests/migrations*`) are read relative to the crate root.

mod api_foundations;
mod auth;
mod auth_email;
mod background_resilience;
mod data_resilience;
mod database;
mod dx;
mod embed;
mod i18n;
mod infra;
mod mail;
mod method;
#[cfg(feature = "postgres")]
mod postgres;
mod queue;
mod security;
mod seo;
mod testing;
mod types;
mod uploads;
mod validation;
mod web_security;
mod webhook;
