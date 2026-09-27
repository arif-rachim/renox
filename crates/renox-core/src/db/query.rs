use std::marker::PhantomData;

use anyhow::anyhow;
use sqlx::sqlite::SqliteExecutor;
use sqlx::{AssertSqlSafe, Row};

use super::value::bind;
use super::{Db, DbValue, Model, Paginated, ToDbValue, now, quote};
use crate::Result;

const OPERATORS: &[&str] = &["=", "!=", "<>", "<", "<=", ">", ">=", "like", "not like"];

#[derive(Clone, Copy, PartialEq)]
enum Trashed {
    Without,
    With,
    Only,
}

/// A query on a model's table, built with chained filters.
///
/// ```ignore
/// let produk = Produk::query()
///     .where_eq("kategori", "kopi")
///     .where_op("harga", "<", 25_000)
///     .order_by("nama")
///     .paginate(&db, page, 20)
///     .await?;
/// ```
///
/// Column names are checked against the model; an unknown column or operator
/// makes the query return an error instead of running.
pub struct Query<M> {
    filters: Vec<String>,
    binds: Vec<DbValue>,
    order: Vec<String>,
    limit: Option<u64>,
    offset: Option<u64>,
    trashed: Trashed,
    error: Option<String>,
    model: PhantomData<fn() -> M>,
}

impl<M> Clone for Query<M> {
    fn clone(&self) -> Self {
        Self {
            filters: self.filters.clone(),
            binds: self.binds.clone(),
            order: self.order.clone(),
            limit: self.limit,
            offset: self.offset,
            trashed: self.trashed,
            error: self.error.clone(),
            model: PhantomData,
        }
    }
}

impl<M: Model> Query<M> {
    pub(crate) fn new() -> Self {
        Self {
            filters: Vec::new(),
            binds: Vec::new(),
            order: Vec::new(),
            limit: None,
            offset: None,
            trashed: Trashed::Without,
            error: None,
            model: PhantomData,
        }
    }

    fn column(&mut self, column: &str) -> Option<String> {
        if M::COLUMNS.contains(&column) {
            Some(quote(column))
        } else {
            self.error
                .get_or_insert_with(|| format!("`{}` has no column `{column}`", M::TABLE));
            None
        }
    }

    pub fn where_eq(self, column: &str, value: impl ToDbValue) -> Self {
        self.where_op(column, "=", value)
    }

    /// Filters with a comparison: `=`, `!=`, `<>`, `<`, `<=`, `>`, `>=`, `like`, `not like`.
    pub fn where_op(mut self, column: &str, op: &str, value: impl ToDbValue) -> Self {
        let op = op.to_ascii_lowercase();
        if !OPERATORS.contains(&op.as_str()) {
            self.error
                .get_or_insert_with(|| format!("unsupported operator `{op}`"));
            return self;
        }
        if let Some(column) = self.column(column) {
            self.filters
                .push(format!("{column} {} ?", op.to_uppercase()));
            self.binds.push(value.to_db_value());
        }
        self
    }

    pub fn where_like(self, column: &str, pattern: impl ToDbValue) -> Self {
        self.where_op(column, "like", pattern)
    }

    pub fn where_null(mut self, column: &str) -> Self {
        if let Some(column) = self.column(column) {
            self.filters.push(format!("{column} IS NULL"));
        }
        self
    }

    pub fn where_not_null(mut self, column: &str) -> Self {
        if let Some(column) = self.column(column) {
            self.filters.push(format!("{column} IS NOT NULL"));
        }
        self
    }

    pub fn where_in<V: ToDbValue>(
        mut self,
        column: &str,
        values: impl IntoIterator<Item = V>,
    ) -> Self {
        let values: Vec<DbValue> = values.into_iter().map(|v| v.to_db_value()).collect();
        if let Some(column) = self.column(column) {
            if values.is_empty() {
                self.filters.push("0 = 1".into());
            } else {
                let marks = vec!["?"; values.len()].join(", ");
                self.filters.push(format!("{column} IN ({marks})"));
                self.binds.extend(values);
            }
        }
        self
    }

    pub fn order_by(mut self, column: &str) -> Self {
        if let Some(column) = self.column(column) {
            self.order.push(format!("{column} ASC"));
        }
        self
    }

    pub fn order_by_desc(mut self, column: &str) -> Self {
        if let Some(column) = self.column(column) {
            self.order.push(format!("{column} DESC"));
        }
        self
    }

    /// Newest first, by `created_at` when the model has it, otherwise by `id`.
    pub fn latest(self) -> Self {
        let column = if M::COLUMNS.contains(&"created_at") {
            "created_at"
        } else {
            "id"
        };
        self.order_by_desc(column).order_by_desc("id")
    }

    pub fn limit(mut self, limit: u64) -> Self {
        self.limit = Some(limit);
        self
    }

