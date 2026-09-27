//! SQLite access: the connection pool, models, queries, pagination,
//! migrations and factories.

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

pub use factory::Factory;
pub(crate) use migrate::Migrator;
pub use migrate::{Migration, MigrationStatus};
pub use model::Model;
pub use paginate::{Page, Paginated};
pub use query::Query;
pub use sqlx::sqlite::SqliteRow;
pub use value::{DbValue, ToDbValue};

use crate::{AppState, Config};

/// The database pool. Take it in a handler with `State(db): State<Db>`.
pub type Db = sqlx::SqlitePool;

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

/// Opens the pool for `DATABASE_URL`, creating the file and its directory if
/// needed. File databases use WAL mode; every connection enforces foreign keys.
pub(crate) async fn connect(config: &Config) -> anyhow::Result<Db> {
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

/// Quotes an identifier for SQLite, e.g. `order` -> `"order"`.
pub(crate) fn quote(ident: &str) -> String {
    format!("\"{}\"", ident.replace('"', "\"\""))
}
