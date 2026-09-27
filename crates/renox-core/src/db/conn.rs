//! Renox's own handle on the database, so the same code runs on SQLite and
//! PostgreSQL: [`Db`] (the pool), [`Transaction`], [`Row`] and raw SQL
//! through [`sql()`].
//!
//! SQL written for Renox uses `?` placeholders. On PostgreSQL they become
//! `$1`, `$2`, … before the query is sent (a `?` inside quotes or comments is
//! left alone).

#[cfg(any(feature = "postgres", test))]
use std::borrow::Cow;
use std::fmt;

use sqlx::sqlite::{Sqlite, SqliteArguments, SqlitePool, SqliteRow};
use sqlx::{AssertSqlSafe, Column, Row as _};

use super::ToDbValue;
use super::value::DbValue;

#[cfg(feature = "postgres")]
use sqlx::postgres::{PgArguments, PgPool, PgRow, Postgres};

/// Which database engine a [`Db`] talks to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dialect {
    Sqlite,
    Postgres,
}

impl fmt::Display for Dialect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Dialect::Sqlite => "sqlite",
            Dialect::Postgres => "postgres",
        })
    }
}

/// The database pool. Take it in a handler with `State(db): State<Db>`.
///
/// Models, queries and [`sql()`] take `&db` (or `&mut tx` inside a
/// [`Transaction`]). For something only sqlx can do, reach the underlying
/// pool with [`Db::sqlite`] or [`Db::postgres`].
#[derive(Clone)]
pub struct Db {
    pool: Pool,
}

#[derive(Clone)]
enum Pool {
    Sqlite(SqlitePool),
    #[cfg(feature = "postgres")]
    Postgres(PgPool),
}

impl fmt::Debug for Db {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Db")
            .field("dialect", &self.dialect())
            .finish_non_exhaustive()
    }
}

impl From<SqlitePool> for Db {
    fn from(pool: SqlitePool) -> Self {
        Self {
            pool: Pool::Sqlite(pool),
        }
    }
}

#[cfg(feature = "postgres")]
impl From<PgPool> for Db {
    fn from(pool: PgPool) -> Self {
        Self {
            pool: Pool::Postgres(pool),
        }
    }
}

impl Db {
    pub fn dialect(&self) -> Dialect {
        match self.pool {
            Pool::Sqlite(_) => Dialect::Sqlite,
            #[cfg(feature = "postgres")]
            Pool::Postgres(_) => Dialect::Postgres,
        }
    }

    /// The sqlx pool, when the database is SQLite.
    pub fn sqlite(&self) -> Option<&SqlitePool> {
        match &self.pool {
            Pool::Sqlite(pool) => Some(pool),
            #[cfg(feature = "postgres")]
            Pool::Postgres(_) => None,
        }
    }

    /// The sqlx pool, when the database is PostgreSQL.
    #[cfg(feature = "postgres")]
    pub fn postgres(&self) -> Option<&PgPool> {
        match &self.pool {
            Pool::Postgres(pool) => Some(pool),
            Pool::Sqlite(_) => None,
        }
    }

    /// Starts a transaction. Pass `&mut tx` wherever a `&db` goes, then
    /// `tx.commit()`; dropping it without committing rolls it back.
    pub async fn begin(&self) -> Result<Transaction, sqlx::Error> {
        let inner = match &self.pool {
            Pool::Sqlite(pool) => TxInner::Sqlite(pool.begin().await?),
            #[cfg(feature = "postgres")]
            Pool::Postgres(pool) => TxInner::Postgres(pool.begin().await?),
        };
        Ok(Transaction { inner })
    }

    /// Closes every connection; later queries fail.
    pub async fn close(&self) {
        match &self.pool {
            Pool::Sqlite(pool) => pool.close().await,
            #[cfg(feature = "postgres")]
            Pool::Postgres(pool) => pool.close().await,
        }
    }
}

/// A database transaction, from [`Db::begin`].
pub struct Transaction {
    inner: TxInner,
}

