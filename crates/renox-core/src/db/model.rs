use std::future::Future;

use anyhow::anyhow;
use sqlx::sqlite::SqliteExecutor;
use sqlx::{AssertSqlSafe, Row};

use super::value::bind;
use super::{DateTime, DbValue, Query, SqliteRow, ToDbValue, now, quote};
use crate::{Error, Result};

/// A struct stored as a row in a table. Derive it with `#[derive(Model)]`.
///
/// ```ignore
/// #[derive(Model, Serialize, Default)]
/// #[model(table = "produk", soft_deletes)]
/// struct Produk {
///     id: i64,
///     nama: String,
///     harga: i64,
///     created_at: Option<DateTime>,
///     updated_at: Option<DateTime>,
///     deleted_at: Option<DateTime>,
/// }
///
/// let mut kopi = Produk { nama: "Kopi".into(), harga: 18_000, ..Default::default() };
/// kopi.save(&db).await?;                      // INSERT, sets id and timestamps
/// let murah = Produk::query().where_op("harga", "<", 20_000).get(&db).await?;
/// ```
///
/// The primary key is an `id: i64` column; `0` means "not saved yet".
pub trait Model: Sized + Send + Sync + Unpin + 'static {
    const TABLE: &'static str;
    /// Every column, including `id`.
    const COLUMNS: &'static [&'static str];
    /// `delete()` sets `deleted_at` instead of removing the row, and queries
    /// skip deleted rows unless asked with `with_trashed()` / `only_trashed()`.
    const SOFT_DELETES: bool = false;

    fn id(&self) -> i64;
    fn set_id(&mut self, id: i64);
    fn from_row(row: &SqliteRow) -> std::result::Result<Self, sqlx::Error>;
    /// Values of every column except `id`, in `COLUMNS` order.
    fn values(&self) -> Vec<DbValue>;
    /// Updates `created_at` / `updated_at` if the model has them.
    fn touch(&mut self, _now: DateTime, _creating: bool) {}
    /// Updates `deleted_at` if the model has it.
    fn set_deleted_at(&mut self, _at: Option<DateTime>) {}

    fn query() -> Query<Self> {
        Query::new()
    }

    /// Shorthand for `query().where_eq(column, value)`.
    fn where_eq(column: &str, value: impl ToDbValue) -> Query<Self> {
        Self::query().where_eq(column, value)
    }

    fn all<'c, E: SqliteExecutor<'c>>(db: E) -> impl Future<Output = Result<Vec<Self>>> + Send {
        Self::query().order_by("id").get(db)
    }

    fn find<'c, E: SqliteExecutor<'c>>(
        db: E,
        id: i64,
    ) -> impl Future<Output = Result<Option<Self>>> + Send {
        Self::query().where_eq("id", id).first(db)
    }

    /// Like `find`, but a missing row becomes a 404 response.
    fn find_or_404<'c, E: SqliteExecutor<'c>>(
        db: E,
        id: i64,
    ) -> impl Future<Output = Result<Self>> + Send {
        async move { Self::find(db, id).await?.ok_or(Error::NotFound) }
    }

    /// Saves a new model and returns it with its id and timestamps.
    fn create<'c, E: SqliteExecutor<'c>>(
        db: E,
        mut model: Self,
    ) -> impl Future<Output = Result<Self>> + Send {
        async move {
            model.save(db).await?;
            Ok(model)
        }
    }

    /// Inserts the model if its id is `0`, otherwise updates its row.
    fn save<'c, E: SqliteExecutor<'c>>(&mut self, db: E) -> impl Future<Output = Result> + Send {
        async move {
            let creating = self.id() == 0;
            self.touch(now(), creating);
            let columns: Vec<String> = Self::COLUMNS
                .iter()
                .filter(|c| **c != "id")
                .map(|c| quote(c))
                .collect();
            let values = self.values();
            let table = quote(Self::TABLE);

            if creating {
                let sql = if columns.is_empty() {
                    format!("INSERT INTO {table} DEFAULT VALUES RETURNING id")
                } else {
                    let marks = vec!["?"; columns.len()].join(", ");
                    format!(
                        "INSERT INTO {table} ({}) VALUES ({marks}) RETURNING id",
                        columns.join(", ")
                    )
                };
                let query = values
                    .into_iter()
                    .fold(sqlx::query(AssertSqlSafe(sql)), bind);
                let id: i64 = query.fetch_one(db).await?.try_get(0)?;
                self.set_id(id);
            } else {
                if columns.is_empty() {
                    return Ok(());
                }
                let sets: Vec<String> = columns.iter().map(|c| format!("{c} = ?")).collect();
                let sql = format!("UPDATE {table} SET {} WHERE id = ?", sets.join(", "));
                let query = values
                    .into_iter()
                    .chain([DbValue::Integer(self.id())])
                    .fold(sqlx::query(AssertSqlSafe(sql)), bind);
                if query.execute(db).await?.rows_affected() == 0 {
                    return Err(Error::NotFound);
                }
            }
            Ok(())
        }
    }

    /// Deletes the row, or marks it deleted for models with soft deletes.
    fn delete<'c, E: SqliteExecutor<'c>>(&mut self, db: E) -> impl Future<Output = Result> + Send {
        async move {
            if !Self::SOFT_DELETES {
                return self.force_delete(db).await;
            }
            let at = now();
            let sql = format!(
                "UPDATE {} SET deleted_at = ? WHERE id = ?",
                quote(Self::TABLE)
            );
            [at.to_db_value(), DbValue::Integer(self.id())]
                .into_iter()
                .fold(sqlx::query(AssertSqlSafe(sql)), bind)
                .execute(db)
                .await?;
            self.set_deleted_at(Some(at));
            Ok(())
        }
    }

    /// Removes the row, even for models with soft deletes.
    fn force_delete<'c, E: SqliteExecutor<'c>>(
        &self,
        db: E,
    ) -> impl Future<Output = Result> + Send {
        async move {
            let sql = format!("DELETE FROM {} WHERE id = ?", quote(Self::TABLE));
            sqlx::query(AssertSqlSafe(sql))
                .bind(self.id())
                .execute(db)
                .await?;
            Ok(())
        }
    }

    /// Brings back a soft-deleted row.
    fn restore<'c, E: SqliteExecutor<'c>>(&mut self, db: E) -> impl Future<Output = Result> + Send {
        async move {
            if !Self::SOFT_DELETES {
                return Err(anyhow!("{} does not use soft deletes", Self::TABLE).into());
            }
            let sql = format!(
                "UPDATE {} SET deleted_at = NULL WHERE id = ?",
                quote(Self::TABLE)
            );
            sqlx::query(AssertSqlSafe(sql))
                .bind(self.id())
                .execute(db)
                .await?;
            self.set_deleted_at(None);
            Ok(())
        }
    }
}
