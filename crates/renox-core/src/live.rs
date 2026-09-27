//! Browser live reload while developing (`APP_ENV=local` with `APP_DEBUG` on).
//!
//! The page subscribes to `/_renox/live` (Server-Sent Events). When a file
//! under the views, public or lang directories changes, the server sends
//! `reload`. When `rnx serve` restarts the app, the browser reconnects, sees a
//! new boot id and reloads too. Polling file times keeps it portable.

use std::convert::Infallible;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use axum::Router;
use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::routing::get;
use futures_util::stream::{self, Stream, StreamExt};
use tokio::sync::broadcast;

use crate::crypto::random_token;
use crate::{AppState, Error, Result};

const POLL: Duration = Duration::from_millis(500);

#[derive(Clone, Copy)]
enum Message {
    Reload,
    Stop,
}

pub(crate) struct Live {
    boot: String,
    tx: broadcast::Sender<Message>,
}

fn fingerprint(paths: &[PathBuf]) -> Vec<(PathBuf, SystemTime, u64)> {
    fn walk(path: &Path, out: &mut Vec<(PathBuf, SystemTime, u64)>) {
        let Ok(meta) = path.metadata() else { return };
        if meta.is_dir() {
            for entry in std::fs::read_dir(path).into_iter().flatten().flatten() {
                walk(&entry.path(), out);
            }
        } else {
            let modified = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
            out.push((path.to_path_buf(), modified, meta.len()));
        }
    }
    let mut files = Vec::new();
    for path in paths {
        walk(path, &mut files);
    }
    files.sort();
    files
}

impl Live {
    /// Starts watching `paths`; must run inside the Tokio runtime.
    pub(crate) fn start(paths: Vec<PathBuf>) -> Arc<Self> {
        let (tx, _) = broadcast::channel(16);
        let live = Arc::new(Self {
            boot: random_token()[..12].to_owned(),
            tx,
        });
        let watcher = Arc::downgrade(&live);
        tokio::spawn(async move {
            let mut last = fingerprint(&paths);
            loop {
                tokio::time::sleep(POLL).await;
                let Some(live) = watcher.upgrade() else { break };
                let now = fingerprint(&paths);
                if now != last {
                    last = now;
                    let _ = live.tx.send(Message::Reload);
                }
            }
        });
        live
    }

    /// Ends open event streams, so a graceful shutdown doesn't wait for them.
    pub(crate) fn stop(&self) {
        let _ = self.tx.send(Message::Stop);
    }
}

pub(crate) fn router() -> Router<AppState> {
    Router::new().route("/_renox/live", get(events))
}

async fn events(
    State(state): State<AppState>,
) -> Result<Sse<impl Stream<Item = std::result::Result<Event, Infallible>>>> {
    let live = state.live.clone().ok_or(Error::NotFound)?;
    let hello = Event::default()
        .event("boot")
        .data(live.boot.clone())
        .retry(Duration::from_millis(500));
    let updates = stream::unfold(live.tx.subscribe(), |mut rx| async move {
        loop {
            match rx.recv().await {
                Ok(Message::Reload) => {
                    return Some((Event::default().event("reload").data("changed"), rx));
                }
                Ok(Message::Stop) | Err(broadcast::error::RecvError::Closed) => return None,
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
            }
        }
    });
    let events = stream::once(async move { hello }).chain(updates).map(Ok);
    Ok(Sse::new(events).keep_alive(KeepAlive::default()))
}
