use std::collections::{HashMap, HashSet};

use super::{Db, Dialect, now, quote, script, sql};
use anyhow::{Context, anyhow, bail};

const TABLE: &str = "renox_migrations";

/// One of the framework's migrations from `crates/renox-core/migrations/DIR/`:
/// `NAME.up.sql` (SQLite), `NAME.postgres.up.sql` and a shared `NAME.down.sql`.
macro_rules! framework_migration {
    ($dir:literal, $name:literal) => {
        $crate::db::Migration {
            name: $name,
            up: include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/migrations/",
                $dir,
                "/",
                $name,
                ".up.sql"
            )),
            down: Some(include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/migrations/",
                $dir,
                "/",
                $name,
                ".down.sql"
            ))),
            sqlite: None,
            postgres: Some($crate::db::Scripts {
                up: include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/migrations/",
                    $dir,
                    "/",
                    $name,
                    ".postgres.up.sql"
                )),
                down: None,
            }),
        }
    };
}
pub(crate) use framework_migration;

/// One migration: SQL to apply it and, optionally, SQL to undo it.
///
/// Usually generated from `migrations/*.up.sql` and `*.down.sql` by
/// `renox::migrations!()`. Names start with a timestamp and run in name order.
///
/// When SQL differs between databases, `NAME.postgres.up.sql` (and
/// `.postgres.down.sql`) or `NAME.sqlite.up.sql` replace the plain files on
/// that database; the plain file may then be left out if both are given.
#[derive(Debug, Clone, Copy)]
pub struct Migration {
    /// The migration's name, e.g. `20260101000000_create_products_table`; names sort the runs.
    pub name: &'static str,
    /// SQL for every database without its own version (may be empty when
    /// each database has one).
    pub up: &'static str,
    /// SQL that undoes `up` on databases without their own version; `None`: irreversible.
    pub down: Option<&'static str>,
    /// Used instead of `up`/`down` on SQLite.
    pub sqlite: Option<Scripts>,
    /// Used instead of `up`/`down` on PostgreSQL.
    pub postgres: Option<Scripts>,
}

/// One database's own version of a migration.
#[derive(Debug, Clone, Copy)]
pub struct Scripts {
    /// The SQL that applies the migration on this database.
    pub up: &'static str,
    /// Falls back to the migration's plain `down` when `None`.
    pub down: Option<&'static str>,
}

impl Migration {
    /// A migration with the same SQL on every database.
    pub const fn new(name: &'static str, up: &'static str, down: Option<&'static str>) -> Self {
        Self {
            name,
            up,
            down,
            sqlite: None,
            postgres: None,
        }
    }

    fn own(&self, dialect: Dialect) -> Option<&Scripts> {
        match dialect {
            Dialect::Sqlite => self.sqlite.as_ref(),
            Dialect::Postgres => self.postgres.as_ref(),
        }
    }

    /// The SQL that applies this migration on `dialect`.
    pub fn up_for(&self, dialect: Dialect) -> &'static str {
        self.own(dialect).map_or(self.up, |own| own.up)
    }

    /// The SQL that undoes this migration on `dialect`, if any.
    pub fn down_for(&self, dialect: Dialect) -> Option<&'static str> {
        self.own(dialect).and_then(|own| own.down).or(self.down)
    }
}

/// Whether a migration has run, and in which batch.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct MigrationStatus {
    /// The migration's name.
    pub name: String,
    /// The batch it ran in; `None` when it hasn't run.
    pub batch: Option<i64>,
    /// Applied, but its file is no longer registered.
    pub missing: bool,
    /// Applied, and its SQL has changed since (editing an applied migration
    /// does nothing; add a new one instead).
    pub changed: bool,
}

/// A line in a migration that makes it run outside a transaction.
const NO_TRANSACTION: &str = "-- renox:no-transaction";

