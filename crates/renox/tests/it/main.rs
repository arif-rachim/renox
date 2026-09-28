//! Renox's integration tests, compiled into one binary: linking one test
//! executable against sqlx, axum and friends is much faster than linking ten.
//! Fixtures (`tests/migrations*`) are read relative to the crate root.

mod accounts;
mod api_foundations;
mod auth;
mod auth_email;
mod authorization;
mod background_resilience;
mod data_layer;
mod data_resilience;
mod database;
mod direct;
mod dx;
mod embed;
mod extension_points;
mod i18n;
mod infra;
mod mail;
mod method;
mod models;
#[cfg(feature = "postgres")]
mod postgres;
mod query_builder;
mod queue;
mod requests;
#[cfg(feature = "s3")]
mod s3;
mod security;
mod send_handlers;
mod seo;
mod testing;
mod types;
mod uploads;
mod validation;
mod web_security;
mod webhook;
