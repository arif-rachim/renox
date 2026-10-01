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
use std::sync::Arc;

use cookie::Key;

use sqlx::sqlite::{Sqlite, SqliteArguments, SqlitePool, SqliteRow};
use sqlx::{AssertSqlSafe, Column, Row as _};

use super::value::DbValue;
use super::{DbError, ToDbValue};

#[cfg(feature = "postgres")]
use sqlx::postgres::{PgArguments, PgPool, PgRow, Postgres};

/// Which database engine a [`Db`] talks to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dialect {
    /// SQLite.
    Sqlite,
    /// PostgreSQL (the `postgres` feature).
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
/// pool with [`Db::sqlite`] or `Db::postgres` (with the `postgres` feature).
#[derive(Clone)]
pub struct Db {
    pool: Pool,
    schema: SchemaEpoch,
    /// Seals and opens `Encrypted` columns (`APP_KEY`, set at boot).
    key: Option<Arc<Key>>,
}

/// When this process last changed the schema (migrations ran).
///
/// A pooled connection keeps the schema it read, and prepares statements
/// against it until it next steps one. sqlx takes a statement's columns from
/// that first prepare, so after `ALTER TABLE users ADD COLUMN …` a
/// `SELECT *` on an older connection gets one column more than sqlx expects
/// and panics (sqlx-sqlite `row.rs`), or PostgreSQL refuses a cached plan.
/// The pools made by [`super::connect`] drop connections opened before the
/// last change instead of reusing them.
#[derive(Clone, Default)]
pub(crate) struct SchemaEpoch(std::sync::Arc<std::sync::Mutex<Option<std::time::Instant>>>);

impl SchemaEpoch {
    pub(crate) fn changed(&self) {
        *self.0.lock().unwrap_or_else(|e| e.into_inner()) = Some(std::time::Instant::now());
    }

    /// Whether a connection of this age was opened before the last change.
    pub(crate) fn is_stale(&self, age: std::time::Duration) -> bool {
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_some_and(|at| age >= at.elapsed())
    }
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
        Self::with_epoch(Pool::Sqlite(pool), SchemaEpoch::default())
    }
}

#[cfg(feature = "postgres")]
impl From<PgPool> for Db {
    fn from(pool: PgPool) -> Self {
        Self::with_epoch(Pool::Postgres(pool), SchemaEpoch::default())
    }
}

impl Db {
    fn with_epoch(pool: Pool, schema: SchemaEpoch) -> Self {
        Self {
            pool,
            schema,
            key: None,
        }
    }

    /// The key `Encrypted` columns are sealed and opened with (the app's
    /// `APP_KEY`, set at boot).
    pub(crate) fn with_key(mut self, key: Key) -> Self {
        self.key = Some(Arc::new(key));
        self
    }

    pub(crate) fn from_sqlite(pool: SqlitePool, schema: SchemaEpoch) -> Self {
        Self::with_epoch(Pool::Sqlite(pool), schema)
    }

    #[cfg(feature = "postgres")]
    pub(crate) fn from_postgres(pool: PgPool, schema: SchemaEpoch) -> Self {
        Self::with_epoch(Pool::Postgres(pool), schema)
    }

    /// Records that the schema changed, so older pooled connections aren't
    /// reused (see `SchemaEpoch`).
    pub(crate) fn schema_changed(&self) {
        self.schema.changed();
    }

    /// The engine this pool talks to.
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
    pub async fn begin(&self) -> Result<Transaction, DbError> {
        let inner = match &self.pool {
            Pool::Sqlite(pool) => TxInner::Sqlite(pool.begin().await?),
            #[cfg(feature = "postgres")]
            Pool::Postgres(pool) => TxInner::Postgres(pool.begin().await?),
        };
        Ok(Transaction {
            inner,
            key: self.key.clone(),
            savepoints: 0,
        })
    }

