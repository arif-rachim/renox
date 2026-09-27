//! A key-value cache for expensive results.
//!
//! ```
//! # use renox::prelude::*;
//! # use std::time::Duration;
//! # #[derive(Model, serde::Serialize, serde::Deserialize, Default)] struct Produk { id: i64, nama: String }
//! # async fn demo(state: AppState) -> Result {
//! let menu: Vec<Produk> = state.cache.remember("menu", Duration::from_secs(600), || async {
//!     Produk::query().order_by("nama").get(&state.db).await
//! }).await?;
//! state.cache.forget("menu").await?;   // after the menu changes
//! # let _ = menu; Ok(()) }
//! ```
//!
//! `CACHE_STORE=memory` (default) keeps values in this process; `database`
//! keeps them in the `cache` table, so they survive restarts and are shared
//! with `queue:work` processes. Values are stored as JSON. Keys starting with
//! `renox:` belong to the framework (e.g. scheduler claims).

use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use anyhow::bail;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::Result;
use crate::db::{Db, Migration};
use crate::queue::unix_now;

pub(crate) const MIGRATION: Migration =
    crate::db::framework_migration!("cache", "00010101000200_create_cache_table");

/// Entries the memory store keeps before dropping expired ones on write.
const SWEEP_AT: usize = 10_000;

/// Value and expiry (unix seconds) per key.
type Entries = HashMap<String, (Value, Option<i64>)>;

#[derive(Clone)]
enum Store {
    Memory(Arc<Mutex<Entries>>),
    Database(Db),
}

#[derive(Clone)]
pub struct Cache {
    store: Store,
    /// One lock per key being computed by `remember` in this process.
    computing: Arc<Mutex<HashMap<String, Weak<tokio::sync::Mutex<()>>>>>,
}

/// Unix seconds when a value stored now for `ttl` expires, rounded up.
fn expiry(ttl: Option<Duration>) -> Option<i64> {
    ttl.map(|ttl| unix_now() + ttl.as_secs() as i64 + i64::from(ttl.subsec_nanos() > 0))
}

/// Keys of framework rows, which `flush` keeps.
const FRAMEWORK_PREFIX: &str = "renox:";

impl Cache {
    pub(crate) fn new(store: &str, db: Db) -> anyhow::Result<Self> {
        let store = match store {
            "memory" => Store::Memory(Arc::default()),
            "database" => Store::Database(db),
            other => bail!("CACHE_STORE must be memory or database, got `{other}`"),
        };
        Ok(Self {
            store,
            computing: Arc::default(),
        })
    }

    async fn raw(&self, key: &str) -> Result<Option<Value>> {
        let now = unix_now();
        match &self.store {
            Store::Memory(map) => {
                let map = map.lock().unwrap_or_else(|e| e.into_inner());
                Ok(map
                    .get(key)
                    .filter(|(_, expires)| expires.is_none_or(|at| at > now))
                    .map(|(value, _)| value.clone()))
            }
            Store::Database(db) => {
                let text: Option<String> = crate::db::sql(
                    "SELECT value FROM cache WHERE key = ? AND (expires_at IS NULL OR expires_at > ?)",
                )
                .bind(key)
                .bind(now)
                .scalar_optional(db)
                .await?;
                Ok(text.and_then(|t| serde_json::from_str(&t).ok()))
            }
        }
    }

    /// The cached value, if present and not expired. A value of another
    /// type than `T` is an error.
    pub async fn get<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        match self.raw(key).await? {
            None => Ok(None),
            Some(value) => serde_json::from_value(value).map(Some).map_err(|err| {
                anyhow::Error::new(err)
                    .context(format!(
                        "the cached `{key}` is not a {}",
                        std::any::type_name::<T>()
                    ))
                    .into()
            }),
        }
    }

    pub async fn has(&self, key: &str) -> Result<bool> {
        Ok(self.raw(key).await?.is_some())
    }

    /// Stores `value` for `ttl`, or until forgotten when `ttl` is `None`. A
    /// zero `ttl` stores nothing (and forgets the key).
    pub async fn put(&self, key: &str, value: &impl Serialize, ttl: Option<Duration>) -> Result {
        if ttl == Some(Duration::ZERO) {
            return self.forget(key).await;
        }
        let value = serde_json::to_value(value)?;
        let expires = expiry(ttl);
        match &self.store {
            Store::Memory(map) => {
                let mut map = map.lock().unwrap_or_else(|e| e.into_inner());
                if map.len() >= SWEEP_AT {
                    let now = unix_now();
                    map.retain(|_, (_, at)| at.is_none_or(|at| at > now));
                }
                map.insert(key.to_owned(), (value, expires));
            }
            Store::Database(db) => {
                crate::db::sql(
                    "INSERT INTO cache (key, value, expires_at) VALUES (?, ?, ?) \
                     ON CONFLICT (key) DO UPDATE SET value = excluded.value, expires_at = excluded.expires_at",
                )
                .bind(key)
                .bind(value.to_string())
                .bind(expires)
                .execute(db)
                .await?;
            }
        }
        Ok(())
    }

    /// Returns the cached value, or runs `compute`, caches its result for
    /// `ttl` and returns it. Errors are not cached. Concurrent calls for the
    /// same key in this process compute once; the others wait for it. A
    /// cached value of another type (e.g. after a deploy) is computed again.
    pub async fn remember<T, F, Fut>(&self, key: &str, ttl: Duration, compute: F) -> Result<T>
    where
        T: Serialize + DeserializeOwned,
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T>>,
    {
        if let Some(value) = self.cached(key).await? {
            return Ok(value);
        }
        let lock = self.computing_lock(key);
        let _computing = lock.lock().await;
        if let Some(value) = self.cached(key).await? {
            return Ok(value);
        }
        let value = compute().await?;
        self.put(key, &value, Some(ttl)).await?;
        Ok(value)
    }

    /// Like `get`, but a value of another type is a miss.
    async fn cached<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        Ok(self
            .raw(key)
            .await?
            .and_then(|v| serde_json::from_value(v).ok()))
    }

    fn computing_lock(&self, key: &str) -> Arc<tokio::sync::Mutex<()>> {
        let mut locks = self.computing.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(lock) = locks.get(key).and_then(Weak::upgrade) {
            return lock;
        }
        locks.retain(|_, lock| lock.strong_count() > 0);
        let lock = Arc::new(tokio::sync::Mutex::new(()));
        locks.insert(key.to_owned(), Arc::downgrade(&lock));
        lock
    }

    pub async fn forget(&self, key: &str) -> Result {
        match &self.store {
            Store::Memory(map) => {
                map.lock().unwrap_or_else(|e| e.into_inner()).remove(key);
            }
            Store::Database(db) => {
                crate::db::sql("DELETE FROM cache WHERE key = ?")
                    .bind(key)
                    .execute(db)
                    .await?;
            }
        }
        Ok(())
    }

    /// Removes everything the app cached (not the framework's `renox:` rows).
    pub async fn flush(&self) -> Result {
        match &self.store {
            Store::Memory(map) => map
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .retain(|key, _| key.starts_with(FRAMEWORK_PREFIX)),
            Store::Database(db) => {
                crate::db::sql("DELETE FROM cache WHERE key NOT LIKE ?")
                    .bind(format!("{FRAMEWORK_PREFIX}%"))
                    .execute(db)
                    .await?;
            }
        }
        Ok(())
    }
}
