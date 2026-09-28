//! The current time, with an offset tests can move (`TestApp::travel`).

use std::future::Future;
use std::time::{Duration, SystemTime};

tokio::task_local! {
    /// Seconds added to the real clock in this task (tests only).
    static OFFSET: i64;
}

/// Now, plus the task's offset.
pub(crate) fn system_now() -> SystemTime {
    let offset = OFFSET.try_with(|o| *o).unwrap_or(0);
    let now = SystemTime::now();
    if offset >= 0 {
        now + Duration::from_secs(offset as u64)
    } else {
        now - Duration::from_secs(offset.unsigned_abs())
    }
}

/// Seconds since the Unix epoch.
pub(crate) fn unix_secs() -> i64 {
    system_now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

/// Milliseconds since the Unix epoch.
pub(crate) fn unix_millis() -> i64 {
    system_now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

/// Runs `fut` with the clock `seconds` ahead (behind when negative).
pub(crate) async fn with_offset<F: Future>(seconds: i64, fut: F) -> F::Output {
    OFFSET.scope(seconds, fut).await
}
