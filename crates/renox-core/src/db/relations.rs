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

/// How many children each parent has (`withCount`), in one `GROUP BY`
/// query; parents without children get 0. `children` sets the filters.
///
/// ```
/// # use renox::prelude::*;
/// # use renox::db::relations::{count_many, sum_many};
/// # #[derive(Model, serde::Serialize, Default)] struct Post { id: i64, title: String }
/// # #[derive(Model, serde::Serialize, Default)] struct Comment { id: i64, post_id: i64, approved: bool }
/// # #[derive(Model, serde::Serialize, Default)] struct Order { id: i64, user_id: i64, total: i64 }
/// # async fn demo(db: Db, posts: Vec<Post>, users: Vec<User>) -> Result {
/// let comments = count_many(&db, &posts, Comment::where_eq("approved", true), "post_id").await?;
/// let spent = sum_many::<i64, _, _>(&db, &users, Order::query(), "user_id", "total").await?;
/// let n = comments[&posts[0].id]; // 0 when a post has none
/// # let _ = (n, spent); Ok(()) }
/// ```
pub fn count_many<'a, C: Model, P: Model>(
    db: &'a Db,
    parents: &[P],
    children: Query<C>,
    column: &str,
) -> impl Future<Output = Result<HashMap<i64, i64>>> + Send + 'a {
    grouped(db, parents, children, column, "COUNT(*)".to_owned())
}

/// The sum of `sum_column` over each parent's children (`withSum`), 0 for
/// parents without children; `T` is `i64` or `f64`.
pub fn sum_many<'a, T: super::Number + Default + Send + 'a, C: Model, P: Model>(
    db: &'a Db,
    parents: &[P],
    children: Query<C>,
    column: &str,
    sum_column: &str,
) -> impl Future<Output = Result<HashMap<i64, T>>> + Send + 'a {
    let expression = if C::COLUMNS.contains(&sum_column) {
        format!(
            "CAST(COALESCE(SUM({}), 0) AS {})",
            quote(sum_column),
            T::SQL_TYPE
        )
    } else {
        // Let the query report the unknown column.
        format!("SUM({})", quote(sum_column))
    };
    let checked = children.check_column(sum_column);
    grouped(db, parents, checked, column, expression)
}

fn grouped<'a, T: crate::db::FromDb + Default + Send + 'a, C: Model, P: Model>(
    db: &'a Db,
    parents: &[P],
    children: Query<C>,
    column: &str,
    expression: String,
) -> impl Future<Output = Result<HashMap<i64, T>>> + Send + 'a {
    let ids: Vec<i64> = parents.iter().map(Model::id).collect();
    let column = column.to_owned();
    async move {
        let mut totals: HashMap<i64, T> = ids.iter().map(|id| (*id, T::default())).collect();
        if ids.is_empty() {
            return Ok(totals);
        }
        let rows: Vec<(i64, T)> = children
            .where_in(&column, ids)
            .group_by(&column)
            .select_as(db, &format!("{}, {expression}", quote(&column)))
            .await?;
        totals.extend(rows);
        Ok(totals)
    }
}

/// A many-to-many relation through a pivot table with two id columns, e.g.
/// `Pivot::new("product_tags", "product_id", "tag_id")`. Give the table a
/// unique index on both columns. The table may have more columns (a role, a
/// quantity): set them with [`Pivot::attach_with`] and
/// [`Pivot::update_pivot`], read them with [`Pivot::load_with_pivot`].
#[derive(Debug, Clone, Copy)]
pub struct Pivot {
    table: &'static str,
    left: &'static str,
    right: &'static str,
    timestamps: bool,
}

/// Extra pivot columns and their values, e.g. `&[("role", &"admin")]`.
pub type PivotData<'a> = &'a [(&'a str, &'a (dyn super::ToDbValue + Sync))];

impl Pivot {
    pub const fn new(table: &'static str, left: &'static str, right: &'static str) -> Self {
        Self {
            table,
            left,
            right,
            timestamps: false,
        }
    }

    /// The pivot table has `created_at` and `updated_at` columns, which
    /// attaching and [`Pivot::update_pivot`] fill.
    pub const fn with_timestamps(mut self) -> Self {
        self.timestamps = true;
        self
    }

    /// The same pivot seen from the other side (`tag_id` → `product_id`).
    pub const fn inverse(self) -> Self {
        Self {
            table: self.table,
            left: self.right,
            right: self.left,
            timestamps: self.timestamps,
        }
    }