/// Whether `sql` can't run inside the transaction migrations get: it says
/// so, manages its own (`BEGIN` … `COMMIT`), or uses `CONCURRENTLY`, which
/// PostgreSQL refuses in a transaction.
fn runs_outside_transaction(sql: &str) -> bool {
    if sql.lines().any(|line| line.trim() == NO_TRANSACTION) {
        return true;
    }
    let upper = sql.to_ascii_uppercase();
    upper.contains(" CONCURRENTLY ")
        || upper.split(';').any(|statement| {
            let statement = statement.trim();
            statement == "BEGIN"
                || statement.starts_with("BEGIN TRANSACTION")
                || statement.starts_with("BEGIN IMMEDIATE")
        })
}

/// Runs `sql` outside the migration transaction. PostgreSQL runs a
/// multi-statement script as one implicit transaction, which `CONCURRENTLY`
/// refuses, so there such a script runs one statement at a time.
async fn run_each(db: &Db, sql: &str) -> Result<(), super::DbError> {
    if db.dialect() == Dialect::Postgres && sql.to_ascii_uppercase().contains(" CONCURRENTLY ") {
        for statement in statements(sql) {
            script(db, statement).await?;
        }
        Ok(())
    } else {
        script(db, sql).await.map(|_| ())
    }
}