    /// Runs `work` in a transaction: committed when it returns `Ok`, rolled
    /// back when it returns `Err`.
    ///
    /// ```
    /// # use renox::prelude::*;
    /// # async fn demo(db: Db) -> Result {
    /// let moved = db
    ///     .transaction_retrying(3, |tx| {
    ///         Box::pin(async move {
    ///             renox::db::sql("UPDATE accounts SET balance = balance - 100 WHERE id = 1")
    ///                 .execute(&mut *tx)
    ///                 .await?;
    ///             renox::db::sql("UPDATE accounts SET balance = balance + 100 WHERE id = 2")
    ///                 .execute(&mut *tx)
    ///                 .await?;
    ///             Ok(100)
    ///         })
    ///     })
    ///     .await?;
    /// # let _: i64 = moved; Ok(()) }
    /// ```
    pub async fn transaction<T, F>(&self, work: F) -> crate::Result<T>
    where
        F: for<'t> FnMut(
            &'t mut Transaction,
        ) -> futures_util::future::BoxFuture<'t, crate::Result<T>>,
    {
        self.transaction_retrying(1, work).await
    }

    /// Like [`Db::transaction`], trying up to `attempts` times when another
    /// transaction got in the way (SQLite busy, PostgreSQL serialization
    /// failure or deadlock), with a short, growing pause in between. `work`
    /// must be safe to run again.
    pub async fn transaction_retrying<T, F>(&self, attempts: u32, mut work: F) -> crate::Result<T>
    where
        F: for<'t> FnMut(
            &'t mut Transaction,
        ) -> futures_util::future::BoxFuture<'t, crate::Result<T>>,
    {
        let attempts = attempts.max(1);
        let mut attempt = 1;
        loop {
            let outcome = async {
                let mut tx = self.begin().await?;
                let value = work(&mut tx).await?;
                tx.commit().await?;
                Ok::<T, crate::Error>(value)
            }
            .await;
            match outcome {
                Err(err) if attempt < attempts && err.is_retryable() => {
                    let pause = std::time::Duration::from_millis(20 * u64::from(attempt));
                    tokio::time::sleep(pause).await;
                    attempt += 1;
                }
                other => return other,
            }
        }
    }

    /// Runs `work` again (up to `attempts` times, with a short, growing
    /// pause) while it fails because another transaction got in the way
    /// (SQLite busy, PostgreSQL serialization failure or deadlock). `work`
    /// opens and commits its own transaction, so unlike
    /// [`Db::transaction_retrying`] it can borrow from the caller, and it can
    /// roll back and still return a value (drop or `rollback` the
    /// transaction instead of committing).
    ///
    /// ```
    /// # use renox::prelude::*;
    /// enum Transfer { Done, Short }
    ///
    /// # async fn demo(db: Db, amount: i64, from: i64, to: i64) -> Result {
    /// let outcome = db
    ///     .retrying(3, || async {
    ///         let mut tx = db.begin().await?;
    ///         let taken = renox::db::sql(
    ///             "UPDATE accounts SET balance = balance - ? WHERE id = ? AND balance >= ?",
    ///         )
    ///         .bind(amount) // borrowed from the caller
    ///         .bind(from)
    ///         .bind(amount)
    ///         .execute(&mut tx)
    ///         .await?;
    ///         if taken == 0 {
    ///             tx.rollback().await?;
    ///             return Ok(Transfer::Short); // rolled back, with an answer
    ///         }
    ///         renox::db::sql("UPDATE accounts SET balance = balance + ? WHERE id = ?")
    ///             .bind(amount)
    ///             .bind(to)
    ///             .execute(&mut tx)
    ///             .await?;
    ///         tx.commit().await?;
    ///         Ok(Transfer::Done)
    ///     })
    ///     .await?;
    /// # let _ = outcome; Ok(()) }
    /// ```
    pub fn retrying<'a, T, F, Fut>(
        &'a self,
        attempts: u32,
        mut work: F,
    ) -> impl Future<Output = crate::Result<T>> + Send + 'a
    where
        T: Send + 'a,
        F: FnMut() -> Fut + Send + 'a,
        Fut: Future<Output = crate::Result<T>> + Send + 'a,
    {
        let attempts = attempts.max(1);
        async move {
            let mut attempt = 1;
            loop {
                match work().await {
                    Err(err) if attempt < attempts && err.is_retryable() => {
                        let pause = std::time::Duration::from_millis(20 * u64::from(attempt));
                        tokio::time::sleep(pause).await;
                        attempt += 1;
                    }
                    other => return other,
                }
            }
        }
    }

    /// A transaction that takes SQLite's write lock at once (`BEGIN
    /// IMMEDIATE`), so what it reads can't change before it writes: the
    /// SQLite counterpart of `lock_for_update`. Same as `begin` on PostgreSQL.
    pub async fn begin_immediate(&self) -> Result<Transaction, DbError> {
        let inner = match &self.pool {
            Pool::Sqlite(pool) => TxInner::Sqlite(pool.begin_with("BEGIN IMMEDIATE").await?),
            #[cfg(feature = "postgres")]
            Pool::Postgres(pool) => TxInner::Postgres(pool.begin().await?),
        };
        Ok(Transaction {
            inner,
            key: self.key.clone(),
            savepoints: 0,
        })
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
    key: Option<Arc<Key>>,
    /// Savepoints open inside it (for their names).
    savepoints: u32,
}

