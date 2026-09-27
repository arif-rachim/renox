use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Failed logins, counted in memory three ways, so neither rotating IPs nor
/// rotating emails gets around the lock:
///
/// - 5 per email and IP a minute, the lock a user who mistyped runs into;
/// - 20 per email in 15 minutes, whatever the IP (guessing one account from
///   many addresses);
/// - 50 per IP in 15 minutes, whatever the email (trying many accounts).
pub(crate) struct LoginThrottle {
    pair: Throttle,
    account: Throttle,
    ip: Throttle,
}

impl LoginThrottle {
    pub fn new() -> Self {
        Self {
            pair: Throttle::new(5, Duration::from_secs(60)),
            account: Throttle::new(20, Duration::from_secs(15 * 60)),
            ip: Throttle::new(50, Duration::from_secs(15 * 60)),
        }
    }

    fn keys(email: &str, ip: Option<IpAddr>) -> [String; 3] {
        let email = email.trim().to_lowercase();
        let ip = ip.map(|ip| ip.to_string()).unwrap_or_default();
        [format!("{email}|{ip}"), email, ip]
    }

    /// Seconds until this email may try again from this IP, if it is locked out.
    pub fn blocked_for(&self, email: &str, ip: Option<IpAddr>) -> Option<u64> {
        let [pair, account, ip] = Self::keys(email, ip);
        [
            self.pair.blocked_for(&pair),
            self.account.blocked_for(&account),
            self.ip.blocked_for(&ip),
        ]
        .into_iter()
        .flatten()
        .max()
    }

    pub fn fail(&self, email: &str, ip: Option<IpAddr>) {
        let [pair, account, ip] = Self::keys(email, ip);
        self.pair.fail(&pair);
        self.account.fail(&account);
        self.ip.fail(&ip);
    }

    /// After a successful login. The IP's count stays: one account the
    /// guesser owns shouldn't reset their tries at the others.
    pub fn clear(&self, email: &str, ip: Option<IpAddr>) {
        let [pair, account, _] = Self::keys(email, ip);
        self.pair.clear(&pair);
        self.account.clear(&account);
    }
}

/// Counts failed attempts per key in memory.
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
    fn locks_an_account_across_ips_and_an_ip_across_accounts() {
        let throttle = LoginThrottle::new();
        for i in 0..20 {
            throttle.fail("Arif@example.com", format!("10.0.0.{i}").parse().ok());
        }
        assert!(
            throttle
                .blocked_for("arif@example.com", "10.9.9.9".parse().ok())
                .is_some()
        );
        assert!(
            throttle
                .blocked_for("budi@example.com", "10.9.9.9".parse().ok())
                .is_none()
        );

        let throttle = LoginThrottle::new();
        let ip = "10.0.0.1".parse().ok();
        for i in 0..50 {
            throttle.fail(&format!("user{i}@example.com"), ip);
        }
        assert!(throttle.blocked_for("new@example.com", ip).is_some());
        assert!(
            throttle
                .blocked_for("new@example.com", "10.0.0.2".parse().ok())
                .is_none()
        );
    }

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