    pub fn offset(mut self, offset: u64) -> Self {
        self.offset = Some(offset);
        self
    }

    /// Include soft-deleted rows.
    pub fn with_trashed(mut self) -> Self {
        self.trashed = Trashed::With;
        self
    }

    /// Only soft-deleted rows.
    pub fn only_trashed(mut self) -> Self {
        self.trashed = Trashed::Only;
        self
    }

    fn check(&self) -> Result {
        match &self.error {
            Some(error) => Err(anyhow!("invalid query: {error}").into()),
            None => Ok(()),
        }
    }

    fn where_sql(&self) -> String {
        let mut filters = self.filters.clone();
        if M::SOFT_DELETES {
            match self.trashed {
                Trashed::Without => filters.push("\"deleted_at\" IS NULL".into()),
                Trashed::Only => filters.push("\"deleted_at\" IS NOT NULL".into()),
                Trashed::With => {}
            }
        }
        if filters.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", filters.join(" AND "))
        }
    }

    fn select_sql(&self) -> String {
        let columns: Vec<String> = M::COLUMNS.iter().map(|c| quote(c)).collect();
        let mut sql = format!(
            "SELECT {} FROM {}{}",
            columns.join(", "),
            quote(M::TABLE),
            self.where_sql()
        );
        if !self.order.is_empty() {
            sql.push_str(&format!(" ORDER BY {}", self.order.join(", ")));
        }
        match (self.limit, self.offset) {
            (Some(limit), Some(offset)) => sql.push_str(&format!(" LIMIT {limit} OFFSET {offset}")),
            (Some(limit), None) => sql.push_str(&format!(" LIMIT {limit}")),
            (None, Some(offset)) => sql.push_str(&format!(" LIMIT -1 OFFSET {offset}")),
            (None, None) => {}
        }
        sql
    }

    pub async fn get<'c, E: SqliteExecutor<'c>>(self, db: E) -> Result<Vec<M>> {
        self.check()?;
        let query = self
            .binds
            .iter()
            .cloned()
            .fold(sqlx::query(AssertSqlSafe(self.select_sql())), bind);
        let rows = query.fetch_all(db).await?;
        Ok(rows
            .iter()
            .map(M::from_row)
            .collect::<std::result::Result<_, _>>()?)
    }

    pub async fn first<'c, E: SqliteExecutor<'c>>(self, db: E) -> Result<Option<M>> {
        Ok(self.limit(1).get(db).await?.into_iter().next())
    }

    pub async fn count<'c, E: SqliteExecutor<'c>>(self, db: E) -> Result<u64> {
        self.check()?;
        let sql = format!(
            "SELECT COUNT(*) FROM {}{}",
            quote(M::TABLE),
            self.where_sql()
        );
        let query = self
            .binds
            .iter()
            .cloned()
            .fold(sqlx::query(AssertSqlSafe(sql)), bind);
        let count: i64 = query.fetch_one(db).await?.try_get(0)?;
        Ok(count as u64)
    }

    pub async fn exists<'c, E: SqliteExecutor<'c>>(self, db: E) -> Result<bool> {
        Ok(self.count(db).await? > 0)
    }

    /// One page of results plus the numbers needed to render page links.
    /// `page` starts at 1; `per_page` is capped at 1000.
    pub async fn paginate(self, db: &Db, page: u32, per_page: u32) -> Result<Paginated<M>> {
        let page = page.max(1);
        let per_page = per_page.clamp(1, 1000);
        let total = self.clone().count(db).await?;
        let items = self
            .limit(u64::from(per_page))
            .offset(u64::from(page - 1) * u64::from(per_page))
            .get(db)
            .await?;
        Ok(Paginated::new(items, page, per_page, total))
    }

    /// Deletes every matching row (soft-deletes them for models with soft deletes).
    pub async fn delete<'c, E: SqliteExecutor<'c>>(self, db: E) -> Result<u64> {
        if M::SOFT_DELETES {
            self.check()?;
            let sql = format!(
                "UPDATE {} SET \"deleted_at\" = ?{}",
                quote(M::TABLE),
                self.where_sql()
            );
            let query = std::iter::once(now().to_db_value())
                .chain(self.binds.iter().cloned())
                .fold(sqlx::query(AssertSqlSafe(sql)), bind);
            return Ok(query.execute(db).await?.rows_affected());
        }
        self.force_delete(db).await
    }

    /// Removes every matching row, even for models with soft deletes.
    pub async fn force_delete<'c, E: SqliteExecutor<'c>>(self, db: E) -> Result<u64> {
        self.check()?;
        let sql = format!("DELETE FROM {}{}", quote(M::TABLE), self.where_sql());
        let query = self
            .binds
            .iter()
            .cloned()
            .fold(sqlx::query(AssertSqlSafe(sql)), bind);
        Ok(query.execute(db).await?.rows_affected())
    }
}
