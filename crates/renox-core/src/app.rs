use std::collections::HashMap;
use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use anyhow::anyhow;
use axum::Router;
use axum::handler::HandlerWithoutStateExt;
use axum::middleware::{from_fn, from_fn_with_state};
use tokio::net::TcpListener;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

use crate::auth::{Gate, Gates, Throttle, User};
use crate::crypto::parse_key;
use crate::db::{Db, Migration, MigrationStatus, Migrator};
use crate::{
    AppState, Config, Environment, Error, Module, Result, RouteTable, Views, assets, auth, csrf,
    session, view,
};

type Seeder = Box<dyn Fn(Db) -> Pin<Box<dyn Future<Output = Result> + Send>> + Send + Sync>;

const USAGE: &str = "\
Usage: <app> [command]

Commands:
  serve                     Start the web server (default)
  migrate                   Run pending migrations
  migrate:rollback [--step N]
                            Undo the last N batches of migrations (default 1)
  migrate:fresh [--seed]    Drop all tables, run every migration, optionally seed
  migrate:status            List migrations and whether they have run
  db:seed                   Run the seeders
  help                      Show this message";

/// The application builder.
///
/// ```ignore
/// fn main() -> renox::Result {
///     App::new()
///         .migrations(renox::migrations!())
///         .module(Produk)
///         .seeder(seed)
///         .run()
/// }
/// ```
///
/// The built binary is also the app's command line, like Laravel's artisan:
/// `my-app migrate`, `my-app db:seed`, `my-app help`.
pub struct App {
    config: Option<Config>,
    modules: Vec<Box<dyn Module>>,
    migrations: Vec<Migration>,
    seeders: Vec<Seeder>,
    gates: HashMap<String, Gate>,
}

impl App {
    /// Creates an application that loads its configuration from `.env` on start.
    pub fn new() -> Self {
        Self {
            config: None,
            modules: Vec::new(),
            migrations: Vec::new(),
            seeders: Vec::new(),
            gates: HashMap::new(),
        }
    }

    /// Uses the given configuration instead of loading it from the environment.
    pub fn with_config(config: Config) -> Self {
        Self {
            config: Some(config),
            ..Self::new()
        }
    }

    pub fn module(mut self, module: impl Module) -> Self {
        self.modules.push(Box::new(module));
        self
    }

    /// Registers app-level migrations, usually `renox::migrations!()`.
    pub fn migrations(mut self, migrations: &[Migration]) -> Self {
        self.migrations.extend_from_slice(migrations);
        self
    }

    /// Registers a seeder for `db:seed`. Seeders run in registration order.
    ///
    /// ```ignore
    /// App::new().seeder(|db| async move {
    ///     Produk::create_many(&db, 50).await?;
    ///     Ok(())
    /// })
    /// ```
    pub fn seeder<F, Fut>(mut self, seeder: F) -> Self
    where
        F: Fn(Db) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result> + Send + 'static,
    {
        self.seeders.push(Box::new(move |db| Box::pin(seeder(db))));
        self
    }

    /// Defines a gate: an ability that depends only on the user.
    ///
    /// ```ignore
    /// App::new().gate("admin", |user| user.email.ends_with("@toko.id"))
    /// // in a handler: auth.gate("admin")?;   in a template: {% if can('admin') %}
    /// ```
    pub fn gate(
        mut self,
        name: &str,
        check: impl Fn(&User) -> bool + Send + Sync + 'static,
    ) -> Self {
        self.gates.insert(name.to_owned(), Arc::new(check));
        self
    }

    /// Connects to the database and builds the router, without serving.
    pub async fn boot(self) -> Result<Kernel> {
        let config = match self.config {
            Some(config) => config,
            None => Config::load()?,
        };
        let mut migrations = self.migrations;
        for module in &self.modules {
            migrations.extend_from_slice(module.migrations());
        }
        let migrator = Migrator::new(migrations)?;
        let db = crate::db::connect(&config).await?;
        let router = build_router(config, &self.modules, db.clone(), Arc::new(self.gates))?;
        Ok(Kernel {
            router,
            db,
            migrator,
            seeders: self.seeders,
        })
    }

    /// Builds the router without starting a server, e.g. for tests.
    pub async fn into_router(self) -> Result<Router> {
        Ok(self.boot().await?.router)
    }

    /// Runs the command given on the command line (`serve` by default) on a
    /// new Tokio runtime.
    pub fn run(self) -> Result {
        let args: Vec<String> = std::env::args().skip(1).collect();
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?
            .block_on(self.command(&args))
    }

    /// Starts the server on the current Tokio runtime.
    pub async fn serve(self) -> Result {
        self.command(&[]).await
    }

