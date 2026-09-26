use axum::Router;

use crate::AppState;

/// A self-contained piece of an application: its routes today, and later its
/// migrations, jobs, policies and views.
///
/// ```ignore
/// pub struct Produk;
///
/// impl Module for Produk {
///     fn name(&self) -> &'static str { "produk" }
///
///     fn routes(&self) -> Router<AppState> {
///         Router::new().route("/produk", get(index))
///     }
/// }
/// ```
pub trait Module: Send + Sync + 'static {
    fn name(&self) -> &'static str;

    fn routes(&self) -> Router<AppState> {
        Router::new()
    }
}
