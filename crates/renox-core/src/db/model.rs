use std::future::Future;

use super::{DateTime, DbValue, Executor, Query, ToDbValue, now, quote, sql};
use crate::{Error, Result};
use anyhow::anyhow;

/// A struct stored as a row in a table. Derive it with `#[derive(Model)]`.
///
/// ```
/// # use renox::prelude::*;
/// # use serde::Serialize;
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
/// # async fn demo(db: Db) -> Result {
/// let mut kopi = Produk { nama: "Kopi".into(), harga: 18_000, ..Default::default() };
/// kopi.save(&db).await?;                      // INSERT, sets id and timestamps
/// let murah = Produk::query().where_op("harga", "<", 20_000).get(&db).await?;
/// # let _ = murah; Ok(()) }
/// ```
///
/// The primary key is an `id: i64` column; `0` means "not saved yet".
pub trait Model: super::FromRow + Sized + Send + Sync + Unpin + 'static {
    const TABLE: &'static str;
    /// Every column, including `id`.
    const COLUMNS: &'static [&'static str];
    /// `delete()` sets `deleted_at` instead of removing the row, and queries
    /// skip deleted rows unless asked with `with_trashed()` / `only_trashed()`.
    const SOFT_DELETES: bool = false;
    /// Select every column (`*`) instead of `COLUMNS`, so `from_row` also
    /// sees columns the struct doesn't list, and queries may filter on them.
    /// The built-in `User` does this to keep the app's own columns.
    const SELECT_ALL: bool = false;

    fn id(&self) -> i64;
    fn set_id(&mut self, id: i64);
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

    fn all<'c, E: Executor<'c>>(db: E) -> impl Future<Output = Result<Vec<Self>>> + Send {
        Self::query().order_by("id").get(db)
    }

    fn find<'c, E: Executor<'c>>(
        db: E,
        id: i64,
    ) -> impl Future<Output = Result<Option<Self>>> + Send {
        Self::query().where_eq("id", id).first(db)
    }

    /// The rows with these ids, in id order (missing ids are skipped).
    fn find_many<'c, E: Executor<'c>>(
        db: E,
        ids: impl IntoIterator<Item = i64>,
    ) -> impl Future<Output = Result<Vec<Self>>> + Send {
        let ids: Vec<i64> = ids.into_iter().collect();
        Self::query().where_in("id", ids).order_by("id").get(db)
    }

    /// Inserts many new models with a few statements (ids aren't returned;
    /// use `create` when you need them). Timestamps are set. Returns the
    /// number of rows inserted.
    fn insert_many<'c, E: Executor<'c>>(
        db: E,
        models: Vec<Self>,
    ) -> impl Future<Output = Result<u64>> + Send {
        async move { write_many::<Self>(db.into_conn(), models, None).await }
    }

    /// Inserts `models`, or updates the rows they clash with on the
    /// `unique_by` columns (which need a unique index), setting `update`
    /// columns (and `updated_at` when the model has it). Returns the rows
    /// written.
    ///
    /// ```
    /// # use renox::prelude::*;
    /// # #[derive(Model, serde::Serialize, Default)] struct Stock { id: i64, sku: String, qty: i64 }
    /// # async fn demo(db: Db) -> Result {
    /// let feed = vec![Stock { sku: "KOPI-1".into(), qty: 12, ..Default::default() }];
    /// Stock::upsert(&db, feed, &["sku"], &["qty"]).await?;
    /// # Ok(()) }
    /// ```
    fn upsert<'c, E: Executor<'c>>(
        db: E,
        models: Vec<Self>,
        unique_by: &[&str],
        update: &[&str],
    ) -> impl Future<Output = Result<u64>> + Send {
        let unique_by: Vec<String> = unique_by.iter().map(|c| (*c).to_owned()).collect();
        let update: Vec<String> = update.iter().map(|c| (*c).to_owned()).collect();
        async move {
            for column in unique_by.iter().chain(&update) {
                if !Self::COLUMNS.contains(&column.as_str()) || column == "id" {
                    return Err(
                        anyhow!("`{}` has no column `{column}` to upsert", Self::TABLE).into(),
                    );
                }
            }
            if unique_by.is_empty() {
                return Err(anyhow!("upsert needs at least one `unique_by` column").into());
            }
            write_many::<Self>(db.into_conn(), models, Some((unique_by, update))).await
        }
    }

    /// Like `find`, but a missing row becomes a 404 response.
    fn find_or_404<'c, E: Executor<'c>>(
        db: E,
        id: i64,
    ) -> impl Future<Output = Result<Self>> + Send {
        async move { Self::find(db, id).await?.ok_or(Error::NotFound) }
    }

    /// Saves a new model and returns it with its id and timestamps.
    fn create<'c, E: Executor<'c>>(
        db: E,
        mut model: Self,
    ) -> impl Future<Output = Result<Self>> + Send {
        async move {
            model.save(db).await?;
            Ok(model)
        }
    }

    /// Inserts the model if its id is `0`, otherwise updates its row.
    fn save<'c, E: Executor<'c>>(&mut self, db: E) -> impl Future<Output = Result> + Send {
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
                let sql_text = if columns.is_empty() {
                    format!("INSERT INTO {table} DEFAULT VALUES RETURNING id")
                } else {
                    let marks = vec!["?"; columns.len()].join(", ");
                    format!(
                        "INSERT INTO {table} ({}) VALUES ({marks}) RETURNING id",
                        columns.join(", ")
                    )
                };
                let id: i64 = sql(sql_text).bind_all(values).scalar(db).await?;
                self.set_id(id);
            } else {
                if columns.is_empty() {
                    return Ok(());
                }
                let sets: Vec<String> = columns.iter().map(|c| format!("{c} = ?")).collect();
                let changed = sql(format!(
                    "UPDATE {table} SET {} WHERE id = ?",
                    sets.join(", ")
                ))
                .bind_all(values)
                .bind(self.id())
                .execute(db)
                .await?;
                if changed == 0 {
                    return Err(Error::NotFound);
                }
            }
            Ok(())
        }
    }

    /// Deletes the row, or marks it deleted for models with soft deletes.
    fn delete<'c, E: Executor<'c>>(&mut self, db: E) -> impl Future<Output = Result> + Send {
        async move {
            if !Self::SOFT_DELETES {
                return self.force_delete(db).await;
            }
            let at = now();
            sql(format!(
                "UPDATE {} SET deleted_at = ? WHERE id = ?",
                quote(Self::TABLE)
            ))
            .bind(at)
            .bind(self.id())
            .execute(db)
            .await?;
            self.set_deleted_at(Some(at));
            Ok(())
        }
    }

    /// Removes the row, even for models with soft deletes.
    fn force_delete<'c, E: Executor<'c>>(&self, db: E) -> impl Future<Output = Result> + Send {
        async move {
            sql(format!("DELETE FROM {} WHERE id = ?", quote(Self::TABLE)))
                .bind(self.id())
                .execute(db)
                .await?;
            Ok(())
        }
    }

    /// Brings back a soft-deleted row.
    fn restore<'c, E: Executor<'c>>(&mut self, db: E) -> impl Future<Output = Result> + Send {
        async move {
            if !Self::SOFT_DELETES {
                return Err(anyhow!("{} does not use soft deletes", Self::TABLE).into());
            }
            sql(format!(
                "UPDATE {} SET deleted_at = NULL WHERE id = ?",
                quote(Self::TABLE)
            ))
            .bind(self.id())
            .execute(db)
            .await?;
            self.set_deleted_at(None);
            Ok(())
        }
    }
}