enum TxInner {
    Sqlite(sqlx::Transaction<'static, Sqlite>),
    #[cfg(feature = "postgres")]
    Postgres(sqlx::Transaction<'static, Postgres>),
}

impl Transaction {
    /// The engine this transaction runs on.
    pub fn dialect(&self) -> Dialect {
        match self.inner {
            TxInner::Sqlite(_) => Dialect::Sqlite,
            #[cfg(feature = "postgres")]
            TxInner::Postgres(_) => Dialect::Postgres,
        }
    }

    /// Commits the transaction.
    pub async fn commit(self) -> Result<(), DbError> {
        match self.inner {
            TxInner::Sqlite(tx) => Ok(tx.commit().await?),
            #[cfg(feature = "postgres")]
            TxInner::Postgres(tx) => Ok(tx.commit().await?),
        }
    }

    /// Rolls the transaction back (dropping it uncommitted does the same).
    pub async fn rollback(self) -> Result<(), DbError> {
        match self.inner {
            TxInner::Sqlite(tx) => Ok(tx.rollback().await?),
            #[cfg(feature = "postgres")]
            TxInner::Postgres(tx) => Ok(tx.rollback().await?),
        }
    }

    /// Runs `work` inside a savepoint (a transaction inside the
    /// transaction): when it returns `Err`, only what it did is undone and
    /// the transaction goes on; when it returns `Ok`, its changes stay, to
    /// be committed (or rolled back) with the rest. Savepoints nest.
    ///
    /// ```
    /// # use renox::prelude::*;
    /// # async fn demo(db: Db) -> Result {
    /// let mut tx = db.begin().await?;
    /// renox::db::sql("INSERT INTO orders (total) VALUES (?)").bind(18_000).execute(&mut tx).await?;
    /// // Optional: a voucher that may be used up already. If it is, the order stays.
    /// let voucher = tx
    ///     .savepoint(|tx| {
    ///         Box::pin(async move {
    ///             let used = renox::db::sql("UPDATE vouchers SET used = used + 1 WHERE code = ? AND used < max_uses")
    ///                 .bind("KOPI10")
    ///                 .execute(&mut *tx)
    ///                 .await?;
    ///             renox::abort_if(used == 0, StatusCode::CONFLICT, "used up")?;
    ///             Ok(())
    ///         })
    ///     })
    ///     .await;
    /// tx.commit().await?;
    /// # let _ = voucher; Ok(()) }
    /// ```
    ///
    /// On PostgreSQL a failed statement stops the whole transaction until
    /// it rolls back; inside a savepoint only the savepoint rolls back, so
    /// the transaction can go on after an expected failure (a unique
    /// violation you handle, say).
    pub async fn savepoint<T, F>(&mut self, work: F) -> crate::Result<T>
    where
        F: for<'t> FnOnce(
            &'t mut Transaction,
        ) -> futures_util::future::BoxFuture<'t, crate::Result<T>>,
    {
        let name = format!("renox_savepoint_{}", self.savepoints + 1);
        sql(format!("SAVEPOINT {name}")).execute(&mut *self).await?;
        self.savepoints += 1;
        let outcome = work(self).await;
        self.savepoints -= 1;
        match outcome {
            Ok(value) => {
                sql(format!("RELEASE SAVEPOINT {name}"))
                    .execute(&mut *self)
                    .await?;
                Ok(value)
            }
            Err(err) => {
                sql(format!("ROLLBACK TO SAVEPOINT {name}"))
                    .execute(&mut *self)
                    .await?;
                sql(format!("RELEASE SAVEPOINT {name}"))
                    .execute(&mut *self)
                    .await?;
                Err(err)
            }
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

impl<'c> Executor<'c> for Conn<'c> {
    fn into_conn(self) -> Conn<'c> {
        self
    }
}

impl Conn<'_> {
    /// The key for `Encrypted` columns of the database this runs on.
    fn key(&self) -> Option<Arc<Key>> {
        match self {
            Conn::Pool(db) => db.key.clone(),
            Conn::Tx(tx) => tx.key.clone(),
        }
    }

    /// The same connection for one more statement.
    pub(crate) fn reborrow(&mut self) -> Conn<'_> {
        match self {
            Conn::Pool(db) => Conn::Pool(db),
            Conn::Tx(tx) => Conn::Tx(tx),
        }
    }

