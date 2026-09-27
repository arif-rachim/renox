//! Core of the Renox web framework.
//!
//! Most applications should depend on the `renox` crate instead, which
//! re-exports everything here through `renox::prelude`.

mod app;
mod assets;
pub mod auth;
pub mod cache;
mod config;
mod crypto;
mod csrf;
pub mod db;
mod error;
pub mod events;
mod health;
mod htmx;
pub mod mail;
pub mod maintenance;
mod module;
pub mod queue;
mod rate_limit;
mod registry;
mod routing;
pub mod schedule;
mod session;
pub mod signed;
mod state;
pub mod storage;
pub mod upload;
pub mod validation;
mod view;

pub use app::{App, Kernel};
pub use assets::{ALPINE_VERSION, HTMX_VERSION};
pub use auth::{AuthUser, Policy};
pub use config::{Config, Environment};
pub use crypto::generate_key;
pub use csrf::{CSRF_FIELD, CSRF_HEADER};
pub use error::{Error, Result};
pub use htmx::{Back, Htmx, HxRedirect, HxRefresh, HxTrigger};
pub use module::Module;
pub use registry::Registry;
pub use routing::{RouteTable, Routes};
pub use session::Session;
pub use state::AppState;
pub use upload::Upload;
pub use validation::{Errors, Valid, Validate, ValidationError, Validator};
pub use view::{View, Views, view};

pub use minijinja::context;

pub use chrono;
pub use fake;
pub use sqlx;
