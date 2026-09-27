//! Example: a complete CRUD module. Products are listed with pagination;
//! logged-in users create them, and only their owner may edit or delete
//! them. Deletes are soft: deleted products wait in a trash and can be
//! restored.
//!
//! Run it from this directory: `cargo run -- migrate`, then `cargo run`.

use renox::prelude::*;

mod app;

pub use app::products::model::Product;

pub fn app() -> App {
    App::new()
        .embed(renox::embedded!())
        .migrations(renox::migrations!())
        .module(Auth::new())
        .module(app::products::Products)
        .seeder(|db| async move {
            let owner = User::register(&db, "Demo", "demo@example.com", "password123").await?;
            for _ in 0..25 {
                Product::create(&db, Product::for_owner(&owner)).await?;
            }
            Ok(())
        })
}
