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
            // Factory states: 20 of the owner's products, then 5 costly ones
            // with numbered names (a sequence), each batch in one transaction.
            let owner_id = owner.id;
            Product::factory()
                .count(20)
                .state(move |p: &mut Product| p.user_id = owner_id)
                .create(&db)
                .await?;
            Product::factory()
                .count(5)
                .state(move |p: &mut Product| p.user_id = owner_id)
                .sequence(|i, p| {
                    p.name = format!("Premium {}", i + 1);
                    p.price = 500_000 + 100_000 * i as i64;
                })
                .create(&db)
                .await?;
            Ok(())
        })
}
