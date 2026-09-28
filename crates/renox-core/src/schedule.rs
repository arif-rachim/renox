//! Tasks that run on a schedule, defined in code and run by `serve` (or
//! `my-app schedule:work`).
//!
//! ```
//! # use renox::prelude::*;
//! # async fn sync(_: &AppState) -> Result { Ok(()) }
//! # let _ =
//! App::new().schedule(|s| {
//!     s.every_minutes(5, "sync-stock", |state| async move { sync(&state).await });
//!     s.daily_at("02:00", "cleanup", |state| async move {
//!         renox::db::sql("DELETE FROM carts WHERE updated_at < ?")
//!             .bind(renox::db::now() - renox::chrono::TimeDelta::days(30))
//!             .execute(&state.db)
//!             .await?;
//!         Ok(())
//!     });
//! })
//! # ;
//! ```
//!
//! Times are in `APP_TIMEZONE`, a UTC offset such as `+07:00` (WIB). A task
//! whose previous run hasn't finished is skipped rather than run twice.
//!
//! Several processes may run the same schedule (e.g. `serve` on two servers
//! sharing a PostgreSQL database): each run is claimed in the `cache` table
//! first, so only one of them runs it.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::time::Duration;

use anyhow::{Context, bail};
use tokio::sync::watch;

use crate::queue::unix_now;
use crate::{AppState, Result};

type TaskFn = Arc<dyn Fn(AppState) -> Pin<Box<dyn Future<Output = Result> + Send>> + Send + Sync>;

#[derive(Debug, Clone, Copy, PartialEq)]
enum When {
    /// Every `n` seconds, on multiples of `n` since midnight in the timezone.
    Every(i64),
    /// Once a day at this many seconds after midnight in the timezone.
    Daily(i64),
}

impl When {
    /// How long a run's claim is kept: past the slot, so a process whose
    /// clock is a little behind doesn't run it again, and no longer.
    fn claim_for(self) -> i64 {
        match self {
            When::Every(seconds) => seconds.max(60) + 60,
            When::Daily(_) => 24 * 60 * 60,
        }
    }
}

/// Clears a task's `running` flag when dropped.
struct Running(Arc<AtomicBool>);

impl Drop for Running {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

#[derive(Clone)]
struct Task {
    name: String,
    when: When,
    run: TaskFn,
    running: Arc<AtomicBool>,
}

/// The app's scheduled tasks.
#[derive(Clone, Default)]
pub struct Schedule {
    tasks: Vec<Task>,
    error: Option<String>,
}

impl Schedule {
    fn add<F, Fut>(&mut self, name: &str, when: When, task: F) -> &mut Self
    where
        F: Fn(AppState) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result> + Send + 'static,
    {
        self.tasks.push(Task {
            name: name.to_owned(),
            when,
            run: Arc::new(move |state| Box::pin(task(state))),
            running: Arc::new(AtomicBool::new(false)),
        });
        self
    }

    /// Every `interval`, aligned to the clock (every 5 minutes runs at :00, :05, ...).
    pub fn every<F, Fut>(&mut self, interval: Duration, name: &str, task: F) -> &mut Self
    where
        F: Fn(AppState) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result> + Send + 'static,
    {
        let seconds = interval.as_secs().max(1) as i64;
        self.add(name, When::Every(seconds), task)
    }

    pub fn every_minute<F, Fut>(&mut self, name: &str, task: F) -> &mut Self
    where
        F: Fn(AppState) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result> + Send + 'static,
    {
        self.every(Duration::from_secs(60), name, task)
    }

    pub fn every_minutes<F, Fut>(&mut self, minutes: u64, name: &str, task: F) -> &mut Self
    where
        F: Fn(AppState) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result> + Send + 'static,
    {
        self.every(Duration::from_secs(minutes * 60), name, task)
    }

    pub fn hourly<F, Fut>(&mut self, name: &str, task: F) -> &mut Self
    where
        F: Fn(AppState) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result> + Send + 'static,
    {
        self.every(Duration::from_secs(3600), name, task)
    }

    /// Every day at `HH:MM` in `APP_TIMEZONE`.
    pub fn daily_at<F, Fut>(&mut self, time: &str, name: &str, task: F) -> &mut Self
    where
        F: Fn(AppState) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result> + Send + 'static,
    {
        match parse_time(time) {
            Ok(seconds) => self.add(name, When::Daily(seconds), task),
            Err(err) => {
                self.error.get_or_insert(format!("task `{name}`: {err}"));
                self
            }
        }
    }

