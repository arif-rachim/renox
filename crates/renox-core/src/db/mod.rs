//! Database access (SQLite, or PostgreSQL with the `postgres` feature): the
//! connection pool, raw SQL, models, queries, pagination, migrations and
//! factories.

mod conn;
mod factory;
mod migrate;
mod model;
mod paginate;
mod query;
mod value;

use std::str::FromStr;
use std::time::Duration;

use anyhow::Context;
use axum::extract::FromRef;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};

#[doc(hidden)]
pub use conn::{Conn, bounds};
pub use conn::{Db, Dialect, Executor, FromDb, Row, RowIndex, Sql, Transaction, sql};
pub(crate) use conn::{RowInner, script};
pub use factory::Factory;
pub(crate) use migrate::Migrator;
pub use migrate::{Migration, MigrationStatus};
pub use model::Model;
pub use paginate::{Page, Paginated};
pub use query::Query;
pub use value::{DbValue, ToDbValue};

use crate::{AppState, Config};

/// The timestamp type for `created_at`, `updated_at` and `deleted_at`.
pub type DateTime = chrono::DateTime<chrono::Utc>;

/// The current time, as stored in timestamps.
pub fn now() -> DateTime {
    chrono::Utc::now()
}

impl FromRef<AppState> for Db {
    fn from_ref(state: &AppState) -> Self {
        state.db.clone()
    }
}

/// Opens the pool for `DATABASE_URL`. `postgres://` URLs need the `postgres`
/// feature; anything else is SQLite.
pub(crate) async fn connect(config: &Config) -> anyhow::Result<Db> {
    let url = &config.database_url;
    if url.starts_with("postgres://") || url.starts_with("postgresql://") {
        return connect_postgres(config).await;
    }
    connect_sqlite(config).await.map(Db::from)
}

#[cfg(feature = "postgres")]
async fn connect_postgres(config: &Config) -> anyhow::Result<Db> {
    let url = &config.database_url;
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(config.database_pool_size)
        .connect(url)
        .await
        .with_context(|| format!("could not connect to the database at `{}`", redact(url)))?;
    Ok(Db::from(pool))
}

#[cfg(not(feature = "postgres"))]
async fn connect_postgres(_config: &Config) -> anyhow::Result<Db> {
    anyhow::bail!(
        "DATABASE_URL points to PostgreSQL, but this build has no PostgreSQL support; \
         enable the `postgres` feature of `renox`"
    )
}

/// The URL with its password hidden, for error messages.
#[cfg_attr(not(feature = "postgres"), allow(dead_code))]
fn redact(url: &str) -> String {
    let Some((scheme, rest)) = url.split_once("://") else {
        return url.to_owned();
    };
    match rest.split_once('@') {
        Some((user, host)) => {
            let user = user.split_once(':').map_or(user, |(name, _)| name);
            format!("{scheme}://{user}:***@{host}")
        }
        None => url.to_owned(),
    }
}

/// Opens a SQLite pool, creating the file and its directory if needed. File
/// databases use WAL mode; every connection enforces foreign keys.
async fn connect_sqlite(config: &Config) -> anyhow::Result<sqlx::SqlitePool> {
    let url = &config.database_url;
    let in_memory = url.contains(":memory:") || url.contains("mode=memory");

    let mut options = SqliteConnectOptions::from_str(url)
        .with_context(|| format!("DATABASE_URL `{url}` is not a valid SQLite URL"))?
        .create_if_missing(true)
        .foreign_keys(true)
        .busy_timeout(Duration::from_secs(5));
    if !in_memory {
        options = options
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal);
        if let Some(dir) = options
            .get_filename()
            .parent()
            .filter(|d| !d.as_os_str().is_empty())
        {
            std::fs::create_dir_all(dir)
                .with_context(|| format!("could not create {}", dir.display()))?;
        }
    }

    // Each connection to `:memory:` is a separate database, so keep exactly one.
    let pool = if in_memory {
        SqlitePoolOptions::new()
            .max_connections(1)
            .min_connections(1)
            .idle_timeout(None)
            .max_lifetime(None)
    } else {
        SqlitePoolOptions::new().max_connections(config.database_pool_size)
    };
    pool.connect_with(options)
        .await
        .with_context(|| format!("could not open the database at `{url}`"))
}

/// Quotes an identifier (the same on SQLite and PostgreSQL), e.g. `order` -> `"order"`.
pub(crate) fn quote(ident: &str) -> String {
    format!("\"{}\"", ident.replace('"', "\"\""))
}