/// Splits SQL on `;`, except inside quotes, comments and `$tag$` bodies.
fn statements(sql: &str) -> Vec<&str> {
    let bytes = sql.as_bytes();
    let mut out = Vec::new();
    let (mut start, mut i) = (0, 0);
    while i < bytes.len() {
        match bytes[i] {
            quote @ (b'\'' | b'"') => {
                i += 1;
                while i < bytes.len() && bytes[i] != quote {
                    i += 1;
                }
            }
            b'-' if bytes.get(i + 1) == Some(&b'-') => {
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if bytes.get(i + 1) == Some(&b'*') => {
                i = sql[i + 2..]
                    .find("*/")
                    .map_or(bytes.len(), |end| i + 2 + end + 1);
            }
            b'$' => {
                let tag_end = sql[i + 1..]
                    .find(|c: char| !(c.is_alphanumeric() || c == '_'))
                    .map(|n| i + 1 + n);
                if let Some(end) = tag_end.filter(|&end| bytes[end] == b'$') {
                    let tag = &sql[i..=end];
                    i = sql[end + 1..]
                        .find(tag)
                        .map_or(bytes.len(), |close| end + 1 + close + tag.len() - 1);
                }
            }
            b';' => {
                out.push(&sql[start..i]);
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    out.push(&sql[start..]);
    out.into_iter()
        .filter(|statement| {
            statement
                .lines()
                .any(|line| !line.trim().is_empty() && !line.trim().starts_with("--"))
        })
        .collect()
}

fn checksum(sql: &str) -> String {
    crate::webhook::sha256_hex(sql)
}

/// Runs migrations in batches, like Laravel: `run` applies everything pending
/// as one batch, `rollback` undoes whole batches, newest first. Runs don't
/// overlap: within a process they wait for each other, and on PostgreSQL an
/// advisory lock makes other processes (replicas deploying at once) wait too.
pub(crate) struct Migrator {
    migrations: Vec<Migration>,
}

/// An applied migration's batch and checksum.
struct Applied {
    batch: i64,
    checksum: Option<String>,
}

/// Held while migrations run; see [`Migrator`].
struct MigrationLock {
    _local: tokio::sync::MutexGuard<'static, ()>,
    #[cfg(feature = "postgres")]
    _postgres: Option<sqlx::postgres::PgConnection>,
}

/// `pg_advisory_lock` key: "renox" + "mig" in ASCII.
#[cfg(feature = "postgres")]
const ADVISORY_KEY: i64 = 0x7265_6e6f_786d_6967;

impl Migrator {
    pub fn new(mut migrations: Vec<Migration>) -> anyhow::Result<Self> {
        migrations.sort_by_key(|m| m.name);
        for pair in migrations.windows(2) {
            if pair[0].name == pair[1].name {
                bail!("migration `{}` is registered twice", pair[0].name);
            }
        }
        Ok(Self { migrations })
    }

    async fn lock(db: &Db) -> anyhow::Result<MigrationLock> {
        static LOCAL: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
        let local = LOCAL.lock().await;
        #[cfg(feature = "postgres")]
        let postgres = match db.postgres() {
            Some(pool) => {
                // Detached: if this future is dropped, closing the connection
                // releases the lock.
                let mut conn = pool.acquire().await?.detach();
                sqlx::query("SELECT pg_advisory_lock($1)")
                    .bind(ADVISORY_KEY)
                    .execute(&mut conn)
                    .await?;
                Some(conn)
            }
            None => None,
        };
        #[cfg(not(feature = "postgres"))]
        let _ = db;
        Ok(MigrationLock {
            _local: local,
            #[cfg(feature = "postgres")]
            _postgres: postgres,
        })
    }

    async fn ensure_table(db: &Db) -> anyhow::Result<()> {
        sql(format!(
            "CREATE TABLE IF NOT EXISTS {TABLE} (
                name TEXT PRIMARY KEY NOT NULL,
                batch BIGINT NOT NULL,
                applied_at TEXT NOT NULL,
                checksum TEXT
            )"
        ))
        .execute(db)
        .await?;
        // Tables made before checksums were kept.
        if sql(format!("SELECT checksum FROM {TABLE} WHERE 1 = 0"))
            .execute(db)
            .await
            .is_err()
        {
            sql(format!("ALTER TABLE {TABLE} ADD COLUMN checksum TEXT"))
                .execute(db)
                .await?;
            db.schema_changed();
        }
        Ok(())
    }

    async fn applied(db: &Db) -> anyhow::Result<HashMap<String, Applied>> {
        Self::ensure_table(db).await?;
        let rows = sql(format!("SELECT name, batch, checksum FROM {TABLE}"))
            .fetch_all(db)
            .await?;
        rows.iter()
            .map(|row| {
                Ok((
                    row.try_get("name")?,
                    Applied {
                        batch: row.try_get("batch")?,
                        checksum: row.try_get("checksum")?,
                    },
                ))
            })
            .collect()
    }

    /// Applies pending migrations as a new batch; returns their names.
    pub async fn run(&self, db: &Db) -> anyhow::Result<Vec<&'static str>> {
        let _lock = Self::lock(db).await?;
        self.run_locked(db).await
    }

    async fn run_locked(&self, db: &Db) -> anyhow::Result<Vec<&'static str>> {
        let dialect = db.dialect();
        let applied = Self::applied(db).await?;
        for migration in &self.migrations {
            let changed = applied.get(migration.name).is_some_and(|a| {
                a.checksum
                    .as_ref()
                    .is_some_and(|sum| *sum != checksum(migration.up_for(dialect)))
            });
            if changed {
                tracing::warn!(
                    migration = migration.name,
                    "an applied migration was edited; the change won't run (add a new migration)"
                );
            }
        }
        let batch = applied.values().map(|a| a.batch).max().unwrap_or(0) + 1;
        let mut done = Vec::new();
        let result = self.run_pending(db, &applied, batch, &mut done).await;
        // Once per batch: every change reopens the pool's connections.
        if !done.is_empty() {
            db.schema_changed();
        }
        result.map(|()| done)
    }

    async fn run_pending(
        &self,
        db: &Db,
        applied: &HashMap<String, Applied>,
        batch: i64,
        done: &mut Vec<&'static str>,
    ) -> anyhow::Result<()> {
        let dialect = db.dialect();

        for migration in self
            .migrations
            .iter()
            .filter(|m| !applied.contains_key(m.name))
        {
            let up = migration.up_for(dialect);
            let failed = || format!("migration `{}` failed", migration.name);
            let record = sql(format!(
                "INSERT INTO {TABLE} (name, batch, applied_at, checksum) VALUES (?, ?, ?, ?)"
            ))
            .bind(migration.name)
            .bind(batch)
            .bind(now().to_rfc3339())
            .bind(checksum(up));
            if runs_outside_transaction(up) {
                run_each(db, up).await.with_context(failed)?;
                record.execute(db).await?;
            } else {
                // IMMEDIATE on SQLite: another process can't slip in between
                // the check and the insert.
                let mut tx = db.begin_immediate().await?;
                let already: i64 = sql(format!("SELECT COUNT(*) FROM {TABLE} WHERE name = ?"))
                    .bind(migration.name)
                    .scalar(&mut tx)
                    .await?;
                if already > 0 {
                    continue;
                }
                script(&mut tx, up).await.with_context(failed)?;
                record.execute(&mut tx).await?;
                tx.commit().await?;
            }
            done.push(migration.name);
        }
        Ok(())
    }

    /// Undoes the last `batches` batches, newest migration first. Nothing is
    /// undone unless every migration in them can be. A migration that is no
    /// longer registered is forgotten, with a warning, and its changes stay.
    pub async fn rollback(&self, db: &Db, batches: u32) -> anyhow::Result<Vec<String>> {
        let _lock = Self::lock(db).await?;
        let dialect = db.dialect();
        let applied = Self::applied(db).await?;
        let mut numbers: Vec<i64> = applied
            .values()
            .map(|a| a.batch)
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        numbers.sort_unstable_by(|a, b| b.cmp(a));
        let targets: HashSet<i64> = numbers.into_iter().take(batches as usize).collect();

        let mut names: Vec<&String> = applied
            .iter()
            .filter(|(_, a)| targets.contains(&a.batch))
            .map(|(name, _)| name)
            .collect();
        names.sort_unstable_by(|a, b| b.cmp(a));

        // Check every step before taking any.
        let mut steps = Vec::new();
        for name in names {
            let down = match self.migrations.iter().find(|m| m.name == name) {
                None => None,
                Some(migration) => Some(migration.down_for(dialect).ok_or_else(|| {
                    anyhow!(
                        "migration `{name}` has no .down.sql, so its batch can't be rolled back; \
                         nothing was rolled back"
                    )
                })?),
            };
            steps.push((name, down));
        }

        let mut done = Vec::new();
        for (name, down) in steps {
            let forget = sql(format!("DELETE FROM {TABLE} WHERE name = ?")).bind(name);
            match down {
                None => {
                    tracing::warn!(
                        migration = %name,
                        "not registered any more; forgotten without undoing its changes"
                    );
                    forget.execute(db).await?;
                }
                Some(down) if runs_outside_transaction(down) => {
                    run_each(db, down)
                        .await
                        .with_context(|| format!("rolling back `{name}` failed"))?;
                    forget.execute(db).await?;
                }
                Some(down) => {
                    let mut tx = db.begin().await?;
                    script(&mut tx, down)
                        .await
                        .with_context(|| format!("rolling back `{name}` failed"))?;
                    forget.execute(&mut tx).await?;
                    tx.commit().await?;
                }
            }
            done.push(name.clone());
            // Undone even if a later step fails: mark each.
            db.schema_changed();
        }
        Ok(done)
    }

    /// Drops everything the migrations made, then runs all migrations.
    pub async fn fresh(&self, db: &Db) -> anyhow::Result<Vec<&'static str>> {
        let _lock = Self::lock(db).await?;
        if let Some(pool) = db.sqlite() {
            drop_all_sqlite(pool).await?;
        }
        #[cfg(feature = "postgres")]
        if let Some(pool) = db.postgres() {
            drop_all_postgres(pool).await?;
        }
        db.schema_changed();
        self.run_locked(db).await
    }

    /// Every registered migration and whether it has run, then applied
    /// migrations that are no longer registered.
    pub async fn status(&self, db: &Db) -> anyhow::Result<Vec<MigrationStatus>> {
        let dialect = db.dialect();
        let applied = Self::applied(db).await?;
        let mut status: Vec<MigrationStatus> = self
            .migrations
            .iter()
            .map(|m| {
                let found = applied.get(m.name);
                MigrationStatus {
                    name: m.name.to_owned(),
                    batch: found.map(|a| a.batch),
                    missing: false,
                    changed: found.is_some_and(|a| {
                        a.checksum
                            .as_ref()
                            .is_some_and(|sum| *sum != checksum(m.up_for(dialect)))
                    }),
                }
            })
            .collect();
        let mut missing: Vec<MigrationStatus> = applied
            .iter()
            .filter(|(name, _)| !self.migrations.iter().any(|m| m.name == name.as_str()))
            .map(|(name, a)| MigrationStatus {
                name: name.clone(),
                batch: Some(a.batch),
                missing: true,
                changed: false,
            })
            .collect();
        missing.sort_by(|a, b| a.name.cmp(&b.name));
        status.extend(missing);
        Ok(status)
    }
}

