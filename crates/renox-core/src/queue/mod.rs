//! Background jobs stored in SQLite, with retries and a failed-jobs table.
//!
//! ```
//! # use renox::prelude::*;
//! # #[derive(Model, serde::Serialize, Default)] struct Order { id: i64 }
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Serialize, Deserialize)]
//! struct SendReceipt { order_id: i64 }
//!
//! impl Job for SendReceipt {
//!     const NAME: &'static str = "send-receipt";
//!     const MAX_ATTEMPTS: u32 = 5;
//!
//!     async fn handle(self, ctx: JobContext) -> Result {
//!         let order = Order::find_or_404(&ctx.state.db, self.order_id).await?;
//!         // … send it
//! #       let _ = order;
//!         Ok(())
//!     }
//! }
//!
//! # let _ =
//! App::new().job::<SendReceipt>();            // register the handler
//! # async fn demo(state: AppState, order_id: i64) -> Result {
//! state.dispatch(SendReceipt { order_id }).await?;   // in a handler
//! # Ok(()) }
//! ```
//!
//! `serve` runs workers in the same process (`QUEUE_WORKERS`, default 2; 0 to
//! turn off); `my-app queue:work` runs them on their own.

mod worker;

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde::de::DeserializeOwned;
use tokio::sync::Notify;

pub use worker::Worker;

use crate::db::{Db, Migration};
use crate::{AppState, Result};

pub(crate) const MIGRATION: Migration =
    crate::db::framework_migration!("queue", "00010101000100_create_jobs_table");

/// A unit of background work. It is stored as JSON, so keep it to ids and
/// small values rather than whole models.
pub trait Job: Serialize + DeserializeOwned + Send + Sync + 'static {
    /// A stable name; stored with each job, so don't rename it while jobs are queued.
    const NAME: &'static str;
    const QUEUE: &'static str = "default";
    /// Runs before the job is moved to `failed_jobs`.
    const MAX_ATTEMPTS: u32 = 3;
    /// How long one attempt may run.
    const TIMEOUT: Duration = Duration::from_secs(60);

    /// Wait before retrying after the given failed attempt (1-based).
    fn backoff(attempt: u32) -> Duration {
        Duration::from_secs(10 * u64::from(attempt))
    }

    fn handle(self, ctx: JobContext) -> impl Future<Output = Result> + Send;
}

/// What a job gets when it runs.
pub struct JobContext {
    pub state: AppState,
    /// 1 on the first try.
    pub attempt: u32,
}

type RunFn =
    Arc<dyn Fn(String, JobContext) -> Pin<Box<dyn Future<Output = Result> + Send>> + Send + Sync>;

/// A registered job type: how to run it and when to retry it.
#[derive(Clone)]
pub struct JobHandler {
    pub(crate) run: RunFn,
    pub(crate) backoff: fn(u32) -> Duration,
    pub(crate) timeout: Duration,
}

/// The handler for `J`; `Registry::job` registers it.
pub fn handler<J: Job>() -> JobHandler {
    JobHandler {
        run: Arc::new(|payload, ctx| {
            Box::pin(async move {
                let job: J = serde_json::from_str(&payload)?;
                job.handle(ctx).await
            })
        }),
        backoff: J::backoff,
        timeout: J::TIMEOUT,
    }
}

pub(crate) type Handlers = Arc<HashMap<&'static str, JobHandler>>;

/// Dispatches jobs and wakes the workers of this process.
#[derive(Clone)]
pub struct Queue {
    db: Db,
    wake: Arc<Notify>,
}

pub(crate) fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or_default()
}

impl Queue {
    pub(crate) fn new(db: Db) -> Self {
        Self {
            db,
            wake: Arc::new(Notify::new()),
        }
    }

    pub(crate) fn wake(&self) -> &Notify {
        &self.wake
    }

    /// Queues a job to run as soon as a worker is free; returns its id.
    pub async fn dispatch<J: Job>(&self, job: J) -> Result<i64> {
        self.dispatch_after(job, Duration::ZERO).await
    }

