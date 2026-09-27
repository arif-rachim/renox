use std::collections::{HashMap, HashSet};

use super::{Db, now, quote, script, sql};
use anyhow::{Context, anyhow, bail};

const TABLE: &str = "renox_migrations";

/// One migration: SQL to apply it and, optionally, SQL to undo it.
///
/// Usually generated from `migrations/*.up.sql` and `*.down.sql` by
/// `renox::migrations!()`. Names start with a timestamp and run in name order.
#[derive(Debug, Clone, Copy)]
pub struct Migration {
    pub name: &'static str,
    pub up: &'static str,
    pub down: Option<&'static str>,
}

/// Whether a migration has run, and in which batch.
#[derive(Debug, Clone)]
pub struct MigrationStatus {
    pub name: String,
    pub batch: Option<i64>,
}

/// Runs migrations in batches, like Laravel: `run` applies everything pending
/// as one batch, `rollback` undoes whole batches, newest first.
pub(crate) struct Migrator {
    migrations: Vec<Migration>,
}

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

    async fn ensure_table(db: &Db) -> anyhow::Result<()> {
        sql(format!(
            "CREATE TABLE IF NOT EXISTS {TABLE} (
                name TEXT PRIMARY KEY NOT NULL,
                batch INTEGER NOT NULL,
                applied_at TEXT NOT NULL
            )"
        ))
        .execute(db)
        .await?;
        Ok(())
    }

    async fn applied(db: &Db) -> anyhow::Result<HashMap<String, i64>> {
        Self::ensure_table(db).await?;
        let rows = sql(format!("SELECT name, batch FROM {TABLE}"))
            .fetch_all(db)
            .await?;
        rows.iter()
            .map(|row| Ok((row.try_get("name")?, row.try_get("batch")?)))
            .collect()
    }

    /// Applies pending migrations as a new batch; returns their names.
    pub async fn run(&self, db: &Db) -> anyhow::Result<Vec<&'static str>> {
        let applied = Self::applied(db).await?;
        let batch = applied.values().max().copied().unwrap_or(0) + 1;
        let mut done = Vec::new();

        for migration in self
            .migrations
            .iter()
            .filter(|m| !applied.contains_key(m.name))
        {
            let mut tx = db.begin().await?;
            script(&mut tx, migration.up)
                .await
                .with_context(|| format!("migration `{}` failed", migration.name))?;
            sql(format!(
                "INSERT INTO {TABLE} (name, batch, applied_at) VALUES (?, ?, ?)"
            ))
            .bind(migration.name)
            .bind(batch)
            .bind(now().to_rfc3339())
            .execute(&mut tx)
            .await?;
            tx.commit().await?;
            done.push(migration.name);
        }
        Ok(done)
    }

    /// Undoes the last `batches` batches, newest migration first.
    pub async fn rollback(&self, db: &Db, batches: u32) -> anyhow::Result<Vec<String>> {
        let applied = Self::applied(db).await?;
        let mut numbers: Vec<i64> = applied
            .values()
            .copied()
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        numbers.sort_unstable_by(|a, b| b.cmp(a));
        let targets: HashSet<i64> = numbers.into_iter().take(batches as usize).collect();

        let mut names: Vec<&String> = applied
            .iter()
            .filter(|(_, batch)| targets.contains(batch))
            .map(|(name, _)| name)
            .collect();
        names.sort_unstable_by(|a, b| b.cmp(a));

        let mut done = Vec::new();
        for name in names {
            let migration = self
                .migrations
                .iter()
                .find(|m| m.name == name)
                .ok_or_else(|| {
                    anyhow!("migration `{name}` was applied but is no longer registered")
                })?;
            let down = migration.down.ok_or_else(|| {
                anyhow!("migration `{name}` has no .down.sql, so it can't be rolled back")
            })?;
            let mut tx = db.begin().await?;
            script(&mut tx, down)
                .await
                .with_context(|| format!("rolling back `{name}` failed"))?;
            sql(format!("DELETE FROM {TABLE} WHERE name = ?"))
                .bind(name)
                .execute(&mut tx)
                .await?;
            tx.commit().await?;
            done.push(name.clone());
        }
        Ok(done)
    }

    /// Drops every table and view, then runs all migrations.
    pub async fn fresh(&self, db: &Db) -> anyhow::Result<Vec<&'static str>> {
        if let Some(pool) = db.sqlite() {
            drop_all_sqlite(pool).await?;
        }
        #[cfg(feature = "postgres")]
        if let Some(pool) = db.postgres() {
            drop_all_postgres(pool).await?;
        }
        self.run(db).await
    }

    /// Every registered migration and whether it has run.
    pub async fn status(&self, db: &Db) -> anyhow::Result<Vec<MigrationStatus>> {
        let applied = Self::applied(db).await?;
        Ok(self
            .migrations
            .iter()
            .map(|m| MigrationStatus {
                name: m.name.to_owned(),
                batch: applied.get(m.name).copied(),
            })
            .collect())
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

/// Drops every table and view in the current schema; `CASCADE` takes care
/// of foreign keys between them.
#[cfg(feature = "postgres")]
async fn drop_all_postgres(pool: &sqlx::PgPool) -> anyhow::Result<()> {
    use sqlx::{AssertSqlSafe, Row};

    let objects = sqlx::query(
        "SELECT 'VIEW' AS kind, table_name::text AS name FROM information_schema.views \
         WHERE table_schema = current_schema() \
         UNION ALL \
         SELECT 'TABLE', tablename::text FROM pg_tables WHERE schemaname = current_schema()",
    )
    .fetch_all(pool)
    .await?;
    for object in &objects {
        let kind: String = object.try_get("kind")?;
        let name: String = object.try_get("name")?;
        let sql = format!("DROP {kind} IF EXISTS {} CASCADE", quote(&name));
        sqlx::query(AssertSqlSafe(sql)).execute(pool).await?;
    }
    Ok(())
}
