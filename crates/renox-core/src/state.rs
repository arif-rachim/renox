use std::sync::Arc;

use crate::Config;

/// Shared state available to every handler through `State<AppState>`.
#[derive(Debug, Clone)]
pub struct AppState {
    pub config: Arc<Config>,
}

impl AppState {
    pub fn new(config: Config) -> Self {
        Self {
            config: Arc::new(config),
        }
    }
}
