//! Example: every kind of form field, from the browser to the database and
//! back into the edit form, on SQLite and PostgreSQL. See the table in
//! docs/types.md in the Renox repository.

use renox::prelude::*;

mod app;

pub use app::products::{Product, Size};

pub fn app() -> App {
    App::new()
        .embed(renox::embedded!())
        .migrations(renox::migrations!())
        // The rich text, Markdown and code editors (renox-editors).
        .module(renox_editors::Editors::new())
        .module(app::products::Products)
        // `cargo run -- db:seed`: two products to open and edit.
        .seeder(|state| async move {
            let db = state.db.clone();
            if Product::query().exists(&db).await? {
                return Ok(()); // seeded already
            }
            for (name, price, stock, size) in [
                ("Highland Coffee", 85_000, 12, Size::Medium),
                ("Jasmine Tea", 25_000, 40, Size::Small),
            ] {
                let product = Product {
                    name: name.into(),
                    price,
                    stock,
                    size,
                    available: true,
                    weight_kg: 0.25,
                    colors: renox::db::Json(vec!["black".into()]),
                    released_on: renox::chrono::NaiveDate::from_ymd_opt(2026, 1, 15),
                    ..Default::default()
                };
                Product::create(&db, product).await?;
            }
            Ok(())
        })
}
