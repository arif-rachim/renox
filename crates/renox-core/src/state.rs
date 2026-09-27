use std::fmt::Display;
use std::sync::Arc;

use cookie::Key;

use crate::auth::{Gates, Throttle};
use crate::db::Db;
use crate::mail::Mailer;
use crate::{Config, Result, RouteTable, Views};

/// Shared state available to every handler through `State<AppState>`.
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub routes: Arc<RouteTable>,
    pub views: Views,
    pub db: Db,
    pub mailer: Mailer,
    pub(crate) key: Key,
    pub(crate) gates: Gates,
    pub(crate) throttle: Arc<Throttle>,
}

impl AppState {
    /// The URL path of a named route, e.g. `state.url("produk.show", &[&id])`.
    pub fn url(&self, name: &str, params: &[&dyn Display]) -> Result<String> {
        Ok(self.routes.url(name, params)?)
    }
}