    /// Which engine the query will run on, for SQL that differs.
    pub(crate) fn dialect(&self) -> Dialect {
        match self {
            Conn::Pool(db) => db.dialect(),
            Conn::Tx(tx) => tx.dialect(),
        }
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
                    ($body).map_err(DbError::from)
                }
                #[cfg(feature = "postgres")]
                Pool::Postgres(pool) => {
                    #[allow(unused_variables)]
                    let $build = postgres_query;
                    let $exec = pool;
                    ($body).map_err(DbError::from)
                }
            },
            Conn::Tx(tx) => match &mut tx.inner {
                TxInner::Sqlite(tx) => {
                    #[allow(unused_variables)]
                    let $build = sqlite_query;
                    let $exec = &mut **tx;
                    ($body).map_err(DbError::from)
                }
                #[cfg(feature = "postgres")]
                TxInner::Postgres(tx) => {
                    #[allow(unused_variables)]
                    let $build = postgres_query;
                    let $exec = &mut **tx;
                    ($body).map_err(DbError::from)
                }
            },
        }
    };
}

/// Seals the values of `Encrypted` fields with the database's key.
fn seal(key: Option<&Key>, args: Vec<DbValue>) -> Result<Vec<DbValue>, DbError> {
    args.into_iter()
        .map(|value| match value {
            DbValue::Encrypted(plain) => match key {
                Some(key) => Ok(DbValue::Text(super::encrypted::seal(key, &plain.0))),
                None => Err(DbError::from(sqlx::Error::Encode(
                    "an Encrypted value needs the app's Db (it has the APP_KEY); \
                     this Db was made outside App"
                        .into(),
                ))),
            },
            other => Ok(other),
        })
        .collect()
}

fn sqlite_query(
    sql: String,
    args: Vec<DbValue>,
) -> sqlx::query::Query<'static, Sqlite, SqliteArguments> {
    args.into_iter().fold(
        sqlx::query(AssertSqlSafe(sql)),
        |query, value| match value.for_sqlite() {
            DbValue::Integer(v) => query.bind(v),
            DbValue::Real(v) => query.bind(v),
            DbValue::Text(v) => query.bind(v),
            DbValue::Blob(v) => query.bind(v),
            _ => query.bind(None::<i64>),
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
            DbValue::Null => query.bind(UntypedNull),
            DbValue::Integer(v) => query.bind(v),
            DbValue::Real(v) => query.bind(v),
            DbValue::Text(v) => query.bind(v),
            DbValue::Blob(v) => query.bind(v),
            DbValue::Bool(v) => query.bind(v),
            DbValue::DateTime(v) => query.bind(v),
            DbValue::NaiveDateTime(v) => query.bind(v),
            DbValue::Date(v) => query.bind(v),
            DbValue::Time(v) => query.bind(v),
            DbValue::Json(v) => query.bind(sqlx::types::Json(v)),
            #[cfg(feature = "uuid")]
            DbValue::Uuid(v) => query.bind(v),
            // Sealed into `Text` before the statement is built.
            DbValue::Encrypted(_) => query.bind(UntypedNull),
        },
    )
}

