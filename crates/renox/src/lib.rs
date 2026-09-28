//! # Renox
//!
//! A batteries-included web framework for Rust, inspired by Laravel.
//! Axum + HTMX + Alpine.js + SQLite or PostgreSQL.
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
pub use renox_macros::{DbEnum, FromRow, Model, embedded, migrations, test};

pub use axum;
pub use tokio;

pub mod prelude {
    pub use renox_core::auth::{Auth, Can, User};
    pub use renox_core::db::{DateTime, Db, Factory, FromRow, Model, Page, Paginated};
    pub use renox_core::events::Event;
    pub use renox_core::queue::{Job, JobContext};
    pub use renox_core::serde_json::json;
    pub use renox_core::webhook::{Webhook, WebhookCall, WebhookRequest};
    pub use renox_core::{
        App, AppState, Back, Config, Environment, Error, Errors, Htmx, HxRedirect, HxRefresh,
        HxTrigger, Module, Result, Routes, Session, Valid, Validate, ValidationError, Validator,
        View, context, view,
    };
    pub use renox_core::{AuthUser, ClientIp, Lang, Policy, Registry, Upload};
    pub use renox_core::{abort, abort_if, abort_unless};
    pub use renox_macros::{DbEnum, FromRow, Model};

    pub use axum::extract::{Form, Json, Path, Query, State};
    pub use axum::http::StatusCode;
    pub use axum::response::{Html, IntoResponse, Redirect, Response};
}

/// Compiles the Rust in docs/types.md as a doctest.
#[cfg(doctest)]
#[doc = include_str!("../../../docs/types.md")]
pub struct TypesGuide;

/// Compiles the Rust in docs/relations.md as a doctest.
#[cfg(doctest)]
#[doc = include_str!("../../../docs/relations.md")]
pub struct RelationsGuide;

/// Compiles the Rust in docs/authorization.md as a doctest.
#[cfg(doctest)]
#[doc = include_str!("../../../docs/authorization.md")]
pub struct AuthorizationGuide;

/// Compiles the Rust in docs/queue.md as a doctest.
#[cfg(doctest)]
#[doc = include_str!("../../../docs/queue.md")]
pub struct QueueGuide;

/// Compiles every Rust example in the README as a doctest, so the front page
/// can't drift from the API.
#[cfg(doctest)]
#[doc = include_str!("../../../README.md")]
pub struct ReadMe;

/// Compiles every Rust example in `CHEATSHEET.md` as a doctest, so the
/// cheat-sheet can't drift from the API.
#[cfg(doctest)]
#[doc = include_str!("../../../CHEATSHEET.md")]
pub struct CheatSheet;

/// Mistakes the derives turn into compile errors (checked as doctests).
///
/// A model needs an `id`:
/// ```compile_fail
/// # use renox::prelude::*;
/// #[derive(Model)]
/// struct NoId { name: String }
/// ```
///
/// `soft_deletes` needs `deleted_at`:
/// ```compile_fail
/// # use renox::prelude::*;
/// #[derive(Model)]
/// #[model(soft_deletes)]
/// struct NoDeletedAt { id: i64 }
/// ```
///
/// Unknown attributes are refused:
/// ```compile_fail
/// # use renox::prelude::*;
/// #[derive(Model)]
/// #[model(tabel = "typo")]
/// struct Typo { id: i64 }
/// ```
///
/// Generic structs can't be models:
/// ```compile_fail
/// # use renox::prelude::*;
/// #[derive(Model)]
/// struct Generic<T> { id: i64, value: T }
/// ```
///
/// `DbEnum` is for fieldless enums:
/// ```compile_fail
/// # use renox::prelude::*;
/// #[derive(DbEnum)]
/// enum WithData { A(i64) }
/// ```
///
/// `FromRow` needs named fields:
/// ```compile_fail
/// # use renox::prelude::*;
/// #[derive(FromRow)]
/// struct Tuple(i64);
/// ```
///
/// A field type that can't come from a column doesn't compile either:
/// ```compile_fail
/// # use renox::prelude::*;
/// struct NotAColumn;
/// #[derive(FromRow)]
/// struct Row { value: NotAColumn }
/// ```
#[cfg(doctest)]
pub struct MacroCompileErrors;
