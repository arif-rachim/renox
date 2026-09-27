use std::future::Future;

use super::{Db, Model};
use crate::Result;

/// Builds models with fake data for seeders and tests.
///
/// ```
/// # use renox::prelude::*;
/// # #[derive(Model, serde::Serialize, Default)] struct Produk { id: i64, nama: String, harga: i64 }
/// use renox::fake::{Fake, faker::lorem::en::Word};
///
/// impl Factory for Produk {
///     fn definition() -> Self {
///         Produk { nama: Word().fake(), harga: (5_000..50_000).fake(), ..Default::default() }
///     }
/// }
///
/// # async fn demo(db: Db) -> Result {
/// Produk::create_many(&db, 50).await?;
/// # Ok(()) }
/// ```
pub trait Factory: Model {
    fn definition() -> Self;

    /// A new, unsaved model.
    fn make() -> Self {
        Self::definition()
    }

    /// A new model, saved.
    fn create_one(db: &Db) -> impl Future<Output = Result<Self>> + Send {
        async move {
            let mut model = Self::definition();
            model.save(db).await?;
            Ok(model)
        }
    }

    /// `count` new models, saved in one transaction.
    fn create_many(db: &Db, count: usize) -> impl Future<Output = Result<Vec<Self>>> + Send {
        async move {
            let mut tx = db.begin().await?;
            let mut models = Vec::with_capacity(count);
            for _ in 0..count {
                let mut model = Self::definition();
                model.save(&mut tx).await?;
                models.push(model);
            }
            tx.commit().await?;
            Ok(models)
        }
    }
}
