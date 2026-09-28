//! Core of the Renox web framework.
//!
//! Most applications should depend on the `renox` crate instead, which
//! re-exports everything here through `renox::prelude`.

pub mod analytics;
mod app;
mod assets;
pub mod auth;
pub mod cache;
mod client_ip;
pub mod command;
mod config;
pub mod context;
mod cookies;
mod counters;
mod crypto;
mod csrf;
pub mod db;
mod download;
mod embedded;
mod error;
pub mod events;
mod health;
mod htmx;
pub mod i18n;
mod live;
pub mod mail;
pub mod maintenance;
mod method;
mod module;
mod provided;
pub mod queue;
mod rate_limit;
mod registry;
mod routing;
pub mod schedule;
pub mod security;
pub mod seo;
mod session;
pub mod shell;
pub mod signed;
mod state;
pub mod storage;
pub mod testing;
pub mod upload;
pub mod validation;
pub mod view;
mod view_filters;
pub mod webhook;

pub use app::{App, Kernel};
pub use assets::{ALPINE_VERSION, HTMX_VERSION};
pub use auth::{AuthUser, Policy};
pub use client_ip::{ClientIp, TrustedProxies};
pub use config::{AnalyticsConfig, Config, CspMode, Environment};
pub use cookies::{Cookies, SetCookie};
pub use crypto::generate_key;
pub use csrf::{CSRF_FIELD, CSRF_HEADER};
pub use download::Download;
pub use embedded::Embedded;
pub use error::{Error, Result, abort, abort_if, abort_unless};
pub use htmx::{Back, Htmx, HxRedirect, HxRefresh, HxTrigger};
pub use i18n::Lang;
pub use method::METHOD_FIELD;
pub use module::Module;
pub use provided::Provided;
pub use registry::Registry;
pub use routing::{RouteInfo, RouteTable, Routes};
pub use session::Session;
pub use state::AppState;
pub use upload::Upload;
pub use validation::{Errors, Valid, Validate, ValidationError, Validator};
pub use view::{View, Views, view};
pub use view_filters::format_number;

pub use minijinja::context;

pub use chrono;
#[cfg(feature = "fake")]
pub use fake;
pub use serde;
pub use serde_json;
#[doc(hidden)]
pub use sqlx as __sqlx;
/// CORS configuration for `Routes::cors_layer`.
pub use tower_http::cors;
