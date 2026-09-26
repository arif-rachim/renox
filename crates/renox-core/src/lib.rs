//! Core of the Renox web framework.
//!
//! Most applications should depend on the `renox` crate instead, which
//! re-exports everything here through `renox::prelude`.

mod app;
mod assets;
mod config;
mod crypto;
mod csrf;
mod error;
mod htmx;
mod module;
mod routing;
mod session;
mod state;
mod view;

pub use app::App;
pub use assets::{ALPINE_VERSION, HTMX_VERSION};
pub use config::{Config, Environment};
pub use crypto::generate_key;
pub use csrf::{CSRF_FIELD, CSRF_HEADER};
pub use error::{Error, Result};
pub use htmx::{Back, Htmx, HxRedirect, HxRefresh, HxTrigger};
pub use module::Module;
pub use routing::{RouteTable, Routes};
pub use session::Session;
pub use state::AppState;
pub use view::{View, Views, view};

pub use minijinja::context;