enum TxInner {
    Sqlite(sqlx::Transaction<'static, Sqlite>),
    #[cfg(feature = "postgres")]
    Postgres(sqlx::Transaction<'static, Postgres>),
}

impl Transaction {
    pub fn dialect(&self) -> Dialect {
        match self.inner {
            TxInner::Sqlite(_) => Dialect::Sqlite,
            #[cfg(feature = "postgres")]
            TxInner::Postgres(_) => Dialect::Postgres,
        }
    }

    pub async fn commit(self) -> Result<(), sqlx::Error> {
        match self.inner {
            TxInner::Sqlite(tx) => tx.commit().await,
            #[cfg(feature = "postgres")]
            TxInner::Postgres(tx) => tx.commit().await,
        }
    }

    pub async fn rollback(self) -> Result<(), sqlx::Error> {
        match self.inner {
            TxInner::Sqlite(tx) => tx.rollback().await,
            #[cfg(feature = "postgres")]
            TxInner::Postgres(tx) => tx.rollback().await,
        }
    }
}

impl fmt::Debug for Transaction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Transaction")
            .field("dialect", &self.dialect())
            .finish_non_exhaustive()
    }
}

/// Where a query runs: `&db` or `&mut tx`.
pub trait Executor<'c>: Send {
    #[doc(hidden)]
    fn into_conn(self) -> Conn<'c>;
}

#[doc(hidden)]
pub enum Conn<'c> {
    Pool(&'c Db),
    Tx(&'c mut Transaction),
}

impl<'c> Executor<'c> for &'c Db {
    fn into_conn(self) -> Conn<'c> {
        Conn::Pool(self)
    }
}

impl<'c> Executor<'c> for &'c mut Transaction {
    fn into_conn(self) -> Conn<'c> {
        Conn::Tx(self)
    }
}

/// Runs `$body` against whichever backend `$conn` is: `$build` makes the
/// backend's query from `(sql, args)` and `$exec` is its sqlx executor.
macro_rules! dispatch {
    ($conn:expr, |$build:ident, $exec:ident| $body:expr) => {
        match $conn {
            Conn::Pool(db) => match &db.pool {
                Pool::Sqlite(pool) => {
                    #[allow(unused_variables)]
                    let $build = sqlite_query;
                    let $exec = pool;
                    $body
                }
                #[cfg(feature = "postgres")]
                Pool::Postgres(pool) => {
                    #[allow(unused_variables)]
                    let $build = postgres_query;
                    let $exec = pool;
                    $body
                }
            },
            Conn::Tx(tx) => match &mut tx.inner {
                TxInner::Sqlite(tx) => {
                    #[allow(unused_variables)]
                    let $build = sqlite_query;
                    let $exec = &mut **tx;
                    $body
                }
                #[cfg(feature = "postgres")]
                TxInner::Postgres(tx) => {
                    #[allow(unused_variables)]
                    let $build = postgres_query;
                    let $exec = &mut **tx;
                    $body
                }
            },
        }
    };
}

fn sqlite_query(
    sql: String,
    args: Vec<DbValue>,
) -> sqlx::query::Query<'static, Sqlite, SqliteArguments> {
    args.into_iter().fold(
        sqlx::query(AssertSqlSafe(sql)),
        |query, value| match value {
            DbValue::Null => query.bind(None::<i64>),
            DbValue::Integer(v) => query.bind(v),
            DbValue::Real(v) => query.bind(v),
            DbValue::Text(v) => query.bind(v),
            DbValue::Blob(v) => query.bind(v),
        },
    )
}

#[cfg(feature = "postgres")]
fn postgres_query(
    sql: String,
    args: Vec<DbValue>,
) -> sqlx::query::Query<'static, Postgres, PgArguments> {
    let sql = numbered_placeholders(&sql).into_owned();
    args.into_iter().fold(
        sqlx::query(AssertSqlSafe(sql)),
        |query, value| match value {
            DbValue::Null => query.bind(None::<i64>),
            DbValue::Integer(v) => query.bind(v),
            DbValue::Real(v) => query.bind(v),
            DbValue::Text(v) => query.bind(v),
            DbValue::Blob(v) => query.bind(v),
        },
    )
}

/// Rewrites `?` placeholders as PostgreSQL's `$1`, `$2`, …, skipping quoted
/// strings, quoted identifiers and comments.
#[cfg(any(feature = "postgres", test))]
pub(crate) fn numbered_placeholders(sql: &str) -> Cow<'_, str> {
    if !sql.contains('?') {
        return Cow::Borrowed(sql);
    }
    let mut out = String::with_capacity(sql.len() + 8);
    let mut chars = sql.chars().peekable();
    let mut n = 0;
    while let Some(c) = chars.next() {
        out.push(c);
        match c {
            '\'' | '"' => {
                // A doubled quote inside is an escaped quote: the loop ends
                // at the first one and the next iteration opens a new run.
                for inner in chars.by_ref() {
                    out.push(inner);
                    if inner == c {
                        break;
                    }
                }
            }
            '-' if chars.peek() == Some(&'-') => {
                for inner in chars.by_ref() {
                    out.push(inner);
                    if inner == '\n' {
                        break;
                    }
                }
            }
            '/' if chars.peek() == Some(&'*') => {
                let mut prev = '\0';
                for inner in chars.by_ref() {
                    out.push(inner);
                    if prev == '*' && inner == '/' {
                        break;
                    }
                    prev = inner;
                }
            }
            '?' => {
                n += 1;
                out.pop();
                out.push_str(&format!("${n}"));
            }
            _ => {}
        }
    }
    Cow::Owned(out)
}