/// A `NULL` parameter without a type, so PostgreSQL takes the type from
/// where it's used (a `NULL` sent as `BIGINT` can't go into a `TIMESTAMPTZ`).
#[cfg(feature = "postgres")]
struct UntypedNull;

#[cfg(feature = "postgres")]
impl sqlx::Type<Postgres> for UntypedNull {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        // OID 0 means "unspecified" in the protocol.
        sqlx::postgres::PgTypeInfo::with_oid(sqlx::postgres::types::Oid(0))
    }
}

#[cfg(feature = "postgres")]
impl sqlx::Encode<'_, Postgres> for UntypedNull {
    fn encode_by_ref(
        &self,
        _buf: &mut sqlx::postgres::PgArgumentBuffer,
    ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
        Ok(sqlx::encode::IsNull::Yes)
    }
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
/// ```
/// # use renox::prelude::*;
/// # async fn demo(db: Db) -> Result {
/// let rows = renox::db::sql("SELECT nama FROM produk WHERE harga < ?")
///     .bind(20_000)
///     .fetch_all(&db)
///     .await?;
/// let nama: String = rows[0].try_get("nama")?;
///
/// let total: i64 = renox::db::sql("SELECT COUNT(*) FROM produk").scalar(&db).await?;
/// # let _ = (nama, total); Ok(()) }
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

    /// Runs the statement and returns every row.
    pub async fn fetch_all<'c>(self, db: impl Executor<'c>) -> Result<Vec<Row>, DbError> {
        let Self { sql, args } = self;
        super::query_log::record(&sql);
        let conn = db.into_conn();
        let key = conn.key();
        let args = seal(key.as_deref(), args)?;
        let rows: Vec<Row> = dispatch!(conn, |build, exec| build(sql, args)
            .fetch_all(exec)
            .await
            .map(|rows| rows.into_iter().map(Row::from).collect()))?;
        Ok(rows
            .into_iter()
            .map(|row| row.with_key(key.clone()))
            .collect())
    }

    /// Runs the statement and returns the first row, if any.
    pub async fn fetch_optional<'c>(self, db: impl Executor<'c>) -> Result<Option<Row>, DbError> {
        let Self { sql, args } = self;
        super::query_log::record(&sql);
        let conn = db.into_conn();
        let key = conn.key();
        let args = seal(key.as_deref(), args)?;
        let row: Option<Row> = dispatch!(conn, |build, exec| build(sql, args)
            .fetch_optional(exec)
            .await
            .map(|row| row.map(Row::from)))?;
        Ok(row.map(|row| row.with_key(key)))
    }

    /// The first row; an error if there is none.
    pub async fn fetch_one<'c>(self, db: impl Executor<'c>) -> Result<Row, DbError> {
        self.fetch_optional(db)
            .await?
            .ok_or_else(|| DbError::from(sqlx::Error::RowNotFound))
    }

    /// Runs the statement and returns the number of rows it changed.
    pub async fn execute<'c>(self, db: impl Executor<'c>) -> Result<u64, DbError> {
        let Self { sql, args } = self;
        super::query_log::record(&sql);
        let conn = db.into_conn();
        let args = seal(conn.key().as_deref(), args)?;
        dispatch!(conn, |build, exec| build(sql, args)
            .execute(exec)
            .await
            .map(|done| done.rows_affected()))
    }

    /// Every row, read into `T`: a model, a `#[derive(FromRow)]` struct or a
    /// tuple; see [`super::FromRow`].
    pub async fn fetch_as<'c, T: super::FromRow>(
        self,
        db: impl Executor<'c>,
    ) -> Result<Vec<T>, DbError> {
        self.fetch_all(db).await?.iter().map(T::from_row).collect()
    }

    /// The first row read into `T`, or `None` if there is no row.
    pub async fn fetch_optional_as<'c, T: super::FromRow>(
        self,
        db: impl Executor<'c>,
    ) -> Result<Option<T>, DbError> {
        self.fetch_optional(db)
            .await?
            .as_ref()
            .map(T::from_row)
            .transpose()
    }

    /// The first row read into `T`; an error if there is none.
    pub async fn fetch_one_as<'c, T: super::FromRow>(
        self,
        db: impl Executor<'c>,
    ) -> Result<T, DbError> {
        T::from_row(&self.fetch_one(db).await?)
    }

    /// The first column of the first row; an error if there is no row.
    pub async fn scalar<'c, T: FromDb>(self, db: impl Executor<'c>) -> Result<T, DbError> {
        self.fetch_one(db).await?.try_get(0)
    }

    /// The first column of the first row, or `None` if there is no row.
    pub async fn scalar_optional<'c, T: FromDb>(
        self,
        db: impl Executor<'c>,
    ) -> Result<Option<T>, DbError> {
        self.fetch_optional(db)
            .await?
            .map(|row| row.try_get(0))
            .transpose()
    }

    /// The first column of every row.
    pub async fn scalars<'c, T: FromDb>(self, db: impl Executor<'c>) -> Result<Vec<T>, DbError> {
        self.fetch_all(db)
            .await?
            .iter()
            .map(|row| row.try_get(0))
            .collect()
    }
}

