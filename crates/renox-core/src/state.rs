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
    pub translator: Arc<Translator>,
    /// Live reload, only while developing locally.
    pub(crate) live: Option<Arc<crate::live::Live>>,
    pub(crate) listeners: Listeners,
    pub(crate) key: Key,
    pub(crate) gates: Gates,
    pub(crate) throttle: Arc<LoginThrottle>,
    pub(crate) security: Arc<crate::security::Security>,
    pub(crate) webhooks: crate::webhook::Handlers,
}

impl AppState {
    /// The URL path of a named route, e.g. `state.url("produk.show", &[&id])`.
    pub fn url(&self, name: &str, params: &[&dyn Display]) -> Result<String> {
        Ok(self.routes.url(name, params)?)
    }
}
