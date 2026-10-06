//! Cart, checkout, payments and counter sales (#234).
//!
//! Made with `rnx make:module sales`, then the files by hand; the models
//! came with #232.

pub mod cart;
pub mod explain;
pub mod factories;
pub mod model;
pub mod payments;

use renox::prelude::*;

/// The sales area, registered in `src/lib.rs`.
pub struct Sales;

impl renox::Module for Sales {
    fn name(&self) -> &'static str {
        "sales"
    }

    fn routes(&self) -> Routes {
        Routes::new()
            .get("/cart", cart::show)
            .name("cart.show")
            .get("/cart/mini", cart::mini)
            .name("cart.mini")
            .post("/cart", cart::add)
            .name("cart.add")
            .patch("/cart/{variant}", cart::update)
            .name("cart.update")
            .delete("/cart/{variant}", cart::remove)
            .name("cart.remove")
            .post("/cart/store", cart::store)
            .name("cart.store")
    }
}
