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

    /// A builder for models with states and sequences (Laravel's
    /// `Product::factory()->count(3)->state(…)->create()`):
    ///
    /// ```
    /// # use renox::prelude::*;
    /// # #[derive(Model, serde::Serialize, Default)] struct Produk { id: i64, nama: String, stok: i64 }
    /// # impl Factory for Produk { fn definition() -> Self { Produk { nama: "Kopi".into(), stok: 10, ..Default::default() } } }
    /// /// A state: a plain function (or closure) that changes the model.
    /// fn sold_out(p: &mut Produk) {
    ///     p.stok = 0;
    /// }
    ///
    /// # async fn demo(db: Db) -> Result {
    /// let gone = Produk::factory().count(3).state(sold_out).create(&db).await?;
    /// let named = Produk::factory()
    ///     .count(2)
    ///     .sequence(|i, p| p.nama = format!("Kopi {}", i + 1)) // "Kopi 1", "Kopi 2"
    ///     .make(); // unsaved
    /// # let _ = (gone, named); Ok(()) }
    /// ```
    fn factory() -> FactoryBuilder<Self> {
        FactoryBuilder {
            count: 1,
            steps: Vec::new(),
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

type Step<M> = Box<dyn FnMut(usize, &mut M) + Send>;

/// Models from [`Factory::definition`] with states and sequences applied,
/// from [`Factory::factory`].
#[must_use = "a factory makes nothing until `make` or `create`"]
pub struct FactoryBuilder<M> {
    count: usize,
    steps: Vec<Step<M>>,
}

impl<M: Factory> FactoryBuilder<M> {
    /// How many models to make (1 by default).
    pub fn count(mut self, count: usize) -> Self {
        self.count = count;
        self
    }

    /// Changes every model, after the definition and the steps before it.
    pub fn state(mut self, mut state: impl FnMut(&mut M) + Send + 'static) -> Self {
        self.steps.push(Box::new(move |_, model| state(model)));
        self
    }

    /// Changes each model knowing its position (0, 1, …), e.g. to cycle
    /// through values: `.sequence(|i, p| p.size = ["S", "M", "L"][i % 3].into())`.
    pub fn sequence(mut self, step: impl FnMut(usize, &mut M) + Send + 'static) -> Self {
        self.steps.push(Box::new(step));
        self
    }

    /// The models, unsaved.
    pub fn make(mut self) -> Vec<M> {
        (0..self.count)
            .map(|i| {
                let mut model = M::definition();
                for step in &mut self.steps {
                    step(i, &mut model);
                }
                model
            })
            .collect()
    }

    /// One model, unsaved (the first of `count`).
    pub fn make_one(self) -> M {
        self.count(1).make().pop().expect("one model")
    }

    /// The models, inserted in one transaction.
    pub fn create(self, db: &Db) -> impl Future<Output = Result<Vec<M>>> + Send + '_ {
        // Made before the future, so it holds no closures across `.await`.
        let models = self.make();
        async move {
            let mut tx = db.begin().await?;
            let mut saved = Vec::with_capacity(models.len());
            for mut model in models {
                model.insert(&mut tx).await?;
                saved.push(model);
            }
            tx.commit().await?;
            Ok(saved)
        }
    }

    /// One model, inserted.
    pub fn create_one(self, db: &Db) -> impl Future<Output = Result<M>> + Send + '_ {
        let mut model = self.make_one();
        async move {
            model.insert(db).await?;
            Ok(model)
        }
    }
}