    pub(crate) fn check(&self) -> anyhow::Result<()> {
        match &self.error {
            Some(error) => bail!("invalid schedule: {error}"),
            None => Ok(()),
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.tasks.is_empty()
    }

    /// Each task with its next run time (unix seconds).
    pub fn upcoming(&self, offset: i64) -> Vec<(String, i64)> {
        let now = unix_now();
        self.tasks
            .iter()
            .map(|t| (t.name.clone(), next_run(t.when, now, offset)))
            .collect()
    }

    /// Runs tasks as they fall due until `shutdown` flips to true.
    pub(crate) async fn run(
        self,
        state: AppState,
        offset: i64,
        mut shutdown: watch::Receiver<bool>,
    ) {
        let mut next: Vec<i64> = self
            .tasks
            .iter()
            .map(|t| next_run(t.when, unix_now(), offset))
            .collect();
        loop {
            let Some(soonest) = next.iter().min().copied() else {
                return;
            };
            let wait = Duration::from_secs((soonest - unix_now()).max(0) as u64);
            tokio::select! {
                _ = tokio::time::sleep(wait) => {}
                _ = shutdown.changed() => return,
            }
            let now = unix_now();
            for (task, at) in self.tasks.iter().zip(next.iter_mut()) {
                if *at > now {
                    continue;
                }
                let slot = *at;
                *at = next_run(task.when, now, offset);
                if task.running.swap(true, Ordering::SeqCst) {
                    tracing::warn!(task = %task.name, "skipped: the previous run is still going");
                    continue;
                }
                let (task, state) = (task.clone(), state.clone());
                tokio::spawn(async move {
                    // Clears `running` however the run ends, panics included.
                    let _running = Running(task.running.clone());
                    if !claim(&state, &task.name, slot, task.when.claim_for()).await {
                        tracing::debug!(task = %task.name, "skipped: another process runs it");
                        return;
                    }
                    tracing::info!(task = %task.name, "scheduled task started");
                    let run =
                        std::panic::AssertUnwindSafe(crate::context::scope((task.run)(state)));
                    match futures_util::FutureExt::catch_unwind(run).await {
                        Ok(Ok(())) => {}
                        Ok(Err(err)) => {
                            tracing::error!(task = %task.name, error = ?err, "scheduled task failed");
                        }
                        Err(_) => tracing::error!(task = %task.name, "scheduled task panicked"),
                    }
                });
            }
        }
    }
}

/// Claims the run of `task` due at `slot`, so that processes sharing the
/// database run it once: the first to insert the claim wins. The claim is
/// kept `keep` seconds. If the database can't be reached the task runs
/// anyway, as a single process would.
#[doc(hidden)]
pub async fn claim(state: &AppState, task: &str, slot: i64, keep: i64) -> bool {
    let now = unix_now();
    prune_claims(state, now).await;
    let claimed = crate::db::sql(
        "INSERT INTO cache (key, value, expires_at) VALUES (?, 'null', ?) \
         ON CONFLICT (key) DO NOTHING",
    )
    .bind(format!("{CLAIM_PREFIX}{task}:{slot}"))
    .bind(now + keep)
    .execute(&state.db)
    .await;
    match claimed {
        Ok(rows) => rows == 1,
        Err(err) => {
            tracing::warn!(task, error = %err, "could not claim the run; running it anyway");
            true
        }
    }
}

/// Framework rows in the `cache` table start with `renox:`; `Cache::flush` keeps them.
const CLAIM_PREFIX: &str = "renox:schedule:";

/// Deletes expired claims, at most once a minute per process.
async fn prune_claims(state: &AppState, now: i64) {
    static LAST: AtomicI64 = AtomicI64::new(0);
    let last = LAST.load(Ordering::Relaxed);
    if now - last < 60
        || LAST
            .compare_exchange(last, now, Ordering::Relaxed, Ordering::Relaxed)
            .is_err()
    {
        return;
    }
    let pruned = crate::db::sql("DELETE FROM cache WHERE key LIKE ? AND expires_at < ?")
        .bind(format!("{CLAIM_PREFIX}%"))
        .bind(now)
        .execute(&state.db)
        .await;
    if let Err(err) = pruned {
        tracing::warn!(error = %err, "could not clear old schedule claims");
    }
}

fn parse_time(time: &str) -> anyhow::Result<i64> {
    let (hour, minute) = time.split_once(':').context("expected HH:MM")?;
    let (hour, minute): (i64, i64) = (hour.parse()?, minute.parse()?);
    if !(0..24).contains(&hour) || !(0..60).contains(&minute) {
        bail!("`{time}` is not a time of day");
    }
    Ok(hour * 3600 + minute * 60)
}

/// Parses `APP_TIMEZONE`: `UTC` or an offset like `+07:00`, in seconds.
pub(crate) fn parse_offset(value: &str) -> anyhow::Result<i64> {
    let value = value.trim();
    if value.is_empty() || value.eq_ignore_ascii_case("utc") || value == "Z" {
        return Ok(0);
    }
    let (sign, rest) = match value.as_bytes()[0] {
        b'+' => (1, &value[1..]),
        b'-' => (-1, &value[1..]),
        _ => bail!("APP_TIMEZONE must be UTC or an offset like +07:00, got `{value}`"),
    };
    Ok(sign * parse_time(rest).with_context(|| format!("APP_TIMEZONE `{value}`"))?)
}

/// The first time after `now` that `when` matches, in unix seconds.
fn next_run(when: When, now: i64, offset: i64) -> i64 {
    let local = now + offset;
    let next_local = match when {
        When::Every(n) => (local / n + 1) * n,
        When::Daily(at) => {
            let midnight = local - local.rem_euclid(86_400);
            let today = midnight + at;
            if today > local { today } else { today + 86_400 }
        }
    };
    next_local - offset
}

#[cfg(test)]
mod tests {
    use super::*;

