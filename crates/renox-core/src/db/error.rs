use std::fmt;

/// A database error: a query that failed, a lost connection, a broken
/// constraint. Converts into [`crate::Error`] with `?`.
///
/// ```
/// # use renox::prelude::*;
/// # async fn demo(db: Db) -> Result {
/// let inserted = renox::db::sql("INSERT INTO tags (name) VALUES (?)").bind("kopi").execute(&db).await;
/// match inserted {
///     Err(err) if err.is_unique_violation() => { /* already there */ }
///     other => { other?; }
/// }
/// # Ok(()) }
/// ```
pub struct DbError(sqlx::Error);

impl DbError {
    /// A `UNIQUE` constraint (or primary key) was violated.
    pub fn is_unique_violation(&self) -> bool {
        matches!(&self.0, sqlx::Error::Database(db) if db.is_unique_violation())
    }

    /// A `REFERENCES` constraint was violated.
    pub fn is_foreign_key_violation(&self) -> bool {
        matches!(&self.0, sqlx::Error::Database(db) if db.is_foreign_key_violation())
    }

    /// `fetch_one` or `scalar` found no row.
    pub fn is_row_not_found(&self) -> bool {
        matches!(self.0, sqlx::Error::RowNotFound)
    }

    /// Another transaction got in the way and trying again may work:
    /// SQLite's "database is locked" (busy), PostgreSQL's serialization
    /// failure or deadlock. `Db::transaction_retrying` retries on these.
    pub fn is_retryable(&self) -> bool {
        let sqlx::Error::Database(db) = &self.0 else {
            return false;
        };
        match db.code() {
            Some(code) if code == "40001" || code == "40P01" => true,
            // SQLite's primary codes 5 (BUSY) and 6 (LOCKED), extended or not.
            Some(code) => code.parse::<i32>().is_ok_and(|c| matches!(c & 0xff, 5 | 6)),
            None => false,
        }
    }

    /// No connection was free within `DATABASE_ACQUIRE_TIMEOUT`.
    pub fn is_timeout(&self) -> bool {
        matches!(self.0, sqlx::Error::PoolTimedOut)
    }

    /// The underlying sqlx error, for what Renox doesn't cover. sqlx's
    /// version isn't part of Renox's stability promise.
    pub fn sqlx(&self) -> &sqlx::Error {
        &self.0
    }
}

impl From<sqlx::Error> for DbError {
    fn from(err: sqlx::Error) -> Self {
        Self(err)
    }
}

impl fmt::Display for DbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl fmt::Debug for DbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}

impl std::error::Error for DbError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        // The wrapped error's own cause; its message is already ours.
        self.0.source()
    }
}