    /// Links `left` to `right` unless they're linked already; returns
    /// whether a link was added.
    async fn insert_link(
        &self,
        conn: &mut super::Conn<'_>,
        left: i64,
        right: i64,
        data: PivotData<'_>,
    ) -> Result<bool> {
        let mut columns = vec![quote(self.left), quote(self.right)];
        let mut values = vec![
            super::DbValue::Integer(left),
            super::DbValue::Integer(right),
        ];
        for (column, value) in data {
            columns.push(quote(column));
            values.push(value.to_db_value());
        }
        if self.timestamps {
            let at = super::ToDbValue::to_db_value(&super::now());
            columns.extend([quote("created_at"), quote("updated_at")]);
            values.extend([at.clone(), at]);
        }
        let marks = vec!["?"; columns.len()].join(", ");
        let added = sql(format!(
            "INSERT INTO {table} ({columns}) SELECT {marks} \
             WHERE NOT EXISTS (SELECT 1 FROM {table} WHERE {l} = ? AND {r} = ?)",
            table = quote(self.table),
            columns = columns.join(", "),
            l = quote(self.left),
            r = quote(self.right),
        ))
        .bind_all(values)
        .bind(left)
        .bind(right)
        .execute(conn.reborrow())
        .await?;
        Ok(added > 0)
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
            added += u64::from(self.insert_link(&mut conn, left, right, &[]).await?);
        }
        Ok(added)
    }

    /// Links `left` to `right` with extra pivot columns, unless they're
    /// linked already (change those with [`Pivot::update_pivot`]); returns
    /// whether a link was added.
    ///
    /// ```
    /// # use renox::prelude::*;
    /// # use renox::db::relations::Pivot;
    /// const MEMBERS: Pivot = Pivot::new("team_user", "team_id", "user_id").with_timestamps();
    /// # async fn demo(db: Db) -> Result {
    /// MEMBERS.attach_with(&db, 1, 7, &[("role", &"admin")]).await?;
    /// MEMBERS.update_pivot(&db, 1, 7, &[("role", &"member")]).await?;
    /// # Ok(()) }
    /// ```
    pub async fn attach_with<'c>(
        &self,
        db: impl Executor<'c>,
        left: i64,
        right: i64,
        data: PivotData<'_>,
    ) -> Result<bool> {
        let mut conn = db.into_conn();
        self.insert_link(&mut conn, left, right, data).await
    }

    /// Changes extra columns of the link between `left` and `right` (and
    /// `updated_at` with timestamps); returns whether they were linked.
    pub async fn update_pivot<'c>(
        &self,
        db: impl Executor<'c>,
        left: i64,
        right: i64,
        data: PivotData<'_>,
    ) -> Result<bool> {
        let mut sets = Vec::new();
        let mut values = Vec::new();
        for (column, value) in data {
            sets.push(format!("{} = ?", quote(column)));
            values.push(value.to_db_value());
        }
        if self.timestamps {
            sets.push(format!("{} = ?", quote("updated_at")));
            values.push(super::ToDbValue::to_db_value(&super::now()));
        }
        if sets.is_empty() {
            return Ok(false);
        }
        let changed = sql(format!(
            "UPDATE {} SET {} WHERE {} = ? AND {} = ?",
            quote(self.table),
            sets.join(", "),
            quote(self.left),
            quote(self.right)
        ))
        .bind_all(values)
        .bind(left)
        .bind(right)
        .execute(db)
        .await?;
        Ok(changed > 0)
    }

    /// Links each of `rights` that isn't linked to `left` and unlinks each
    /// that is, in one transaction; returns `(attached, detached)`.
    pub async fn toggle(
        &self,
        db: &Db,
        left: i64,
        rights: impl IntoIterator<Item = i64>,
    ) -> Result<(Vec<i64>, Vec<i64>)> {
        let mut tx = db.begin().await?;
        let current = self.ids(&mut tx, left).await?;
        let (detach, attach): (Vec<i64>, Vec<i64>) =
            rights.into_iter().partition(|id| current.contains(id));
        self.detach(&mut tx, left, detach.iter().copied()).await?;
        self.attach(&mut tx, left, attach.iter().copied()).await?;
        tx.commit().await?;
        Ok((attach, detach))
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

    /// Like [`Pivot::load`], with each model's pivot row decoded as `D`
    /// (a `#[derive(FromRow)]` struct naming the pivot columns it wants).
    ///
    /// ```
    /// # use renox::prelude::*;
    /// # use renox::db::relations::Pivot;
    /// # const MEMBERS: Pivot = Pivot::new("team_user", "team_id", "user_id");
    /// #[derive(FromRow)]
    /// struct Membership { role: String }
    ///
    /// # async fn demo(db: Db) -> Result {
    /// let members = MEMBERS.load_with_pivot::<User, Membership>(&db, [1]).await?;
    /// for (user, membership) in members.get(&1).map_or(&[][..], Vec::as_slice) {
    ///     println!("{} is {}", user.name, membership.role);
    /// }
    /// # Ok(()) }
    /// ```
    pub fn load_with_pivot<'a, T: Model + Clone, D: super::FromRow + Send + 'a>(
        &'a self,
        db: &'a Db,
        lefts: impl IntoIterator<Item = i64>,
    ) -> impl Future<Output = Result<HashMap<i64, Vec<(T, D)>>>> + Send + 'a {
        let lefts: Vec<i64> = lefts.into_iter().collect();
        async move {
            let mut grouped: HashMap<i64, Vec<(T, D)>> = HashMap::new();
            if lefts.is_empty() {
                return Ok(grouped);
            }
            let marks = vec!["?"; lefts.len()].join(", ");
            let rows = sql(format!(
                "SELECT * FROM {} WHERE {} IN ({marks})",
                quote(self.table),
                quote(self.left)
            ))
            .bind_all(lefts.into_iter().map(super::DbValue::Integer))
            .fetch_all(db)
            .await?;
            let mut links = Vec::with_capacity(rows.len());
            for row in &rows {
                let left: i64 = row.try_get(self.left)?;
                let right: i64 = row.try_get(self.right)?;
                links.push((left, right, D::from_row(row)?));
            }
            let mut rights: Vec<i64> = links.iter().map(|(_, right, _)| *right).collect();
            rights.sort_unstable();
            rights.dedup();
            let models: HashMap<i64, T> = T::find_many(db, rights)
                .await?
                .into_iter()
                .map(|model| (model.id(), model))
                .collect();
            for (left, right, data) in links {
                if let Some(model) = models.get(&right) {
                    grouped.entry(left).or_default().push((model.clone(), data));
                }
            }
            Ok(grouped)
        }
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

/// A polymorphic relation: a child row that belongs to one of several
/// parent tables, through a type column holding the parent's table name and
/// an id column, e.g. comments on both posts and videos:
///
/// ```
/// # use renox::prelude::*;
/// use renox::db::relations::Morph;
///
/// # #[derive(Model, serde::Serialize, Default, Clone)] struct Post { id: i64, title: String }
/// # #[derive(Model, serde::Serialize, Default, Clone)] struct Video { id: i64, title: String }
/// #[derive(Model, serde::Serialize, Default, Clone)]
/// struct Comment { id: i64, commentable_type: String, commentable_id: i64, body: String }
///
/// const COMMENTABLE: Morph = Morph::new("commentable_type", "commentable_id");
///
/// # async fn demo(db: Db, post: Post) -> Result {
/// Comment::create(&db, Comment {
///     commentable_type: Post::TABLE.into(), // "posts"
///     commentable_id: post.id,
///     body: "Nice".into(),
///     ..Default::default()
/// }).await?;
///
/// let posts = Post::query().latest().limit(20).get(&db).await?;
/// let comments = COMMENTABLE.load_many(&db, &posts, Comment::query(), |c| c.commentable_id).await?;
/// let on_this_post = COMMENTABLE.of(&post, Comment::query()).count(&db).await?;
///
/// // The other way: each comment's parent, one query per parent type.
/// let recent = Comment::query().latest().limit(50).get(&db).await?;
/// let parent = |c: &Comment| (c.commentable_type.clone(), c.commentable_id);
/// let post_parents = COMMENTABLE.parents::<Post, _>(&db, &recent, parent).await?;
/// let video_parents = COMMENTABLE.parents::<Video, _>(&db, &recent, parent).await?;
/// # let _ = (comments, on_this_post, post_parents, video_parents); Ok(()) }
/// ```
///
/// There's no foreign key to the parents, so deleting a parent doesn't
/// delete its children: do it in the parent's `deleting` hook
/// ([`ModelHooks`](super::ModelHooks)) or its delete handler.
#[derive(Debug, Clone, Copy)]
pub struct Morph {
    type_column: &'static str,
    id_column: &'static str,
}

impl Morph {
    pub const fn new(type_column: &'static str, id_column: &'static str) -> Self {
        Self {
            type_column,
            id_column,
        }
    }

    /// `children` narrowed to those of `parent`.
    pub fn of<C: Model, P: Model>(&self, parent: &P, children: Query<C>) -> Query<C> {
        children
            .where_eq(self.type_column, P::TABLE)
            .where_eq(self.id_column, parent.id())
    }

    /// The children of each parent, grouped by the parent's id, in one
    /// query (`has_many` for a polymorphic relation). `foreign_key` reads
    /// the id column.
    pub fn load_many<'a, C: Model, P: Model>(
        &self,
        db: &'a Db,
        parents: &[P],
        children: Query<C>,
        foreign_key: impl Fn(&C) -> i64 + Send + 'a,
    ) -> impl Future<Output = Result<HashMap<i64, Vec<C>>>> + Send + 'a {
        let children = children.where_eq(self.type_column, P::TABLE);
        has_many(db, parents, children, self.id_column, foreign_key)
    }

    /// The parents of type `P` of these children, by id, in one query;
    /// `parent` reads a child's type and id columns. Children of other
    /// parent types are skipped: call it once per type.
    pub fn parents<'a, P: Model, C>(
        &self,
        db: &'a Db,
        children: &[C],
        parent: impl Fn(&C) -> (String, i64),
    ) -> impl Future<Output = Result<HashMap<i64, P>>> + Send + 'a {
        let mut ids: Vec<i64> = children
            .iter()
            .filter_map(|child| {
                let (kind, id) = parent(child);
                (kind == P::TABLE).then_some(id)
            })
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
}
