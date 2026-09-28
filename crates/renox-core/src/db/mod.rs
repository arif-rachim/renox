//! Database access (SQLite, or PostgreSQL with the `postgres` feature): the
//! connection pool, raw SQL, models, queries, pagination, migrations and
//! factories.

mod conn;
mod error;
mod factory;
mod from_row;
mod json;
mod migrate;
mod model;
mod paginate;
mod query;
pub mod relations;
mod value;

use std::str::FromStr;
use std::time::Duration;

use anyhow::Context;
use axum::extract::FromRef;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};

#[doc(hidden)]
pub use conn::{Conn, bounds};
pub use conn::{Db, Dialect, Executor, FromDb, Row, RowIndex, Sql, Transaction, sql};
pub(crate) use conn::{RowInner, SchemaEpoch, script};
pub use error::DbError;
pub use factory::Factory;
pub use from_row::FromRow;
pub use json::Json;
pub use migrate::{Migration, MigrationStatus, Scripts};
pub(crate) use migrate::{Migrator, framework_migration};
pub use model::Model;
pub use paginate::{Page, Paginated};
pub use query::{Number, Query};
/// sqlx, for what Renox's own API doesn't cover: `Db::sqlite()`, `Db::postgres()`,
/// `Row::sqlite()`, `Row::postgres()` and `DbError::sqlx()` hand out its types.
/// sqlx may move to a new version in a minor Renox release; see docs/stability.md.
pub use sqlx;
pub use value::{DbValue, ToDbValue};

use crate::{AppState, Config};

/// The timestamp type for `created_at`, `updated_at` and `deleted_at`.
pub type DateTime = chrono::DateTime<chrono::Utc>;

/// The current time, as stored in timestamps: to the microsecond, which is
/// what PostgreSQL keeps, so a saved model equals the same row read back.
pub fn now() -> DateTime {
    use chrono::SubsecRound;
    chrono::Utc::now().trunc_subsecs(6)
}

impl FromRef<AppState> for Db {
    fn from_ref(state: &AppState) -> Self {
        state.db.clone()
    }
}

/// Opens the pool for `DATABASE_URL`. `postgres://` URLs need the `postgres`
/// feature; anything else is SQLite.
///
/// An in-memory SQLite database (what tests use) is swapped for a fresh
/// schema in `TEST_DATABASE_URL` when that is set to a PostgreSQL URL (in the
/// environment or in `.env`), so a test suite can run against PostgreSQL
/// unchanged.
pub(crate) async fn connect(config: &Config) -> anyhow::Result<Db> {
    let url = &config.database_url;
    if config.database_pool_size == 0 {
        anyhow::bail!("DATABASE_POOL_SIZE must be at least 1");
    }
    if is_postgres(url) {
        return connect_postgres(url, config, false).await;
    }
    if !url.starts_with("sqlite:") {
        // A typo such as `postgress://` must not become a SQLite file.
        anyhow::bail!(
            "DATABASE_URL `{}` must start with sqlite:, postgres:// or postgresql://",
            redact(url)
        );
    }
    if is_memory(url)
        && let Some(test_url) = test_database_url().filter(|u| is_postgres(u))
    {
        return connect_postgres(&test_url, config, true).await;
    }
    let schema = SchemaEpoch::default();
    let pool = connect_sqlite(config, schema.clone()).await?;
    Ok(Db::from_sqlite(pool, schema))
}

/// `TEST_DATABASE_URL` from the environment, else from `.env` in the current
/// directory (without loading the rest of `.env` into the environment).
fn test_database_url() -> Option<String> {
    std::env::var("TEST_DATABASE_URL").ok().or_else(|| {
        dotenvy::from_path_iter(".env")
            .ok()?
            .flatten()
            .find(|(key, _)| key == "TEST_DATABASE_URL")
            .map(|(_, url)| url)
    })
}

fn is_postgres(url: &str) -> bool {
    url.starts_with("postgres://") || url.starts_with("postgresql://")
}

fn is_memory(url: &str) -> bool {
    url.contains(":memory:") || url.contains("mode=memory")
}

