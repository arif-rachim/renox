use std::fmt::Display;
use std::sync::Arc;

use cookie::Key;

use crate::auth::{Gates, LoginThrottle};
use crate::cache::Cache;
use crate::db::Db;
use crate::events::Listeners;
use crate::i18n::Translator;
use crate::mail::Mailer;
use crate::queue::Queue;
use crate::storage::Storage;
use crate::{Config, Result, RouteTable, Views};

/// Shared state available to every handler through `State<AppState>`.
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub routes: Arc<RouteTable>,
    pub views: Views,
    pub db: Db,
    pub mailer: Mailer,
    pub queue: Queue,
    pub cache: Cache,
    pub storage: Storage,
    /// Calls other services; faked in tests (`TestApp::fake_http`).
    pub http: crate::http::Http,
    pub translator: Arc<Translator>,
    /// Live reload, only while developing locally.
    pub(crate) live: Option<Arc<crate::live::Live>>,
    pub(crate) listeners: Listeners,
    pub(crate) key: Key,
    pub(crate) gates: Gates,
    pub(crate) async_gates: Arc<std::collections::HashMap<String, crate::auth::AsyncGate>>,
    pub(crate) throttle: Arc<LoginThrottle>,
    pub(crate) security: Arc<crate::security::Security>,
    pub(crate) webhooks: crate::webhook::Handlers,
    /// Values every view gets (`App::share`).
    pub(crate) shares: Arc<Vec<(String, crate::view::ShareFn)>>,
    /// The app's notification channels (`App::channel`).
    pub(crate) channels:
        Arc<std::collections::HashMap<String, crate::auth::notifications::ChannelFn>>,
    /// The app's own values (`App::provide`).
    pub(crate) provided: crate::provided::ProvidedMap,
}

impl AppState {
    /// The URL path of a named route, e.g. `state.url("produk.show", &[&id])`.
    pub fn url(&self, name: &str, params: &[&dyn Display]) -> Result<String> {
        Ok(self.routes.url(name, params)?)
    }

    /// Encrypts `plain` with `APP_KEY` (AES-256-GCM), e.g. an API secret a
    /// user saves, before storing it. The result is base64 text; only
    /// [`AppState::decrypt`] with the same key reads it back, and any change
    /// to it is detected. Rotating `APP_KEY` makes old values unreadable.
    ///
    /// ```
    /// # use renox::prelude::*;
    /// # fn demo(state: AppState) -> Result {
    /// let sealed = state.encrypt("sk_live_123");
    /// assert_eq!(state.decrypt(&sealed)?, "sk_live_123");
    /// # Ok(()) }
    /// ```
    pub fn encrypt(&self, plain: &str) -> String {
        crate::crypto::seal(&self.key, plain)
    }

    /// Reads a value from [`AppState::encrypt`]. Fails if it was changed or
    /// sealed with another key.
    pub fn decrypt(&self, sealed: &str) -> Result<String> {
        Ok(crate::crypto::open(&self.key, sealed)?)
    }
}