/// Drops every table and view on one connection, with foreign keys off so
/// the order doesn't matter.
async fn drop_all_sqlite(pool: &sqlx::SqlitePool) -> anyhow::Result<()> {
    use sqlx::{AssertSqlSafe, Row};

    let mut conn = pool.acquire().await?;
    let objects = sqlx::query(
        "SELECT type, name FROM sqlite_master \
         WHERE type IN ('table', 'view') AND name NOT LIKE 'sqlite_%'",
    )
    .fetch_all(&mut *conn)
    .await?;
    sqlx::query("PRAGMA foreign_keys = OFF")
        .execute(&mut *conn)
        .await?;
    for object in &objects {
        let kind: String = object.try_get("type")?;
        let name: String = object.try_get("name")?;
        let sql = format!("DROP {} IF EXISTS {}", kind.to_uppercase(), quote(&name));
        sqlx::query(AssertSqlSafe(sql)).execute(&mut *conn).await?;
    }
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&mut *conn)
        .await?;
    Ok(())
}

/// Drops everything in the current schema that isn't an extension's:
/// materialized views, views, tables, sequences, functions and types.
/// `CASCADE` takes care of what depends on what.
#[cfg(feature = "postgres")]
async fn drop_all_postgres(pool: &sqlx::PgPool) -> anyhow::Result<()> {
    use sqlx::{AssertSqlSafe, Row};

    // Objects created by an extension are dropped with the extension.
    const NOT_FROM_EXTENSION: &str = "NOT EXISTS (SELECT 1 FROM pg_depend d \
         WHERE d.objid = {oid} AND d.deptype = 'e')";
    let owned = |oid: &str| NOT_FROM_EXTENSION.replace("{oid}", oid);
    let queries = [
        format!(
            "SELECT format('DROP MATERIALIZED VIEW IF EXISTS %I CASCADE', c.relname) \
             FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace \
             WHERE n.nspname = current_schema() AND c.relkind = 'm' AND {}",
            owned("c.oid")
        ),
        format!(
            "SELECT format('DROP VIEW IF EXISTS %I CASCADE', c.relname) \
             FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace \
             WHERE n.nspname = current_schema() AND c.relkind = 'v' AND {}",
            owned("c.oid")
        ),
        format!(
            "SELECT format('DROP TABLE IF EXISTS %I CASCADE', c.relname) \
             FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace \
             WHERE n.nspname = current_schema() AND c.relkind IN ('r', 'p') AND {}",
            owned("c.oid")
        ),
        format!(
            "SELECT format('DROP SEQUENCE IF EXISTS %I CASCADE', c.relname) \
             FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace \
             WHERE n.nspname = current_schema() AND c.relkind = 'S' AND {}",
            owned("c.oid")
        ),
        format!(
            "SELECT format('DROP ROUTINE IF EXISTS %s CASCADE', p.oid::regprocedure) \
             FROM pg_proc p JOIN pg_namespace n ON n.oid = p.pronamespace \
             WHERE n.nspname = current_schema() AND p.prokind IN ('f', 'p') AND {}",
            owned("p.oid")
        ),
        format!(
            "SELECT format(CASE t.typtype WHEN 'd' THEN 'DROP DOMAIN IF EXISTS %I CASCADE' \
                                          ELSE 'DROP TYPE IF EXISTS %I CASCADE' END, t.typname) \
             FROM pg_type t JOIN pg_namespace n ON n.oid = t.typnamespace \
             LEFT JOIN pg_class c ON c.oid = t.typrelid \
             WHERE n.nspname = current_schema() AND t.typtype IN ('e', 'd', 'r', 'c') \
             AND (t.typtype <> 'c' OR c.relkind = 'c') AND {}",
            owned("t.oid")
        ),
    ];
    for query in queries {
        let statements = sqlx::query(AssertSqlSafe(query)).fetch_all(pool).await?;
        for statement in &statements {
            let statement: String = statement.try_get(0)?;
            sqlx::query(AssertSqlSafe(statement)).execute(pool).await?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_statements_outside_quotes_comments_and_bodies() {
        let sql = "-- renox:no-transaction\nCREATE TABLE t (s TEXT DEFAULT ';');\n\
                   /* a; b */ CREATE FUNCTION f() RETURNS INT AS $body$ SELECT 1; $body$ LANGUAGE SQL;\n\
                   CREATE INDEX CONCURRENTLY i ON t (s);\n";
        let parts = statements(sql);
        assert_eq!(parts.len(), 3, "{parts:?}");
        assert!(parts[1].contains("SELECT 1; $body$"));
        assert!(parts[2].trim().starts_with("CREATE INDEX CONCURRENTLY"));
        assert!(runs_outside_transaction(sql));
        assert!(runs_outside_transaction(
            "BEGIN; CREATE TABLE a (id INT); COMMIT;"
        ));
        assert!(!runs_outside_transaction(
            "CREATE TABLE begin_log (id INT);"
        ));
    }

    /// Connections that read the schema before a migration aren't reused
    /// after it: with them, `SELECT *` on the altered table panicked inside
    /// sqlx-sqlite (a flaky macOS CI failure in M16b).
    #[tokio::test]
    async fn pooled_connections_see_columns_added_by_a_migration() {
        let dir = tempfile::tempdir().unwrap();
        let config = crate::Config {
            database_url: format!("sqlite://{}/app.db", dir.path().display()),
            database_pool_size: 4,
            ..crate::Config::default()
        };
        let db = super::super::connect(&config).await.unwrap();
        let create = Migration::new(
            "1_notes",
            "CREATE TABLE notes (id INTEGER PRIMARY KEY, a TEXT);",
            None,
        );
        Migrator::new(vec![create]).unwrap().run(&db).await.unwrap();
        sql("INSERT INTO notes (a) VALUES ('x')")
            .execute(&db)
            .await
            .unwrap();

        // Every connection in the pool reads the table (and caches the query).
        let mut open = Vec::new();
        for _ in 0..4 {
            let mut tx = db.begin().await.unwrap();
            sql("SELECT * FROM notes").fetch_all(&mut tx).await.unwrap();
            open.push(tx);
        }
        drop(open);

        let alter = Migration::new("2_notes_b", "ALTER TABLE notes ADD COLUMN b TEXT;", None);
        Migrator::new(vec![create, alter])
            .unwrap()
            .run(&db)
            .await
            .unwrap();
        for _ in 0..8 {
            let rows = sql("SELECT * FROM notes").fetch_all(&db).await.unwrap();
            assert_eq!(rows[0].try_get::<Option<String>>("b").unwrap(), None);
        }
    }
}
