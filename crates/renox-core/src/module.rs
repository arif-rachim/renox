use crate::db::Migration;
use crate::{Registry, Routes};

/// A self-contained piece of an application: its routes and migrations, and
/// later its jobs, policies and views.
///
/// ```ignore
/// pub struct Produk;
///
/// impl Module for Produk {
///     fn name(&self) -> &'static str { "produk" }
///
///     fn routes(&self) -> Routes {
///         Routes::new()
///             .get("/produk", index).name("produk.index")
///             .get("/produk/{id}", show).name("produk.show")
///     }
/// }
/// ```
pub trait Module: Send + Sync + 'static {
    fn name(&self) -> &'static str;

    fn routes(&self) -> Routes {
        Routes::new()
    }

    /// Migrations this module owns, e.g. `renox::migrations!("src/app/produk/migrations")`.
    fn migrations(&self) -> &'static [Migration] {
        &[]
    }

    /// Registers the module's jobs, event listeners and scheduled tasks.
    ///
    /// ```ignore
    /// fn register(&self, app: &mut Registry) {
    ///     app.job::<SendReceipt>()
    ///         .listen(|e: OrderPlaced, state| async move { ... });
    ///     app.schedule().daily_at("02:00", "close-day", close_day);
    /// }
    /// ```
    fn register(&self, _app: &mut Registry) {}
}