    // 2026-09-27 10:07:30 UTC
    const NOW: i64 = 1_790_503_650;

    #[test]
    fn intervals_align_to_the_clock() {
        assert_eq!(next_run(When::Every(300), NOW, 0) % 300, 0);
        assert!(next_run(When::Every(300), NOW, 0) - NOW <= 300);
        assert_eq!(next_run(When::Every(60), NOW, 0), NOW + 30);
    }

    #[test]
    fn daily_times_use_the_offset() {
        let wib = parse_offset("+07:00").unwrap();
        // 10:07 UTC is 17:07 WIB; the next 02:00 WIB is 19:00 UTC today.
        let at = next_run(When::Daily(parse_time("02:00").unwrap()), NOW, wib);
        assert_eq!((at + wib).rem_euclid(86_400), 2 * 3600);
        assert_eq!(at - NOW, 8 * 3600 + 52 * 60 + 30);
        let later_today = next_run(When::Daily(parse_time("18:00").unwrap()), NOW, wib);
        assert_eq!(later_today - NOW, 52 * 60 + 30);
    }

    #[test]
    fn every_constructor_picks_its_interval() {
        let mut schedule = Schedule::default();
        schedule
            .every_minute("a", |_| async { Ok(()) })
            .every_minutes(15, "b", |_| async { Ok(()) })
            .hourly("c", |_| async { Ok(()) })
            .every(Duration::ZERO, "d", |_| async { Ok(()) });
        let whens: Vec<When> = schedule.tasks.iter().map(|t| t.when).collect();
        assert_eq!(
            whens,
            [
                When::Every(60),
                When::Every(900),
                When::Every(3600),
                When::Every(1)
            ],
            "a zero interval runs every second rather than never"
        );
        schedule.check().unwrap();
        let upcoming = schedule.upcoming(0);
        assert_eq!(upcoming.len(), 4);
        assert_eq!(upcoming[1].1 % 900, 0, "aligned to the quarter hour");
        assert_eq!(upcoming[2].1 % 3600, 0, "on the hour");
    }

    #[test]
    fn a_bad_daily_time_is_a_boot_error() {
        let mut schedule = Schedule::default();
        schedule.daily_at("25:00", "report", |_| async { Ok(()) });
        assert!(schedule.check().unwrap_err().to_string().contains("report"));
    }

    #[test]
    fn negative_offsets_and_midnight() {
        let ny = parse_offset("-05:00").unwrap();
        // 10:07:30 UTC is 05:07:30 in UTC-5; 04:00 there is tomorrow, 23:30 is tonight.
        let early = next_run(When::Daily(parse_time("04:00").unwrap()), NOW, ny);
        assert_eq!((early + ny).rem_euclid(86_400), 4 * 3600);
        assert_eq!(early - NOW, 22 * 3600 + 52 * 60 + 30);
        let late = next_run(When::Daily(parse_time("23:30").unwrap()), NOW, ny);
        assert_eq!(late - NOW, 18 * 3600 + 22 * 60 + 30);
        // A run due exactly now is the next day's.
        let midnight_utc = NOW - NOW.rem_euclid(86_400);
        assert_eq!(
            next_run(When::Daily(0), midnight_utc, 0),
            midnight_utc + 86_400
        );
        // UTC+14 is already tomorrow: 00:30 there is 10:30 UTC today.
        let kiribati = parse_offset("+14:00").unwrap();
        let soon = next_run(When::Daily(parse_time("00:30").unwrap()), NOW, kiribati);
        assert_eq!(soon - NOW, 22 * 60 + 30);
    }

    #[test]
    fn parses_times_and_offsets() {
        assert_eq!(parse_offset("UTC").unwrap(), 0);
        assert_eq!(parse_offset("-03:30").unwrap(), -(3 * 3600 + 30 * 60));
        assert!(parse_offset("Asia/Jakarta").is_err());
        assert!(parse_time("24:00").is_err());
    }
}