    async fn command(self, args: &[String]) -> Result {
        let command = args.first().map(String::as_str).unwrap_or("serve");
        if matches!(command, "help" | "--help" | "-h") {
            println!("{USAGE}");
            return Ok(());
        }

        let config = match self.config.clone() {
            Some(config) => config,
            None => Config::load()?,
        };
        init_tracing(&config, command == "serve");
        if command == "serve" && config.key.is_none() && config.env != Environment::Testing {
            tracing::warn!(
                "APP_KEY is not set; using a temporary key, so sessions end on restart. \
                 Run `rnx key:generate`."
            );
        }
        let name = config.name.clone();
        let addr = config.addr();
        let kernel = App {
            config: Some(config),
            ..self
        }
        .boot()
        .await?;

        match command {
            "serve" => {
                let listener = TcpListener::bind(addr).await?;
                tracing::info!("{name} listening on http://{}", listener.local_addr()?);
                let service = kernel
                    .router
                    .into_make_service_with_connect_info::<SocketAddr>();
                axum::serve(listener, service)
                    .with_graceful_shutdown(shutdown_signal())
                    .await?;
                tracing::info!("{name} stopped");
            }
            "migrate" => print_done("Migrated", &kernel.migrate().await?),
            "migrate:rollback" => {
                let steps = flag_value(args, "--step")?.unwrap_or(1);
                print_done("Rolled back", &kernel.rollback(steps).await?);
            }
            "migrate:fresh" => {
                println!("Dropped all tables.");
                print_done("Migrated", &kernel.fresh().await?);
                if args.iter().any(|a| a == "--seed") {
                    kernel.seed().await?;
                    println!("Seeded.");
                }
            }
            "migrate:status" => {
                for m in kernel.migration_status().await? {
                    match m.batch {
                        Some(batch) => println!("  ran (batch {batch})  {}", m.name),
                        None => println!("  pending          {}", m.name),
                    }
                }
            }
            "db:seed" => {
                kernel.seed().await?;
                println!("Seeded.");
            }
            other => return Err(anyhow!("unknown command `{other}`\n\n{USAGE}").into()),
        }
        Ok(())
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

/// A booted application: its router, database and maintenance commands.
pub struct Kernel {
    router: Router,
    db: Db,
    migrator: Migrator,
    seeders: Vec<Seeder>,
}

impl Kernel {
    pub fn router(&self) -> Router {
        self.router.clone()
    }

    pub fn db(&self) -> &Db {
        &self.db
    }

    /// Runs pending migrations; returns their names.
    pub async fn migrate(&self) -> Result<Vec<String>> {
        let done = self.migrator.run(&self.db).await?;
        Ok(done.into_iter().map(str::to_owned).collect())
    }

    /// Undoes the last `batches` batches of migrations; returns their names.
    pub async fn rollback(&self, batches: u32) -> Result<Vec<String>> {
        Ok(self.migrator.rollback(&self.db, batches).await?)
    }

    /// Drops every table and runs all migrations.
    pub async fn fresh(&self) -> Result<Vec<String>> {
        let done = self.migrator.fresh(&self.db).await?;
        Ok(done.into_iter().map(str::to_owned).collect())
    }

    pub async fn migration_status(&self) -> Result<Vec<MigrationStatus>> {
        Ok(self.migrator.status(&self.db).await?)
    }

    /// Runs every seeder in registration order.
    pub async fn seed(&self) -> Result {
        for seeder in &self.seeders {
            seeder(self.db.clone()).await?;
        }
        Ok(())
    }
}

fn print_done(verb: &str, names: &[String]) {
    if names.is_empty() {
        println!("Nothing to do.");
    }
    for name in names {
        println!("{verb}: {name}");
    }
}

fn flag_value(args: &[String], flag: &str) -> Result<Option<u32>> {
    let Some(i) = args.iter().position(|a| a == flag) else {
        return Ok(None);
    };
    match args.get(i + 1).and_then(|v| v.parse().ok()) {
        Some(value) => Ok(Some(value)),
        None => Err(anyhow!("{flag} needs a number").into()),
    }
}

fn build_router(
    config: Config,
    modules: &[Box<dyn Module>],
    db: Db,
    gates: Gates,
) -> Result<Router> {
    crate::error::set_debug(config.debug);

    let key = match &config.key {
        Some(key) => parse_key(key)?,
        None => parse_key(&crate::generate_key())?,
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
        db,
        key,
        gates,
        // Five failed logins per email and IP per minute.
        throttle: Arc::new(Throttle::new(5, Duration::from_secs(60))),
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
        .layer(from_fn_with_state(state.clone(), auth::middleware))
        .layer(from_fn_with_state(state.clone(), session::middleware))
        .merge(assets::router())
        .layer(TraceLayer::new_for_http())
        .with_state(state))
}

/// Serving logs requests; other commands only log warnings and errors.
fn init_tracing(config: &Config, serving: bool) {
    let default = match (serving, config.debug) {
        (true, true) => "info,renox=debug",
        (true, false) => "info",
        (false, _) => "warn",
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
