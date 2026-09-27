//! Per-route rate limits: `Routes::throttle(60, Duration::from_secs(60))`.
//!
//! Requests are counted per logged-in user, or per IP address for guests
//! (see [`ClientIp`](crate::ClientIp) behind a proxy), in
//! fixed windows kept in memory. Over the limit, the response is 429 with
//! `Retry-After`; every allowed response carries `X-RateLimit-Limit` and
//! `X-RateLimit-Remaining`.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use axum::extract::Request;
use axum::http::HeaderValue;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::Error;
use crate::auth::CurrentUser;

/// Keys kept before stale windows are dropped on the next hit.
const SWEEP_AT: usize = 10_000;

pub(crate) struct Limiter {
    max: u32,
    window: Duration,
    hits: Mutex<HashMap<String, (u32, Instant)>>,
}

pub(crate) enum Verdict {
    Allowed { remaining: u32 },
    Limited { retry_after: u64 },
}

impl Limiter {
    pub fn new(max: u32, window: Duration) -> Self {
        Self {
            max: max.max(1),
            window,
            hits: Mutex::new(HashMap::new()),
        }
    }

    pub fn hit(&self, key: &str) -> Verdict {
        let mut hits = self.hits.lock().unwrap_or_else(|e| e.into_inner());
        if hits.len() >= SWEEP_AT {
            hits.retain(|_, (_, start)| start.elapsed() < self.window);
        }
        let entry = hits.entry(key.to_owned()).or_insert((0, Instant::now()));
        if entry.1.elapsed() >= self.window {
            *entry = (0, Instant::now());
        }
        if entry.0 >= self.max {
            let left = self.window.saturating_sub(entry.1.elapsed());
            return Verdict::Limited {
                retry_after: left.as_secs().max(1),
            };
        }
        entry.0 += 1;
        Verdict::Allowed {
            remaining: self.max - entry.0,
        }
    }

    pub fn max(&self) -> u32 {
        self.max
    }
}

fn key(req: &Request) -> String {
    let user = req
        .extensions()
        .get::<CurrentUser>()
        .and_then(|c| c.user.as_ref().map(|u| u.id));
    match user {
        Some(id) => format!("user:{id}"),
        None => match crate::ClientIp::of(req) {
            Some(ip) => format!("ip:{ip}"),
            None => "ip:unknown".to_owned(),
        },
    }
}

pub(crate) async fn check(limiter: &Limiter, req: Request, next: Next) -> Response {
    match limiter.hit(&key(&req)) {
        Verdict::Limited { retry_after } => {
            let mut res = Error::TooManyRequests.into_response();
            if let Ok(value) = HeaderValue::from_str(&retry_after.to_string()) {
                res.headers_mut().insert("retry-after", value);
            }
            res
        }
        Verdict::Allowed { remaining } => {
            let mut res = next.run(req).await;
            let headers = res.headers_mut();
            headers.insert("x-ratelimit-limit", limiter.max().into());
            headers.insert("x-ratelimit-remaining", remaining.into());
            res
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_per_key_and_window() {
        let limiter = Limiter::new(2, Duration::from_millis(50));
        assert!(matches!(
            limiter.hit("a"),
            Verdict::Allowed { remaining: 1 }
        ));
        assert!(matches!(
            limiter.hit("a"),
            Verdict::Allowed { remaining: 0 }
        ));
        assert!(matches!(
            limiter.hit("a"),
            Verdict::Limited { retry_after: 1 }
        ));
        assert!(matches!(limiter.hit("b"), Verdict::Allowed { .. }));
        std::thread::sleep(Duration::from_millis(60));
        assert!(matches!(
            limiter.hit("a"),
            Verdict::Allowed { remaining: 1 }
        ));
    }
}
