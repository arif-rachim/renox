use std::time::Duration;

use tokio::sync::watch;
use tokio::task::JoinSet;

use super::{Handlers, JobContext, unix_now};
use crate::AppState;

/// A reserved job older than this is assumed abandoned (e.g. the process
/// crashed) and becomes available again.
const RESERVATION: i64 = 15 * 60;
const POLL: Duration = Duration::from_secs(1);

struct Reserved {
    id: i64,
    queue: String,
    job: String,
    payload: String,
    attempts: u32,
    max_attempts: u32,
}

/// Runs queued jobs.
#[derive(Clone)]
pub struct Worker {
    state: AppState,
    handlers: Handlers,
    queues: Vec<String>,
}

impl Worker {
    pub(crate) fn new(state: AppState, handlers: Handlers, queues: Vec<String>) -> Self {
        Self {
            state,
            handlers,
            queues,
        }
    }

    async fn reserve(&self) -> crate::Result<Option<Reserved>> {
        let now = unix_now();
        let queues = if self.queues.is_empty() {
            String::new()
        } else {
            let marks = vec!["?"; self.queues.len()].join(", ");
            format!(" AND queue IN ({marks})")
        };
        let sql = format!(
            "UPDATE jobs SET reserved_at = ?, attempts = attempts + 1 WHERE id = (\
                SELECT id FROM jobs WHERE available_at <= ? \
                AND (reserved_at IS NULL OR reserved_at <= ?){queues} \
                ORDER BY available_at, id LIMIT 1) \
             RETURNING id, queue, job, payload, attempts, max_attempts"
        );
        let mut query = crate::db::sql(sql)
            .bind(now)
            .bind(now)
            .bind(now - RESERVATION);
        for queue in &self.queues {
            query = query.bind(queue);
        }
        let Some(row) = query.fetch_optional(&self.state.db).await? else {
            return Ok(None);
        };
        Ok(Some(Reserved {
            id: row.try_get("id")?,
            queue: row.try_get("queue")?,
            job: row.try_get("job")?,
            payload: row.try_get("payload")?,
            attempts: row.try_get::<i64>("attempts")? as u32,
            max_attempts: row.try_get::<i64>("max_attempts")? as u32,
        }))
    }

    /// Runs one available job; returns whether there was one.
    pub async fn run_next(&self) -> crate::Result<bool> {
        let Some(job) = self.reserve().await? else {
            return Ok(false);
        };
        let handler = self.handlers.get(job.job.as_str()).cloned();
        let outcome = match &handler {
            None => Err(format!("no handler registered for job `{}`", job.job)),
            Some(handler) => {
                let ctx = JobContext {
                    state: self.state.clone(),
                    attempt: job.attempts,
                };
                let run = (handler.run)(job.payload.clone(), ctx);
                match tokio::time::timeout(handler.timeout, run).await {
                    Ok(Ok(())) => Ok(()),
                    Ok(Err(err)) => Err(format!("{err:?}")),
                    Err(_) => Err(format!("timed out after {:?}", handler.timeout)),
                }
            }
        };

        let db = &self.state.db;
        match outcome {
            Ok(()) => {
                crate::db::sql("DELETE FROM jobs WHERE id = ?")
                    .bind(job.id)
                    .execute(db)
                    .await?;
                tracing::info!(job = %job.job, id = job.id, "job done");
            }
            Err(error) if handler.is_some() && job.attempts < job.max_attempts => {
                let backoff = handler
                    .map(|h| (h.backoff)(job.attempts))
                    .unwrap_or_default();
                crate::db::sql("UPDATE jobs SET reserved_at = NULL, available_at = ? WHERE id = ?")
                    .bind(unix_now() + backoff.as_secs() as i64)
                    .bind(job.id)
                    .execute(db)
                    .await?;
                tracing::warn!(job = %job.job, id = job.id, attempt = job.attempts, %error, "job failed, will retry");
            }
            Err(error) => {
                let mut tx = db.begin().await?;
                crate::db::sql(
                    "INSERT INTO failed_jobs (queue, job, payload, max_attempts, error, failed_at) \
                     VALUES (?, ?, ?, ?, ?, ?)",
                )
                .bind(&job.queue)
                .bind(&job.job)
                .bind(&job.payload)
                .bind(i64::from(job.max_attempts))
                .bind(&error)
                .bind(unix_now())
                .execute(&mut tx)
                .await?;
                crate::db::sql("DELETE FROM jobs WHERE id = ?")
                    .bind(job.id)
                    .execute(&mut tx)
                    .await?;
                tx.commit().await?;
                tracing::error!(job = %job.job, id = job.id, %error, "job failed for good");
            }
        }
        Ok(true)
    }

    /// Runs every job that is available now; returns how many ran.
    pub async fn drain(&self) -> crate::Result<usize> {
        let mut ran = 0;
        while self.run_next().await? {
            ran += 1;
        }
        Ok(ran)
    }

    /// Runs `concurrency` loops until `shutdown` flips to true, then lets
    /// running jobs finish.
    pub async fn run(self, concurrency: usize, mut shutdown: watch::Receiver<bool>) {
        let mut loops = JoinSet::new();
        for _ in 0..concurrency.max(1) {
            let worker = self.clone();
            let mut stop = shutdown.clone();
            loops.spawn(async move {
                loop {
                    if *stop.borrow() {
                        break;
                    }
                    match worker.run_next().await {
                        Ok(true) => continue,
                        Ok(false) => {}
                        Err(err) => tracing::error!(error = ?err, "queue worker error"),
                    }
                    tokio::select! {
                        _ = worker.state.queue.wake().notified() => {}
                        _ = tokio::time::sleep(POLL) => {}
                        _ = stop.changed() => {}
                    }
                }
            });
        }
        let _ = shutdown.changed().await;
        while loops.join_next().await.is_some() {}
    }
}
