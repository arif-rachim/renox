//! Relations as plain, typed calls: load the related rows of a whole page in
//! one query instead of one per row (no N+1). See docs/relations.md.
//!
//! ```
//! # use renox::prelude::*;
//! use renox::db::relations::{Pivot, belongs_to, has_many};
//!
//! # #[derive(Model, serde::Serialize, Default, Clone)] struct Category { id: i64, name: String }
//! # #[derive(Model, serde::Serialize, Default, Clone)] struct Product { id: i64, category_id: Option<i64>, name: String }
//! # #[derive(Model, serde::Serialize, Default, Clone)] struct Review { id: i64, product_id: i64, stars: i64 }
//! # #[derive(Model, serde::Serialize, Default, Clone)] struct Tag { id: i64, name: String }
//! const PRODUCT_TAGS: Pivot = Pivot::new("product_tags", "product_id", "tag_id");
//!
//! # async fn demo(db: Db) -> Result {
//! let products = Product::query().order_by("name").limit(20).get(&db).await?;
//! let categories = belongs_to::<Category, _, _>(&db, &products, |p| p.category_id).await?;
//! let reviews = has_many(&db, &products, Review::query().latest(), "product_id", |r| r.product_id).await?;
//! let tags = PRODUCT_TAGS.load_for::<Tag, _>(&db, &products).await?;
//!
//! for product in &products {
//!     let category = product.category_id.and_then(|id| categories.get(&id));
//!     let reviews = reviews.get(&product.id).map_or(&[][..], Vec::as_slice);
//!     let tags = tags.get(&product.id).map_or(&[][..], Vec::as_slice);
//! #   let _ = (category, reviews, tags);
//! }
//! PRODUCT_TAGS.sync(&db, products[0].id, [1, 2]).await?; // the first product's tags are now 1 and 2
//! # Ok(()) }
//! ```

use std::collections::HashMap;

use super::{Db, Executor, Model, Query, quote, sql};
use crate::Result;

/// A foreign key's value: `i64`, or `Option<i64>` for an optional relation.
pub trait ForeignKey {
    fn key(&self) -> Option<i64>;
}

impl ForeignKey for i64 {
    fn key(&self) -> Option<i64> {
        Some(*self)
    }
}

impl ForeignKey for Option<i64> {
    fn key(&self) -> Option<i64> {
        *self
    }
}

/// The parent of each child (`product.category_id` → `Category`), by id, in
/// one query. Children without a parent, or whose parent is gone, have no
/// entry.
//
// Not an `async fn`: the ids are read before the future is made, so the
// future holds no closure or borrowed children. Holding a `Fn(&C)` across an
// `.await` makes the handler's future fail axum's `Send` check (rustc issue
// #100013), and so do the other loaders below.
pub fn belongs_to<'a, P: Model, C, K: ForeignKey>(
    db: &'a Db,
    children: &[C],
    foreign_key: impl Fn(&C) -> K,
) -> impl Future<Output = Result<HashMap<i64, P>>> + Send + 'a {
    let mut ids: Vec<i64> = children
        .iter()
        .filter_map(|c| foreign_key(c).key())
        .collect();
    ids.sort_unstable();
    ids.dedup();
    async move {
        if ids.is_empty() {
            return Ok(HashMap::new());
        }
        Ok(P::find_many(db, ids)
            .await?
            .into_iter()
            .map(|parent| (parent.id(), parent))
            .collect())
    }
}

/// The children of each parent (`order` → its `OrderItem`s), grouped by the
/// parent's id, in one query. `children` is the query to start from, for
/// order and filters (`OrderItem::query()` for all); `column` is the
/// children's foreign key column and `foreign_key` reads it.
pub fn has_many<'a, C: Model, P: Model, K: ForeignKey>(
    db: &'a Db,
    parents: &[P],
    children: Query<C>,
    column: &str,
    foreign_key: impl Fn(&C) -> K + Send + 'a,
) -> impl Future<Output = Result<HashMap<i64, Vec<C>>>> + Send + 'a {
    let ids: Vec<i64> = parents.iter().map(Model::id).collect();
    let query = (!ids.is_empty()).then(|| children.where_in(column, ids));
    async move {
        let mut grouped: HashMap<i64, Vec<C>> = HashMap::new();
        let Some(query) = query else {
            return Ok(grouped);
        };
        let rows = query.get(db).await?;
        for child in rows {
            if let Some(parent) = foreign_key(&child).key() {
                grouped.entry(parent).or_default().push(child);
            }
        }
        Ok(grouped)
    }
}

/// A many-to-many relation through a pivot table with two id columns, e.g.
/// `Pivot::new("product_tags", "product_id", "tag_id")`. Give the table a
/// unique index on both columns.
#[derive(Debug, Clone, Copy)]
pub struct Pivot {
    table: &'static str,
    left: &'static str,
    right: &'static str,
}

impl Pivot {
    pub const fn new(table: &'static str, left: &'static str, right: &'static str) -> Self {
        Self { table, left, right }
    }

