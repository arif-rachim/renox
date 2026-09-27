//! # Renox
//!
//! A batteries-included web framework for Rust, inspired by Laravel.
//! Axum + HTMX + Alpine.js + SQLite.
//!
//! ```no_run
//! use renox::prelude::*;
//!
//! struct Hello;
//!
//! impl Module for Hello {
//!     fn name(&self) -> &'static str { "hello" }
//!
//!     fn routes(&self) -> Routes {
//!         Routes::new().get("/", home).name("home")
//!     }
//! }
//!
//! async fn home() -> View {
//!     view("home.html", context! { title => "Hello from Renox" })
//! }
//!
//! fn main() -> renox::Result {
//!     App::new().module(Hello).run()
//! }
//! ```

pub use renox_core::*;
pub use renox_macros::{Model, embedded, migrations, test};

pub use axum;
pub use tokio;

pub mod prelude {
    pub use renox_core::auth::{Auth, Can, User};
    pub use renox_core::db::{DateTime, Db, Factory, Model, Page, Paginated};
    pub use renox_core::events::Event;
    pub use renox_core::queue::{Job, JobContext};
    pub use renox_core::{
        App, AppState, Back, Config, Environment, Error, Errors, Htmx, HxRedirect, HxRefresh,
        HxTrigger, Module, Result, Routes, Session, Valid, Validate, ValidationError, Validator,
        View, context, view,
    };
    pub use renox_core::{AuthUser, Lang, Policy, Registry, Upload};
    pub use renox_macros::Model;

    pub use axum::extract::{Form, Json, Path, Query, State};
    pub use axum::http::StatusCode;
    pub use axum::response::{Html, IntoResponse, Redirect, Response};
}

/// Compiles every Rust example in `CHEATSHEET.md` as a doctest, so the
/// cheat-sheet can't drift from the API.
#[cfg(doctest)]
#[doc = include_str!("../../../CHEATSHEET.md")]
pub struct CheatSheet;
