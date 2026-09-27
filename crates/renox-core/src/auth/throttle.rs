use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Counts failed attempts per key (email and IP) in memory.
pub(crate) struct Throttle {
    max_attempts: u32,
    window: Duration,
    attempts: Mutex<HashMap<String, (u32, Instant)>>,
}

impl Throttle {
    pub fn new(max_attempts: u32, window: Duration) -> Self {
        Self {
            max_attempts,
            window,
            attempts: Mutex::new(HashMap::new()),
        }
    }

    /// Seconds until `key` may try again, if it is locked out.
    pub fn blocked_for(&self, key: &str) -> Option<u64> {
        let attempts = self.attempts.lock().unwrap_or_else(|e| e.into_inner());
        let (count, since) = attempts.get(key)?;
        let elapsed = since.elapsed();
        (*count >= self.max_attempts && elapsed < self.window)
            .then(|| (self.window - elapsed).as_secs().max(1))
    }

    pub fn fail(&self, key: &str) {
        let mut attempts = self.attempts.lock().unwrap_or_else(|e| e.into_inner());
        attempts.retain(|_, (_, since)| since.elapsed() < self.window);
        let entry = attempts
            .entry(key.to_owned())
            .or_insert((0, Instant::now()));
        entry.0 += 1;
    }

    pub fn clear(&self, key: &str) {
        self.attempts
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locks_out_after_max_attempts() {
        let throttle = Throttle::new(3, Duration::from_secs(60));
        for _ in 0..2 {
            throttle.fail("a");
        }
        assert!(throttle.blocked_for("a").is_none());
        throttle.fail("a");
        assert!(throttle.blocked_for("a").unwrap() <= 60);
        assert!(throttle.blocked_for("b").is_none());
        throttle.clear("a");
        assert!(throttle.blocked_for("a").is_none());
    }

    #[test]
    fn windows_expire() {
        let throttle = Throttle::new(1, Duration::from_millis(1));
        throttle.fail("a");
        std::thread::sleep(Duration::from_millis(5));
        assert!(throttle.blocked_for("a").is_none());
    }
}