/// Connects to PostgreSQL; with `fresh_schema`, in a new, empty schema of its
/// own (named `renox_test_…`, left behind for inspection).
#[cfg(feature = "postgres")]
async fn connect_postgres(url: &str, config: &Config, fresh_schema: bool) -> anyhow::Result<Db> {
    use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
    use sqlx::{ConnectOptions, Connection};

    let failed = || format!("could not connect to the database at `{}`", redact(url));
    let mut options = PgConnectOptions::from_str(url).with_context(failed)?;
    if fresh_schema {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let schema = format!(
            "renox_test_{}_{}_{nanos}",
            std::process::id(),
            COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        );
        let mut conn = options.connect().await.with_context(failed)?;
        sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
            "CREATE SCHEMA {}",
            quote(&schema)
        )))
        .execute(&mut conn)
        .await
        .context("could not create a schema for the test")?;
        conn.close().await.ok();
        options = options.options([("search_path", schema)]);
    }
    if let Some(limit) = config.database_statement_timeout {
        options = options.options([("statement_timeout", format!("{}ms", limit.as_millis()))]);
    }
    let pool_size = config.database_pool_size;
    // Test suites boot many apps at once; a few connections each keeps them
    // under the server's limit (100 by default).
    let pool_size = if fresh_schema {
        pool_size.min(3)
    } else {
        pool_size
    };
    let schema = SchemaEpoch::default();
    let epoch = schema.clone();
    let pool = PgPoolOptions::new()
        .max_connections(pool_size)
        .acquire_timeout(config.database_acquire_timeout)
        .before_acquire(move |_, meta| {
            let fresh = !epoch.is_stale(meta.age);
            Box::pin(async move { Ok(fresh) })
        })
        .connect_with(options)
        .await
        .with_context(failed)?;
    Ok(Db::from_postgres(pool, schema))
}

#[cfg(not(feature = "postgres"))]
async fn connect_postgres(_url: &str, _config: &Config, _fresh_schema: bool) -> anyhow::Result<Db> {
    anyhow::bail!(
        "the database URL points to PostgreSQL, but this build has no PostgreSQL support; \
         enable the `postgres` feature of `renox`"
    )
}

/// The URL with its password hidden, for error messages.
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
async fn connect_sqlite(config: &Config, schema: SchemaEpoch) -> anyhow::Result<sqlx::SqlitePool> {
    let url = &config.database_url;
    let in_memory = is_memory(url);

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
        // Connections opened before a migration would read the old schema.
        SqlitePoolOptions::new()
            .max_connections(config.database_pool_size)
            .before_acquire(move |_, meta| {
                let fresh = !schema.is_stale(meta.age);
                Box::pin(async move { Ok(fresh) })
            })
    }
    .acquire_timeout(if in_memory {
        // The one connection is busy for a moment at most, unless a task
        // waits for itself (a query through `&db` while its own transaction
        // is open): fail such a test fast instead of letting it hang.
        config.database_acquire_timeout.min(Duration::from_secs(2))
    } else {
        config.database_acquire_timeout
    });
    pool.connect_with(options)
        .await
        .with_context(|| format!("could not open the database at `{url}`"))
}

/// Quotes an identifier (the same on SQLite and PostgreSQL), e.g. `order` -> `"order"`.
pub(crate) fn quote(ident: &str) -> String {
    format!("\"{}\"", ident.replace('"', "\"\""))
}

/// Implements sqlx's traits for a type stored as text through `Display` and
/// `FromStr` (what `#[derive(DbEnum)]` generates), for every database this
/// build of Renox supports. Chosen when renox-core is compiled, so apps don't
/// need to know which features it has.
#[doc(hidden)]
#[cfg(not(feature = "postgres"))]
#[macro_export]
macro_rules! __db_text_type {
    ($t:ty) => {
        $crate::__db_text_type_for!($t, $crate::__sqlx::sqlite::Sqlite);
    };
}

#[doc(hidden)]
#[cfg(feature = "postgres")]
#[macro_export]
macro_rules! __db_text_type {
    ($t:ty) => {
        $crate::__db_text_type_for!($t, $crate::__sqlx::sqlite::Sqlite);
        $crate::__db_text_type_for!($t, $crate::__sqlx::postgres::Postgres);
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __db_text_type_for {
    ($t:ty, $db:ty) => {
        impl $crate::__sqlx::Type<$db> for $t {
            fn type_info() -> <$db as $crate::__sqlx::Database>::TypeInfo {
                <::std::string::String as $crate::__sqlx::Type<$db>>::type_info()
            }

            fn compatible(ty: &<$db as $crate::__sqlx::Database>::TypeInfo) -> bool {
                <::std::string::String as $crate::__sqlx::Type<$db>>::compatible(ty)
            }
        }

        impl<'r> $crate::__sqlx::Decode<'r, $db> for $t {
            fn decode(
                value: <$db as $crate::__sqlx::Database>::ValueRef<'r>,
            ) -> ::std::result::Result<Self, $crate::__sqlx::error::BoxDynError> {
                let text = <::std::string::String as $crate::__sqlx::Decode<$db>>::decode(value)?;
                text.parse::<$t>().map_err(::std::convert::Into::into)
            }
        }
    };
}
