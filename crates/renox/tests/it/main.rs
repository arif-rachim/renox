//! Renox's integration tests, compiled into one binary: linking one test
//! executable against sqlx, axum and friends is much faster than linking ten.
//! Fixtures (`tests/migrations*`) are read relative to the crate root.

mod accounts;
mod actions;
mod api_foundations;
mod auth;
mod auth_email;
mod authorization;
mod background;
mod background_resilience;
mod commands;
mod dashboards;
mod data_layer;
mod data_resilience;
mod database;
mod derive_validate;
mod direct;
mod dx;
mod embed;
mod extension_points;
mod forms;
mod grid;
mod i18n;
mod infolist;
mod infra;
mod jobs;
mod leftovers;
mod mail;
mod method;
mod model_keys;
mod models;
mod notification_bell;
mod operations;
mod parity;
mod parity_more;
mod polish;
#[cfg(feature = "postgres")]
mod postgres;
mod query_builder;
mod queue;
mod requests;
#[cfg(feature = "s3")]
mod s3;
mod second_factor;
mod secrets;
mod security;
mod send_handlers;
mod seo;
mod services;
mod sessions;
mod testing;
mod testing_tools;
mod tooling;
mod types;
mod ui;
mod uploads;
mod validation;
mod web_security;
mod webhook;
