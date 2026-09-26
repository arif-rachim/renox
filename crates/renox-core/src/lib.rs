//! Core of the Renox web framework.
//!
//! Most applications should depend on the `renox` crate instead, which
//! re-exports everything here through `renox::prelude`.

mod app;
mod config;
mod error;
mod module;
mod state;

pub use app::App;
pub use config::{Config, Environment};
pub use error::{Error, Result};
pub use module::Module;
pub use state::AppState;