/// A raw SQL statement with `?` placeholders.
///
/// ```ignore
/// let rows = renox::db::sql("SELECT nama FROM produk WHERE harga < ?")
///     .bind(20_000)
///     .fetch_all(&db)
///     .await?;
/// let nama: String = rows[0].try_get("nama")?;
///
/// let total: i64 = renox::db::sql("SELECT COUNT(*) FROM produk").scalar(&db).await?;
/// ```
pub fn sql(sql: impl Into<String>) -> Sql {
    Sql {
        sql: sql.into(),
        args: Vec::new(),
    }
}

/// A statement built by [`sql()`].
#[derive(Debug, Clone)]
#[must_use = "a statement does nothing until it is run"]
pub struct Sql {
    sql: String,
    args: Vec<DbValue>,
}

impl Sql {
    /// Binds the next `?`.
    pub fn bind(mut self, value: impl ToDbValue) -> Self {
        self.args.push(value.to_db_value());
        self
    }

    /// Binds several values in order.
    pub fn bind_all(mut self, values: impl IntoIterator<Item = DbValue>) -> Self {
        self.args.extend(values);
        self
    }

    pub async fn fetch_all<'c>(self, db: impl Executor<'c>) -> Result<Vec<Row>, sqlx::Error> {
        let Self { sql, args } = self;
        dispatch!(db.into_conn(), |build, exec| build(sql, args)
            .fetch_all(exec)
            .await
            .map(|rows| rows.into_iter().map(Row::from).collect()))
    }

    pub async fn fetch_optional<'c>(
        self,
        db: impl Executor<'c>,
    ) -> Result<Option<Row>, sqlx::Error> {
        let Self { sql, args } = self;
        dispatch!(db.into_conn(), |build, exec| build(sql, args)
            .fetch_optional(exec)
            .await
            .map(|row| row.map(Row::from)))
    }

    /// The first row; an error if there is none.
    pub async fn fetch_one<'c>(self, db: impl Executor<'c>) -> Result<Row, sqlx::Error> {
        self.fetch_optional(db)
            .await?
            .ok_or(sqlx::Error::RowNotFound)
    }

    /// Runs the statement and returns the number of rows it changed.
    pub async fn execute<'c>(self, db: impl Executor<'c>) -> Result<u64, sqlx::Error> {
        let Self { sql, args } = self;
        dispatch!(db.into_conn(), |build, exec| build(sql, args)
            .execute(exec)
            .await
            .map(|done| done.rows_affected()))
    }

    /// The first column of the first row; an error if there is no row.
    pub async fn scalar<'c, T: FromDb>(self, db: impl Executor<'c>) -> Result<T, sqlx::Error> {
        self.fetch_one(db).await?.try_get(0)
    }

    /// The first column of the first row, or `None` if there is no row.
    pub async fn scalar_optional<'c, T: FromDb>(
        self,
        db: impl Executor<'c>,
    ) -> Result<Option<T>, sqlx::Error> {
        self.fetch_optional(db)
            .await?
            .map(|row| row.try_get(0))
            .transpose()
    }

    /// The first column of every row.
    pub async fn scalars<'c, T: FromDb>(
        self,
        db: impl Executor<'c>,
    ) -> Result<Vec<T>, sqlx::Error> {
        self.fetch_all(db)
            .await?
            .iter()
            .map(|row| row.try_get(0))
            .collect()
    }
}

/// Runs SQL that may hold several statements and no parameters, e.g. a
/// migration file. Returns the number of rows changed.
pub(crate) async fn script<'c>(db: impl Executor<'c>, sql: &str) -> Result<u64, sqlx::Error> {
    let sql = sql.to_owned();
    dispatch!(db.into_conn(), |build, exec| sqlx::raw_sql(AssertSqlSafe(
        sql
    ))
    .execute(exec)
    .await
    .map(|done| done.rows_affected()))
}

/// One result row. Read columns by name or position with [`Row::try_get`].
pub struct Row(pub(crate) RowInner);

pub(crate) enum RowInner {
    Sqlite(SqliteRow),
    #[cfg(feature = "postgres")]
    Postgres(PgRow),
}