    /// The same pivot seen from the other side (`tag_id` → `product_id`).
    pub const fn inverse(self) -> Self {
        Self {
            table: self.table,
            left: self.right,
            right: self.left,
        }
    }

    /// The right-hand ids linked to `left`.
    pub async fn ids<'c>(&self, db: impl Executor<'c>, left: i64) -> Result<Vec<i64>> {
        Ok(sql(format!(
            "SELECT {} FROM {} WHERE {} = ? ORDER BY {}",
            quote(self.right),
            quote(self.table),
            quote(self.left),
            quote(self.right)
        ))
        .bind(left)
        .scalars(db)
        .await?)
    }

    /// Links `left` to each of `rights` that isn't linked yet; returns how
    /// many links were added.
    pub async fn attach<'c>(
        &self,
        db: impl Executor<'c>,
        left: i64,
        rights: impl IntoIterator<Item = i64>,
    ) -> Result<u64> {
        let mut conn = db.into_conn();
        let mut added = 0;
        for right in rights {
            added += sql(format!(
                "INSERT INTO {table} ({l}, {r}) SELECT ?, ? \
                 WHERE NOT EXISTS (SELECT 1 FROM {table} WHERE {l} = ? AND {r} = ?)",
                table = quote(self.table),
                l = quote(self.left),
                r = quote(self.right),
            ))
            .bind(left)
            .bind(right)
            .bind(left)
            .bind(right)
            .execute(conn.reborrow())
            .await?;
        }
        Ok(added)
    }

    /// Unlinks `left` from `rights`; returns how many links were removed.
    pub async fn detach<'c>(
        &self,
        db: impl Executor<'c>,
        left: i64,
        rights: impl IntoIterator<Item = i64>,
    ) -> Result<u64> {
        let rights: Vec<i64> = rights.into_iter().collect();
        if rights.is_empty() {
            return Ok(0);
        }
        let marks = vec!["?"; rights.len()].join(", ");
        Ok(sql(format!(
            "DELETE FROM {} WHERE {} = ? AND {} IN ({marks})",
            quote(self.table),
            quote(self.left),
            quote(self.right)
        ))
        .bind(left)
        .bind_all(rights.into_iter().map(super::DbValue::Integer))
        .execute(db)
        .await?)
    }

    /// Makes `left` linked to exactly `rights` (e.g. the checked boxes of a
    /// form), in one transaction.
    pub async fn sync(&self, db: &Db, left: i64, rights: impl IntoIterator<Item = i64>) -> Result {
        let wanted: Vec<i64> = rights.into_iter().collect();
        let mut tx = db.begin().await?;
        let current = self.ids(&mut tx, left).await?;
        let gone: Vec<i64> = current
            .iter()
            .copied()
            .filter(|id| !wanted.contains(id))
            .collect();
        self.detach(&mut tx, left, gone).await?;
        self.attach(&mut tx, left, wanted).await?;
        tx.commit().await?;
        Ok(())
    }

    /// The right-hand models linked to each left id, in one query per table.
    pub fn load<'a, T: Model + Clone>(
        &'a self,
        db: &'a Db,
        lefts: impl IntoIterator<Item = i64>,
    ) -> impl Future<Output = Result<HashMap<i64, Vec<T>>>> + Send + 'a {
        let lefts: Vec<i64> = lefts.into_iter().collect();
        self.load_ids(db, lefts)
    }

    async fn load_ids<T: Model + Clone>(
        &self,
        db: &Db,
        lefts: Vec<i64>,
    ) -> Result<HashMap<i64, Vec<T>>> {
        let mut grouped: HashMap<i64, Vec<T>> = HashMap::new();
        if lefts.is_empty() {
            return Ok(grouped);
        }
        let marks = vec!["?"; lefts.len()].join(", ");
        let links: Vec<(i64, i64)> = sql(format!(
            "SELECT {}, {} FROM {} WHERE {} IN ({marks})",
            quote(self.left),
            quote(self.right),
            quote(self.table),
            quote(self.left)
        ))
        .bind_all(lefts.into_iter().map(super::DbValue::Integer))
        .fetch_as(db)
        .await?;
        let mut rights: Vec<i64> = links.iter().map(|(_, right)| *right).collect();
        rights.sort_unstable();
        rights.dedup();
        let models: HashMap<i64, T> = T::find_many(db, rights)
            .await?
            .into_iter()
            .map(|model| (model.id(), model))
            .collect();
        for (left, right) in links {
            if let Some(model) = models.get(&right) {
                grouped.entry(left).or_default().push(model.clone());
            }
        }
        Ok(grouped)
    }

    /// `load` for these parents' ids.
    pub fn load_for<'a, T: Model + Clone, P: Model>(
        &'a self,
        db: &'a Db,
        parents: &[P],
    ) -> impl Future<Output = Result<HashMap<i64, Vec<T>>>> + Send + 'a {
        let ids: Vec<i64> = parents.iter().map(Model::id).collect();
        self.load_ids(db, ids)
    }
}
