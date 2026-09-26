use axum::Router;
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

use crate::{AppState, Config, Error, Module, Result};

/// The application builder.
///
/// ```ignore
/// fn main() -> renox::Result {
///     App::new().module(Produk).run()
/// }
/// ```
pub struct App {
    config: Option<Config>,
    modules: Vec<Box<dyn Module>>,
}

impl App {
    /// Creates an application that loads its configuration from `.env` on start.
    pub fn new() -> Self {
        Self {
            config: None,
            modules: Vec::new(),
        }
    }

    /// Uses the given configuration instead of loading it from the environment.
    pub fn with_config(config: Config) -> Self {
        Self {
            config: Some(config),
            modules: Vec::new(),
        }
    }

    pub fn module(mut self, module: impl Module) -> Self {
        self.modules.push(Box::new(module));
        self
    }

    /// Builds the router without starting a server, e.g. for tests.
    pub fn into_router(self) -> Result<Router> {
        let config = match self.config {
            Some(config) => config,
            None => Config::load()?,
        };
        Ok(build_router(config, &self.modules))
    }

    /// Starts the server on a new Tokio runtime and blocks until shutdown.
    pub fn run(self) -> Result {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?
            .block_on(self.serve())
    }

    /// Starts the server on the current Tokio runtime.
    pub async fn serve(self) -> Result {
        let config = match self.config {
            Some(config) => config,
            None => Config::load()?,
        };
        init_tracing(&config);

        let addr = config.addr();
        let name = config.name.clone();
        let router = build_router(config, &self.modules);

        let listener = TcpListener::bind(addr).await?;
        tracing::info!("{name} listening on http://{}", listener.local_addr()?);
        axum::serve(listener, router)
            .with_graceful_shutdown(shutdown_signal())
            .await?;
        tracing::info!("{name} stopped");
        Ok(())
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

fn build_router(config: Config, modules: &[Box<dyn Module>]) -> Router {
    crate::error::set_debug(config.debug);

    let mut router = Router::new();
    for module in modules {
        tracing::debug!(module = module.name(), "registering module");
        router = router.merge(module.routes());
    }

    router
        .fallback(|| async { Error::NotFound })
        .layer(TraceLayer::new_for_http())
        .with_state(AppState::new(config))
}

fn init_tracing(config: &Config) {
    let default = if config.debug {
        "info,renox=debug"
    } else {
        "info"
    };
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default));
    let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(_) => std::future::pending().await,
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    tracing::info!("shutdown signal received");
}
