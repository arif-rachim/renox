use std::sync::Arc;

use axum::Router;
use axum::handler::HandlerWithoutStateExt;
use axum::middleware::{from_fn, from_fn_with_state};
use tokio::net::TcpListener;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

use crate::crypto::parse_key;
use crate::{
    AppState, Config, Environment, Error, Module, Result, RouteTable, Views, assets, csrf, session,
    view,
};

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
        build_router(config, &self.modules)
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
        let router = build_router(config, &self.modules)?;

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

fn build_router(config: Config, modules: &[Box<dyn Module>]) -> Result<Router> {
    crate::error::set_debug(config.debug);

    let key = match &config.key {
        Some(key) => parse_key(key)?,
        None => {
            if config.env != Environment::Testing {
                tracing::warn!(
                    "APP_KEY is not set; using a temporary key, so sessions end on restart. \
                     Run `renox key:generate`."
                );
            }
            parse_key(&crate::generate_key())?
        }
    };

    let mut router = Router::new();
    let mut routes = RouteTable::default();
    for module in modules {
        tracing::debug!(module = module.name(), "registering module");
        let (module_router, names) = module.routes().into_parts();
        router = router.merge(module_router);
        for (name, path) in names {
            routes.insert(name, path)?;
        }
    }
    let routes = Arc::new(routes);

    let public = config.public_path.clone();
    let views = Views::new(&config, routes.clone());
    let state = AppState {
        config: Arc::new(config),
        routes,
        views,
        key,
    };

    let not_found = || async { Error::NotFound };
    let router = if public.is_dir() {
        router.fallback_service(ServeDir::new(public).not_found_service(not_found.into_service()))
    } else {
        router.fallback(not_found)
    };

    Ok(router
        .layer(from_fn_with_state(state.clone(), view::middleware))
        .layer(from_fn(csrf::middleware))
        .layer(from_fn_with_state(state.clone(), session::middleware))
        .merge(assets::router())
        .layer(TraceLayer::new_for_http())
        .with_state(state))
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