/// Runs SQL that may hold several statements and no parameters, e.g. a
/// migration file. Returns the number of rows changed.
pub(crate) async fn script<'c>(db: impl Executor<'c>, sql: &str) -> Result<u64, DbError> {
    let sql = sql.to_owned();
    dispatch!(db.into_conn(), |build, exec| sqlx::raw_sql(AssertSqlSafe(
        sql
    ))
    .execute(exec)
    .await
    .map(|done| done.rows_affected()))
}

/// One result row. Read columns by name or position with [`Row::try_get`].
pub struct Row(pub(crate) RowInner, Option<Arc<Key>>);

pub(crate) enum RowInner {
    Sqlite(SqliteRow),
    #[cfg(feature = "postgres")]
    Postgres(PgRow),
}

impl From<SqliteRow> for Row {
    fn from(row: SqliteRow) -> Self {
        Self(RowInner::Sqlite(row), None)
    }
}

#[cfg(feature = "postgres")]
impl From<PgRow> for Row {
    fn from(row: PgRow) -> Self {
        Self(RowInner::Postgres(row), None)
    }
}

impl Row {
    fn with_key(mut self, key: Option<Arc<Key>>) -> Self {
        self.1 = key;
        self
    }

    /// A column's value, by name (`"nama"`) or position (`0`).
    pub fn try_get<T: FromDb>(&self, index: impl RowIndex) -> Result<T, DbError> {
        super::encrypted::reading(self.1.as_ref(), || match &self.0 {
            RowInner::Sqlite(row) => Ok(row.try_get(index)?),
            #[cfg(feature = "postgres")]
            RowInner::Postgres(row) => Ok(row.try_get(index)?),
        })
    }

    /// A column's value as JSON, whatever its type; `null` when it can't be
    /// read as a number, boolean, text, timestamp or JSON.
    pub(crate) fn json(&self, column: &str) -> serde_json::Value {
        use serde_json::Value;
        if let Ok(v) = self.try_get::<Option<i64>>(column) {
            return v.map_or(Value::Null, Value::from);
        }
        if let Ok(v) = self.try_get::<Option<i32>>(column) {
            return v.map_or(Value::Null, Value::from);
        }
        if let Ok(v) = self.try_get::<Option<f64>>(column) {
            return v.map_or(Value::Null, Value::from);
        }
        if let Ok(v) = self.try_get::<Option<bool>>(column) {
            return v.map_or(Value::Null, Value::from);
        }
        if let Ok(v) = self.try_get::<Option<chrono::DateTime<chrono::Utc>>>(column) {
            return v.map_or(Value::Null, |d| Value::from(d.to_rfc3339()));
        }
        if let Ok(v) = self.try_get::<Option<chrono::NaiveDateTime>>(column) {
            return v.map_or(Value::Null, |d| {
                Value::from(d.format("%Y-%m-%dT%H:%M:%S").to_string())
            });
        }
        if let Ok(v) = self.try_get::<Option<chrono::NaiveDate>>(column) {
            return v.map_or(Value::Null, |d| Value::from(d.to_string()));
        }
        if let Ok(v) = self.try_get::<Option<String>>(column) {
            return v.map_or(Value::Null, Value::from);
        }
        if let Ok(v) = self.try_get::<Option<serde_json::Value>>(column) {
            return v.unwrap_or(Value::Null);
        }
        Value::Null
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
