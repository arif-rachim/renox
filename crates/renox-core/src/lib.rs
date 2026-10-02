//! Core of the Renox web framework.
//!
//! Most applications should depend on the `renox` crate instead, which
//! re-exports everything here through `renox::prelude`.
#![recursion_limit = "256"]
// the built-in auth texts are one large `json!`
// Every public item is documented; clippy's `-D warnings` in CI keeps it so.
#![warn(missing_docs)]

pub mod analytics;
mod app;
mod assets;
pub mod audit;
pub mod auth;
pub mod cache;
mod client_ip;
mod clock;
pub mod command;
mod config;
pub mod context;
mod cookies;
mod counters;
mod crypto;
mod csrf;
pub mod db;
mod domain;
mod download;
mod embedded;
mod error;
pub mod events;
pub mod grid;
mod health;
mod htmx;
pub mod http;
pub mod i18n;
mod inspector;
mod live;
pub mod mail;
pub mod maintenance;
mod method;
mod module;
mod path;
pub mod prompt;
mod provided;
pub mod queue;
pub mod rate_limit;
mod redirect;
mod registry;
pub mod report;
mod request_id;
mod routing;
pub mod schedule;
pub mod security;
pub mod select;
pub mod seo;
mod session;
pub mod shell;
pub mod signed;
mod state;
pub mod storage;
pub mod testing;
pub mod timezone;
pub mod toast;
pub mod upload;
pub mod validation;
/// Views: MiniJinja templates, the `View` response and template globals.
pub mod view;
mod view_filters;
mod view_stack;
pub mod webhook;

pub use app::{App, Kernel};
pub use assets::{ALPINE_VERSION, CALLY_VERSION, HTMX_VERSION};
pub use auth::{AuthUser, Policy};
pub use client_ip::{ClientIp, TrustedProxies};
pub use config::{AnalyticsConfig, Config, CspMode, Environment};
pub use cookies::{Cookies, SetCookie};
pub use crypto::{generate_key, random_token};
pub use csrf::{CSRF_FIELD, CSRF_HEADER};
pub use domain::DomainParams;
pub use download::Download;
pub use embedded::Embedded;
pub use error::{Error, Result, abort, abort_if, abort_unless};
pub use htmx::{Back, Htmx, HxPushUrl, HxRedirect, HxRefresh, HxReswap, HxRetarget, HxTrigger};
pub use i18n::Lang;
pub use method::METHOD_FIELD;
pub use module::Module;
pub use path::Path;
pub use provided::Provided;
pub use redirect::RedirectExt;
pub use registry::Registry;
pub use request_id::RequestId;
pub use routing::Resource;
pub use routing::{CurrentRoute, RouteInfo, RouteTable, Routes};
pub use session::Session;
pub use state::AppState;
pub use state::SentNotification;
pub use toast::Toast;
pub use upload::Upload;
pub use validation::{Errors, KeyValues, Valid, Validate, ValidationError, Validator};
pub use view::{View, Views, view};
pub use view_filters::format_number;

pub use minijinja::context;

/// `anyhow`, for errors with context: `renox::anyhow::anyhow!("…")`,
/// `.context("…")`, `Error::permanent(err)`.
pub use anyhow;
pub use chrono;
pub use clap;
#[cfg(feature = "fake")]
pub use fake;
pub use serde;
pub use serde_json;
#[doc(hidden)]
pub use sqlx as __sqlx;
/// CORS configuration for `Routes::cors_layer`.
pub use tower_http::cors;
/// `uuid`, for `Uuid` model keys and fields (the `uuid` feature):
/// `renox::uuid::Uuid`.
#[cfg(feature = "uuid")]
pub use uuid;
