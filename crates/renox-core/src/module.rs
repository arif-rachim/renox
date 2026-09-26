use crate::Routes;

/// A self-contained piece of an application: its routes today, and later its
/// migrations, jobs, policies and views.
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
}