/// Most parameters one statement binds; both databases allow more
/// (SQLite 32,766, PostgreSQL 65,535).
const MAX_BINDS: usize = 30_000;

/// `insert_many` / `upsert`: multi-row INSERTs in chunks under the bind limit.
async fn write_many<M: Model>(
    mut conn: super::Conn<'_>,
    mut models: Vec<M>,
    upsert: Option<(Vec<String>, Vec<String>)>,
) -> Result<u64> {
    let at = now();
    for model in &mut models {
        model.touch(at, true);
    }
    let columns: Vec<&str> = M::COLUMNS.iter().copied().filter(|c| *c != "id").collect();
    if columns.is_empty() || models.is_empty() {
        return Ok(0);
    }
    let quoted: Vec<String> = columns.iter().map(|c| quote(c)).collect();
    let row_marks = format!("({})", vec!["?"; columns.len()].join(", "));
    let conflict = upsert.map(|(unique_by, update)| {
        let targets: Vec<String> = unique_by.iter().map(|c| quote(c)).collect();
        let mut sets: Vec<String> = update
            .iter()
            .map(|c| format!("{0} = excluded.{0}", quote(c)))
            .collect();
        if columns.contains(&"updated_at") && !update.iter().any(|c| c == "updated_at") {
            sets.push(format!("{0} = excluded.{0}", quote("updated_at")));
        }
        if sets.is_empty() {
            format!(" ON CONFLICT ({}) DO NOTHING", targets.join(", "))
        } else {
            format!(
                " ON CONFLICT ({}) DO UPDATE SET {}",
                targets.join(", "),
                sets.join(", ")
            )
        }
    });
    let per_statement = (MAX_BINDS / columns.len()).max(1);
    let mut written = 0;
    for chunk in models.chunks(per_statement) {
        let marks = vec![row_marks.as_str(); chunk.len()].join(", ");
        let statement = format!(
            "INSERT INTO {} ({}) VALUES {marks}{}",
            quote(M::TABLE),
            quoted.join(", "),
            conflict.as_deref().unwrap_or_default()
        );
        written += sql(statement)
            .bind_all(chunk.iter().flat_map(Model::values))
            .execute(conn.reborrow())
            .await?;
    }
    Ok(written)
}
