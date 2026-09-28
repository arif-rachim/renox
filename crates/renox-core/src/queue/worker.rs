use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Duration;

use tokio::sync::watch;
use tokio::task::JoinSet;

use super::{Handlers, JobContext, unix_now};
use crate::AppState;
use crate::db::Dialect;

/// A reserved job older than this is assumed abandoned (e.g. the process
/// crashed) and becomes available again. Jobs with a longer `TIMEOUT` get
/// their reservation extended (`extend_reservation`).
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
    /// Unix seconds of the last `sweep_exhausted`.
    last_sweep: Arc<AtomicI64>,
}

/// Why an attempt failed, and whether another attempt could succeed.
struct Failure {
    error: String,
    permanent: bool,
}

impl Failure {
    fn retry(error: String) -> Self {
        Self {
            error,
            permanent: false,
        }
    }

    fn permanent(error: String) -> Self {
        Self {
            error,
            permanent: true,
        }
    }
}

fn panic_message(err: tokio::task::JoinError) -> String {
    match err.try_into_panic() {
        Ok(panic) => format!("the job panicked: {}", crate::error::panic_message(&*panic)),
        Err(err) => format!("the job was cancelled: {err}"),
    }
}

async fn fail(
    tx: &mut crate::db::Transaction,
    queue: &str,
    job: &str,
    payload: &str,
    max_attempts: u32,
    error: &str,
) -> crate::Result {
    crate::db::sql(
        "INSERT INTO failed_jobs (queue, job, payload, max_attempts, error, failed_at) \
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(queue)
    .bind(job)
    .bind(payload)
    .bind(i64::from(max_attempts))
    .bind(error)
    .bind(unix_now())
    .execute(tx)
    .await?;
    Ok(())
}

/// Runs a bookkeeping write, retrying for about 10 seconds while the
/// database is briefly unavailable (restarting, or SQLite busy).
async fn retry_write<F, Fut>(mut write: F) -> crate::Result
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = crate::Result>,
{
    let mut delays = [100u64, 400, 1000, 2500, 6000].into_iter();
    loop {
        match write().await {
            Ok(()) => return Ok(()),
            Err(err) => match delays.next() {
                Some(ms) => {
                    tracing::warn!(error = ?err, "queue bookkeeping failed, retrying");
                    tokio::time::sleep(Duration::from_millis(ms)).await;
                }
                None => return Err(err),
            },
        }
    }
}

impl Worker {
    pub(crate) fn new(state: AppState, handlers: Handlers, queues: Vec<String>) -> Self {
        Self {
            state,
            handlers,
            queues,
            last_sweep: Arc::default(),
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
        // SQLite runs one write at a time, so the UPDATE is enough. On
        // PostgreSQL, workers on several servers must not pick the same row.
        let lock = match self.state.db.dialect() {
            Dialect::Sqlite => "",
            Dialect::Postgres => " FOR UPDATE SKIP LOCKED",
        };
        let sql = format!(
            "UPDATE jobs SET reserved_at = ?, attempts = attempts + 1 WHERE id = (\
                SELECT id FROM jobs WHERE available_at <= ? \
                AND (reserved_at IS NULL OR reserved_at <= ?) \
                AND attempts < max_attempts{queues} \
                ORDER BY available_at, id LIMIT 1{lock}) \
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
        self.sweep_exhausted().await?;
        let Some(job) = self.reserve().await? else {
            return Ok(false);
        };
        let handler = self.handlers.get(job.job.as_str()).cloned();
        if let Some(handler) = &handler {
            self.extend_reservation(&job, handler.timeout).await?;
        }
        let outcome = match &handler {
            None => Err(Failure::permanent(format!(
                "no handler registered for job `{}`",
                job.job
            ))),
            Some(handler) => {
                let ctx = JobContext {
                    state: self.state.clone(),
                    attempt: job.attempts,
                };
                // Its own task, so a panic is a failed attempt, not a dead worker.
                let run = (handler.run)(job.payload.clone(), ctx);
                let run = crate::context::scope_app(self.state.clone(), run);
                let mut task = tokio::spawn(run);
                match tokio::time::timeout(handler.timeout, &mut task).await {
                    Ok(Ok(Ok(()))) => Ok(()),
                    Ok(Ok(Err(err))) => Err(Failure {
                        permanent: err.is_permanent(),
                        error: format!("{err:?}"),
                    }),
                    Ok(Err(join)) => Err(Failure::retry(panic_message(join))),
                    Err(_) => {
                        task.abort();
                        Err(Failure::retry(format!(
                            "timed out after {:?}",
                            handler.timeout
                        )))
                    }
                }
            }
        };

        // The job ran; don't let a brief database hiccup strand it until its
        // reservation expires.
        let backoff = handler
            .as_ref()
            .map(|h| (h.backoff)(job.attempts))
            .unwrap_or_default();
        retry_write(|| self.record(&job, &outcome, backoff)).await?;
        match outcome {
            Ok(()) => tracing::info!(job = %job.job, id = job.id, "job done"),
            Err(failure) if !failure.permanent && job.attempts < job.max_attempts => {
                tracing::warn!(job = %job.job, id = job.id, attempt = job.attempts, error = %failure.error, "job failed, will retry");
            }
            Err(failure) => {
                tracing::error!(job = %job.job, id = job.id, error = %failure.error, "job failed for good");
            }
        }
        Ok(true)
    }

    /// Writes a job's outcome: deleted, back on the queue, or failed.
    async fn record(
        &self,
        job: &Reserved,
        outcome: &Result<(), Failure>,
        backoff: Duration,
    ) -> crate::Result {
        let db = &self.state.db;
        match outcome {
            Ok(()) => {
                crate::db::sql("DELETE FROM jobs WHERE id = ?")
                    .bind(job.id)
                    .execute(db)
                    .await?;
            }
            Err(failure) if !failure.permanent && job.attempts < job.max_attempts => {
                crate::db::sql("UPDATE jobs SET reserved_at = NULL, available_at = ? WHERE id = ?")
                    .bind(unix_now() + backoff.as_secs() as i64)
                    .bind(job.id)
                    .execute(db)
                    .await?;
            }
            Err(failure) => {
                let mut tx = db.begin().await?;
                let deleted = crate::db::sql("DELETE FROM jobs WHERE id = ?")
                    .bind(job.id)
                    .execute(&mut tx)
                    .await?;
                if deleted > 0 {
                    fail(
                        &mut tx,
                        &job.queue,
                        &job.job,
                        &job.payload,
                        job.max_attempts,
                        &failure.error,
                    )
                    .await?;
                }
                tx.commit().await?;
            }
        }
        Ok(())
    }

    /// A job whose attempts may outlast the reservation keeps its row for
    /// the attempt's timeout plus a minute, so no other worker starts it again.
    async fn extend_reservation(&self, job: &Reserved, timeout: Duration) -> crate::Result {
        let needed = timeout.as_secs() as i64 + 60;
        if needed > RESERVATION {
            crate::db::sql("UPDATE jobs SET reserved_at = ? WHERE id = ?")
                .bind(unix_now() + (needed - RESERVATION))
                .bind(job.id)
                .execute(&self.state.db)
                .await?;
        }
        Ok(())
    }

    /// Moves jobs whose last attempt never finished (the process crashed,
    /// was killed or ran out of memory) to `failed_jobs`, at most once a minute.
    async fn sweep_exhausted(&self) -> crate::Result {
        let now = unix_now();
        let last = self.last_sweep.load(Ordering::Relaxed);
        if now - last < 60
            || self
                .last_sweep
                .compare_exchange(last, now, Ordering::Relaxed, Ordering::Relaxed)
                .is_err()
        {
            return Ok(());
        }
        let mut tx = self.state.db.begin().await?;
        let rows = crate::db::sql(
            "DELETE FROM jobs WHERE attempts >= max_attempts AND reserved_at IS NOT NULL \
             AND reserved_at <= ? RETURNING queue, job, payload, max_attempts",
        )
        .bind(now - RESERVATION)
        .fetch_all(&mut tx)
        .await?;
        for row in &rows {
            let job: String = row.try_get("job")?;
            fail(
                &mut tx,
                &row.try_get::<String>("queue")?,
                &job,
                &row.try_get::<String>("payload")?,
                row.try_get::<i64>("max_attempts")? as u32,
                "the worker stopped during the last attempt (crash, kill or out of memory)",
            )
            .await?;
            tracing::error!(job = %job, "job's last attempt never finished; moved to failed_jobs");
        }
        tx.commit().await?;
        Ok(())
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
                let wake = worker.state.queue.wake();
                loop {
                    if *stop.borrow() {
                        break;
                    }
                    // Listen before looking for work: a job dispatched while
                    // the query runs still wakes this loop instead of waiting
                    // for the next poll.
                    let notified = wake.notified();
                    tokio::pin!(notified);
                    notified.as_mut().enable();
                    match worker.run_next().await {
                        Ok(true) => continue,
                        Ok(false) => {}
                        Err(err) => tracing::error!(error = ?err, "queue worker error"),
                    }
                    tokio::select! {
                        _ = notified => {}
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
