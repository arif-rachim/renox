use std::collections::{HashMap, HashSet};

use anyhow::{Context, anyhow, bail};
use sqlx::{AssertSqlSafe, Row};

use super::{Db, now, quote};

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
        sqlx::query(AssertSqlSafe(format!(
            "CREATE TABLE IF NOT EXISTS {TABLE} (
                name TEXT PRIMARY KEY NOT NULL,
                batch INTEGER NOT NULL,
                applied_at TEXT NOT NULL
            )"
        )))
        .execute(db)
        .await?;
        Ok(())
    }

    async fn applied(db: &Db) -> anyhow::Result<HashMap<String, i64>> {
        Self::ensure_table(db).await?;
        let rows = sqlx::query(AssertSqlSafe(format!("SELECT name, batch FROM {TABLE}")))
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
            sqlx::raw_sql(AssertSqlSafe(migration.up))
                .execute(&mut *tx)
                .await
                .with_context(|| format!("migration `{}` failed", migration.name))?;
            sqlx::query(AssertSqlSafe(format!(
                "INSERT INTO {TABLE} (name, batch, applied_at) VALUES (?, ?, ?)"
            )))
            .bind(migration.name)
            .bind(batch)
            .bind(now().to_rfc3339())
            .execute(&mut *tx)
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
            sqlx::raw_sql(AssertSqlSafe(down))
                .execute(&mut *tx)
                .await
                .with_context(|| format!("rolling back `{name}` failed"))?;
            sqlx::query(AssertSqlSafe(format!("DELETE FROM {TABLE} WHERE name = ?")))
                .bind(name)
                .execute(&mut *tx)
                .await?;
            tx.commit().await?;
            done.push(name.clone());
        }
        Ok(done)
    }

    /// Drops every table and view, then runs all migrations.
    pub async fn fresh(&self, db: &Db) -> anyhow::Result<Vec<&'static str>> {
        let mut conn = db.acquire().await?;
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
        drop(conn);
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