impl From<SqliteRow> for Row {
    fn from(row: SqliteRow) -> Self {
        Self(RowInner::Sqlite(row))
    }
}

#[cfg(feature = "postgres")]
impl From<PgRow> for Row {
    fn from(row: PgRow) -> Self {
        Self(RowInner::Postgres(row))
    }
}

impl Row {
    /// A column's value, by name (`"nama"`) or position (`0`).
    pub fn try_get<T: FromDb>(&self, index: impl RowIndex) -> Result<T, sqlx::Error> {
        match &self.0 {
            RowInner::Sqlite(row) => row.try_get(index),
            #[cfg(feature = "postgres")]
            RowInner::Postgres(row) => row.try_get(index),
        }
    }

    /// The column names, in order.
    pub fn columns(&self) -> Vec<&str> {
        match &self.0 {
            RowInner::Sqlite(row) => row.columns().iter().map(Column::name).collect(),
            #[cfg(feature = "postgres")]
            RowInner::Postgres(row) => row.columns().iter().map(Column::name).collect(),
        }
    }

    /// The sqlx row, when the database is SQLite.
    pub fn sqlite(&self) -> Option<&SqliteRow> {
        match &self.0 {
            RowInner::Sqlite(row) => Some(row),
            #[cfg(feature = "postgres")]
            RowInner::Postgres(_) => None,
        }
    }

    /// The sqlx row, when the database is PostgreSQL.
    #[cfg(feature = "postgres")]
    pub fn postgres(&self) -> Option<&PgRow> {
        match &self.0 {
            RowInner::Postgres(row) => Some(row),
            RowInner::Sqlite(_) => None,
        }
    }
}

impl fmt::Debug for Row {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Row")
            .field("columns", &self.columns())
            .finish_non_exhaustive()
    }
}

/// A Rust type that can be read from a column on every enabled database:
/// integers, floats, `bool`, `String`, `Vec<u8>`, chrono types, `Option<T>`, …
pub trait FromDb: for<'r> sqlx::Decode<'r, Sqlite> + sqlx::Type<Sqlite> + bounds::Postgres {}

impl<T> FromDb for T where
    T: for<'r> sqlx::Decode<'r, Sqlite> + sqlx::Type<Sqlite> + bounds::Postgres
{
}

/// A column name (`&str`) or position (`usize`).
pub trait RowIndex: sqlx::ColumnIndex<SqliteRow> + bounds::PostgresIndex {}

impl<T> RowIndex for T where T: sqlx::ColumnIndex<SqliteRow> + bounds::PostgresIndex {}

/// Extra bounds that only apply when the `postgres` feature is on.
#[doc(hidden)]
pub mod bounds {
    #[cfg(feature = "postgres")]
    mod on {
        use sqlx::postgres::PgRow;

        pub trait Postgres:
            for<'r> sqlx::Decode<'r, sqlx::Postgres> + sqlx::Type<sqlx::Postgres>
        {
        }
        impl<T> Postgres for T where T: for<'r> sqlx::Decode<'r, sqlx::Postgres> + sqlx::Type<sqlx::Postgres>
        {}

        pub trait PostgresIndex: sqlx::ColumnIndex<PgRow> {}
        impl<T: sqlx::ColumnIndex<PgRow>> PostgresIndex for T {}
    }

    #[cfg(not(feature = "postgres"))]
    mod on {
        pub trait Postgres {}
        impl<T: ?Sized> Postgres for T {}

        pub trait PostgresIndex {}
        impl<T: ?Sized> PostgresIndex for T {}
    }

    pub use on::{Postgres, PostgresIndex};
}

#[cfg(test)]
mod tests {
    use super::numbered_placeholders;

    #[test]
    fn numbers_placeholders_outside_quotes_and_comments() {
        assert_eq!(
            numbered_placeholders("SELECT * FROM t WHERE a = ? AND b IN (?, ?)"),
            "SELECT * FROM t WHERE a = $1 AND b IN ($2, $3)"
        );
        assert_eq!(
            numbered_placeholders("SELECT '?', \"a?\" -- why?\nFROM t /* ? */ WHERE x = ?"),
            "SELECT '?', \"a?\" -- why?\nFROM t /* ? */ WHERE x = $1"
        );
        assert_eq!(
            numbered_placeholders("SELECT 'it''s ?' WHERE y = ?"),
            "SELECT 'it''s ?' WHERE y = $1"
        );
        assert_eq!(numbered_placeholders("SELECT 1"), "SELECT 1");
    }
}