    /// Queues a job to run after `delay`; returns its id.
    pub async fn dispatch_after<J: Job>(&self, job: J, delay: Duration) -> Result<i64> {
        let id = Self::insert(&self.db, &job, delay).await?;
        self.wake.notify_waiters();
        Ok(id)
    }

    /// Queues a job inside `tx`, so it only exists if the transaction
    /// commits. Call `wake()` after committing.
    pub(crate) async fn dispatch_in<J: Job>(
        tx: &mut crate::db::Transaction,
        job: &J,
    ) -> Result<i64> {
        Self::insert(tx, job, Duration::ZERO).await
    }

    pub(crate) fn wake_workers(&self) {
        self.wake.notify_waiters();
    }

    async fn insert<'c, J: Job>(
        db: impl crate::db::Executor<'c>,
        job: &J,
        delay: Duration,
    ) -> Result<i64> {
        let now = unix_now();
        let id: i64 = crate::db::sql(
            "INSERT INTO jobs (queue, job, payload, max_attempts, available_at, created_at) \
             VALUES (?, ?, ?, ?, ?, ?) RETURNING id",
        )
        .bind(J::QUEUE)
        .bind(J::NAME)
        .bind(serde_json::to_string(job)?)
        .bind(i64::from(J::MAX_ATTEMPTS.max(1)))
        .bind(now + delay.as_secs() as i64)
        .bind(now)
        .scalar(db)
        .await?;
        Ok(id)
    }

    /// Jobs waiting or running.
    pub async fn pending(&self) -> Result<i64> {
        Ok(crate::db::sql("SELECT COUNT(*) FROM jobs")
            .scalar(&self.db)
            .await?)
    }

    pub async fn failed(&self) -> Result<Vec<FailedJob>> {
        let rows = crate::db::sql(
            "SELECT id, queue, job, payload, error, failed_at FROM failed_jobs ORDER BY id",
        )
        .fetch_all(&self.db)
        .await?;
        rows.iter()
            .map(|r| {
                Ok(FailedJob {
                    id: r.try_get("id")?,
                    queue: r.try_get("queue")?,
                    job: r.try_get("job")?,
                    payload: r.try_get("payload")?,
                    error: r.try_get("error")?,
                    failed_at: r.try_get("failed_at")?,
                })
            })
            .collect()
    }

    /// Puts failed jobs back on their queue with fresh attempts; `None` retries all.
    pub async fn retry(&self, id: Option<i64>) -> Result<u64> {
        let mut tx = self.db.begin().await?;
        let filter = if id.is_some() { " WHERE id = ?" } else { "" };
        let insert = format!(
            "INSERT INTO jobs (queue, job, payload, max_attempts, available_at, created_at) \
             SELECT queue, job, payload, max_attempts, ?, ? FROM failed_jobs{filter}"
        );
        let mut query = crate::db::sql(insert).bind(unix_now()).bind(unix_now());
        if let Some(id) = id {
            query = query.bind(id);
        }
        let moved = query.execute(&mut tx).await?;
        let mut delete = crate::db::sql(format!("DELETE FROM failed_jobs{filter}"));
        if let Some(id) = id {
            delete = delete.bind(id);
        }
        delete.execute(&mut tx).await?;
        tx.commit().await?;
        self.wake.notify_waiters();
        Ok(moved)
    }

    /// Deletes failed jobs.
    pub async fn flush_failed(&self) -> Result<u64> {
        Ok(crate::db::sql("DELETE FROM failed_jobs")
            .execute(&self.db)
            .await?)
    }
}

/// A job that used up its attempts.
#[derive(Debug, Clone, Serialize)]
pub struct FailedJob {
    pub id: i64,
    pub queue: String,
    pub job: String,
    pub payload: String,
    pub error: String,
    /// Unix seconds.
    pub failed_at: i64,
}

impl AppState {
    /// Shorthand for `state.queue.dispatch(job)`.
    pub async fn dispatch<J: Job>(&self, job: J) -> Result<i64> {
        self.queue.dispatch(job).await
    }
}
