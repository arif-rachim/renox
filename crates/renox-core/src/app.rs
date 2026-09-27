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
use tokio::sync::watch;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

use crate::auth::{Gate, Throttle, User};
use crate::crypto::parse_key;
use crate::db::{Db, Migration, MigrationStatus, Migrator};
use crate::events::Event;
use crate::mail::Mailer;
use crate::queue::{Handlers, Job, Queue, Worker};
use crate::routing::RouteInfo;
use crate::schedule::{Schedule, parse_offset};
use crate::{
    AppState, Config, Environment, Error, Module, Registry, Result, RouteTable, Views, assets,
    auth, csrf, session, view,
};

type Seeder = Box<dyn Fn(Db) -> Pin<Box<dyn Future<Output = Result> + Send>> + Send + Sync>;

/// How long `serve` waits for running jobs after a shutdown signal.
const SHUTDOWN_GRACE: Duration = Duration::from_secs(30);

const USAGE: &str = "\
Usage: <app> [command]

Commands:
  serve                     Start the web server, queue workers and scheduler (default)
  migrate                   Run pending migrations
  migrate:rollback [--step N]
                            Undo the last N batches of migrations (default 1)
  migrate:fresh [--seed]    Drop all tables, run every migration, optionally seed
  migrate:status            List migrations and whether they have run
  db:seed                   Run the seeders
  queue:work [--queue a,b] [--workers N] [--once]
                            Run queued jobs (until stopped, or --once for what's there)
  queue:failed              List failed jobs
  queue:retry <id|all>      Put failed jobs back on the queue
  queue:flush               Delete failed jobs
  schedule:list             List scheduled tasks and when they run next
  schedule:work             Run scheduled tasks (when SCHEDULER=false for serve)
  route:list                List every route with its name, module and guards
  db:shell                  Run SQL against the database (`.tables`, `.quit`)
  down [--secret S] [--retry N]
                            Maintenance mode: answer 503 (visit /S to bypass it)
  up                        Leave maintenance mode
  help                      Show this message";

/// The application builder.
///
/// ```ignore
/// fn main() -> renox::Result {
///     App::new()
///         .migrations(renox::migrations!())
///         .module(Produk)
///         .job::<SendReceipt>()
///         .schedule(|s| { s.daily_at("02:00", "cleanup", cleanup); })
///         .seeder(seed)
///         .run()
/// }
/// ```
///
/// The built binary is also the app's command line, like Laravel's artisan:
/// `my-app migrate`, `my-app queue:work`, `my-app help`.
pub struct App {
    config: Option<Config>,
    modules: Vec<Box<dyn Module>>,
    migrations: Vec<Migration>,
    seeders: Vec<Seeder>,
    gates: HashMap<String, Gate>,
    registry: Registry,
    embedded: Option<crate::Embedded>,
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
            registry: Registry::default(),
            embedded: None,
        }
    }

    /// Views, translations and public files compiled into the binary:
    /// `.embed(renox::embedded!())`. They're used when `APP_DEBUG` is off;
    /// while debugging, files are read from disk so edits show up at once.
    pub fn embed(mut self, embedded: crate::Embedded) -> Self {
        self.embedded = Some(embedded);
        self
    }

    /// Uses the given configuration instead of loading it from the environment.
    pub fn with_config(config: Config) -> Self {
        Self {
            config: Some(config),
            ..Self::new()
        }
    }

    /// Replaces the configuration, e.g. in tests.
    pub fn config(mut self, config: Config) -> Self {
        self.config = Some(config);
        self
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

    /// Lets queue workers run jobs of type `J`.
    pub fn job<J: Job>(mut self) -> Self {
        self.registry.job::<J>();
        self
    }

    /// Runs `listener` whenever an `E` is emitted with `state.emit(..)`.
    pub fn listen<E, F, Fut>(mut self, listener: F) -> Self
    where
        E: Event,
        F: Fn(E, AppState) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result> + Send + 'static,
    {
        self.registry.listen(listener);
        self
    }

    /// Defines scheduled tasks.
    pub fn schedule(mut self, define: impl FnOnce(&mut Schedule)) -> Self {
        define(self.registry.schedule());
        self
    }

    /// Connects to the database and builds the router, without serving.
    pub async fn boot(mut self) -> Result<Kernel> {
        let config = match self.config {
            Some(config) => config,
            None => Config::load()?,
        };

        self.registry.job::<crate::mail::SendMail>();
        let mut migrations = vec![crate::queue::MIGRATION, crate::cache::MIGRATION];
        migrations.extend(self.migrations);
        for module in &self.modules {
            migrations.extend_from_slice(module.migrations());
            module.register(&mut self.registry);
        }
        let Registry {
            jobs,
            listeners,
            schedule,
            duplicate_job,
        } = self.registry;
        if let Some(name) = duplicate_job {
            return Err(anyhow!("job `{name}` is registered twice").into());
        }
        schedule.check()?;
        let offset = parse_offset(&config.timezone)?;

        let migrator = Migrator::new(migrations)?;
        let db = crate::db::connect(&config).await?;
        let key = match &config.key {
            Some(key) => parse_key(key)?,
            None => parse_key(&crate::generate_key())?,
        };

        let mut router = Router::new();
        let mut routes = RouteTable::default();
        let mut listing = Vec::new();
        for module in &self.modules {
            tracing::debug!(module = module.name(), "registering module");
            let (module_router, names, infos) = module.routes().into_parts();
            router = router.merge(module_router);
            for (name, path) in names {
                routes.insert(name, path)?;
            }
            listing.extend(infos.into_iter().map(|info| RouteInfo {
                module: module.name().to_owned(),
                ..info
            }));
        }
        listing.extend(framework_routes(&config));
        listing.sort_by(|a, b| (&a.path, &a.method).cmp(&(&b.path, &b.method)));
        let routes = Arc::new(routes);

        let storage = crate::storage::Storage::from_config(&config)?;
        // Release builds serve what was compiled in; debug builds read the disk.
        let embedded = self.embedded.filter(|_| !config.debug);
        let views = Views::new(
            &config,
            routes.clone(),
            storage.clone(),
            embedded.map(|e| e.views),
        );
        let state = AppState {
            mailer: Mailer::from_config(&config)?,
            queue: Queue::new(db.clone()),
            cache: crate::cache::Cache::new(&config.cache_store, db.clone())?,
            storage,
            translator: Arc::new(match embedded {
                Some(files) => crate::i18n::Translator::embedded(files.lang)?,
                None => crate::i18n::Translator::load(&config.lang_path, config.debug)?,
            }),
            live: (config.debug && config.env == Environment::Local).then(|| {
                crate::live::Live::start(vec![
                    config.views_path.clone(),
                    config.public_path.clone(),
                    config.lang_path.clone(),
                ])
            }),
            listeners: Arc::new(listeners),
            config: Arc::new(config),
            routes,
            views,
            db,
            key,
            gates: Arc::new(self.gates),
            // Five failed logins per email and IP per minute.
            throttle: Arc::new(Throttle::new(5, Duration::from_secs(60))),
        };

        Ok(Kernel {
            listing,
            router: build_router(router, state.clone(), embedded.map(|e| e.public)),
            state,
            migrator,
            seeders: self.seeders,
            handlers: Arc::new(jobs),
            schedule,
            offset,
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
        let long_running = matches!(command, "serve" | "queue:work" | "schedule:work");
        init_tracing(&config, long_running);
        if command == "serve" && config.key.is_none() && config.env != Environment::Testing {
            tracing::warn!(
                "APP_KEY is not set; using a temporary key, so sessions end on restart. \
                 Run `rnx key:generate`."
            );
        }
        let kernel = App {
            config: Some(config),
            ..self
        }
        .boot()
        .await?;

        match command {
            "serve" => kernel.serve().await?,
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
            "queue:work" => {
                let queues: Vec<String> = flag_text(args, "--queue")
                    .map(|q| q.split(',').map(|s| s.trim().to_owned()).collect())
                    .unwrap_or_default();
                let worker = kernel.worker(queues);
                if args.iter().any(|a| a == "--once") {
                    println!("Ran {} job(s).", worker.drain().await?);
                } else {
                    let workers = flag_value(args, "--workers")?.unwrap_or(1) as usize;
                    let (stop, stopped) = watch::channel(false);
                    let running = tokio::spawn(worker.run(workers, stopped));
                    shutdown_signal().await;
                    let _ = stop.send(true);
                    let _ = running.await;
                }
            }
            "queue:failed" => {
                let failed = kernel.state.queue.failed().await?;
                if failed.is_empty() {
                    println!("No failed jobs.");
                }
                for job in failed {
                    println!("  #{} {} ({}): {}", job.id, job.job, job.queue, job.error);
                }
            }
            "queue:retry" => {
                let id = match args.get(1).map(String::as_str) {
                    Some("all") => None,
                    Some(id) => Some(
                        id.parse()
                            .map_err(|_| anyhow!("expected a job id or `all`"))?,
                    ),
                    None => return Err(anyhow!("usage: queue:retry <id|all>").into()),
                };
                println!(
                    "Queued {} job(s) again.",
                    kernel.state.queue.retry(id).await?
                );
            }
            "queue:flush" => println!(
                "Deleted {} failed job(s).",
                kernel.state.queue.flush_failed().await?
            ),
            "schedule:list" => {
                if kernel.schedule.is_empty() {
                    println!("No scheduled tasks.");
                }
                for (name, at) in kernel.schedule.upcoming(kernel.offset) {
                    let at = chrono::DateTime::from_timestamp(at + kernel.offset, 0)
                        .map(|t| t.format("%Y-%m-%d %H:%M").to_string())
                        .unwrap_or_default();
                    println!("  {at}  {name}");
                }
            }
            "schedule:work" => {
                let (stop, stopped) = watch::channel(false);
                let running = tokio::spawn(kernel.schedule.clone().run(
                    kernel.state.clone(),
                    kernel.offset,
                    stopped,
                ));
                shutdown_signal().await;
                let _ = stop.send(true);
                let _ = running.await;
            }
            "route:list" => print_routes(kernel.routes()),
            "db:shell" => crate::shell::run(kernel.db()).await?,
            "down" => {
                let secret = flag_text(args, "--secret").map(str::to_owned);
                let retry = flag_value(args, "--retry")?.map(u64::from);
                crate::maintenance::down(&kernel.state.config.storage_path, secret.clone(), retry)?;
                match secret {
                    Some(secret) => println!("The app is down. Visit /{secret} to bypass it."),
                    None => println!("The app is down."),
                }
            }
            "up" => match crate::maintenance::up(&kernel.state.config.storage_path)? {
                true => println!("The app is up."),
                false => println!("The app was not down."),
            },
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

/// A booted application: its router, database, queue and maintenance commands.
pub struct Kernel {
    router: Router,
    listing: Vec<RouteInfo>,
    state: AppState,
    migrator: Migrator,
    seeders: Vec<Seeder>,
    handlers: Handlers,
    schedule: Schedule,
    offset: i64,
}

impl Kernel {
    pub fn router(&self) -> Router {
        self.router.clone()
    }

    /// Every route, sorted by path, as `route:list` prints them.
    pub fn routes(&self) -> &[RouteInfo] {
        &self.listing
    }

    /// What handlers get as `State<AppState>`.
    pub fn state(&self) -> &AppState {
        &self.state
    }

    pub fn db(&self) -> &Db {
        &self.state.db
    }

    /// The mailer; with `MAIL_MAILER=memory`, `mailer().sent()` lists what was sent.
    pub fn mailer(&self) -> &Mailer {
        &self.state.mailer
    }

    /// A worker for the given queues (all queues when empty).
    pub fn worker(&self, queues: Vec<String>) -> Worker {
        Worker::new(self.state.clone(), self.handlers.clone(), queues)
    }

    /// Runs every job that is available now, e.g. in tests; returns how many ran.
    pub async fn run_jobs(&self) -> Result<usize> {
        self.worker(Vec::new()).drain().await
    }

    /// Serves HTTP with queue workers and the scheduler in the same process,
    /// until Ctrl-C or SIGTERM; then lets running jobs finish.
    pub async fn serve(self) -> Result {
        let config = self.state.config.clone();
        let listener = TcpListener::bind(config.addr()).await?;
        tracing::info!(
            "{} listening on http://{}",
            config.name,
            listener.local_addr()?
        );

        let (stop, stopped) = watch::channel(false);
        let mut background = Vec::new();
        if config.queue_workers > 0 {
            tracing::info!(workers = config.queue_workers, "queue workers started");
            let worker = self.worker(Vec::new());
            background.push(tokio::spawn(
                worker.run(config.queue_workers, stopped.clone()),
            ));
        }
        if config.scheduler && !self.schedule.is_empty() {
            tracing::info!("scheduler started");
            background.push(tokio::spawn(self.schedule.clone().run(
                self.state.clone(),
                self.offset,
                stopped,
            )));
        }

        let service = self
            .router
            .into_make_service_with_connect_info::<SocketAddr>();
        let live = self.state.live.clone();
        axum::serve(listener, service)
            .with_graceful_shutdown(async move {
                shutdown_signal().await;
                // Open live-reload streams would otherwise hold the shutdown.
                if let Some(live) = live {
                    live.stop();
                }
            })
            .await?;

        let _ = stop.send(true);
        let finished = async {
            for task in background {
                let _ = task.await;
            }
        };
        if tokio::time::timeout(SHUTDOWN_GRACE, finished)
            .await
            .is_err()
        {
            tracing::warn!("background work was still running after {SHUTDOWN_GRACE:?}");
        }
        tracing::info!("{} stopped", config.name);
        Ok(())
    }

    /// Runs pending migrations; returns their names.
    pub async fn migrate(&self) -> Result<Vec<String>> {
        let done = self.migrator.run(self.db()).await?;
        Ok(done.into_iter().map(str::to_owned).collect())
    }

    /// Undoes the last `batches` batches of migrations; returns their names.
    pub async fn rollback(&self, batches: u32) -> Result<Vec<String>> {
        Ok(self.migrator.rollback(self.db(), batches).await?)
    }

    /// Drops every table and runs all migrations.
    pub async fn fresh(&self) -> Result<Vec<String>> {
        let done = self.migrator.fresh(self.db()).await?;
        Ok(done.into_iter().map(str::to_owned).collect())
    }

    pub async fn migration_status(&self) -> Result<Vec<MigrationStatus>> {
        Ok(self.migrator.status(self.db()).await?)
    }

    /// Runs every seeder in registration order.
    pub async fn seed(&self) -> Result {
        for seeder in &self.seeders {
            seeder(self.db().clone()).await?;
        }
        Ok(())
    }
}

/// Routes Renox adds itself.
fn framework_routes(config: &Config) -> Vec<RouteInfo> {
    let route = |method: &str, path: &str| RouteInfo {
        method: method.to_owned(),
        path: path.to_owned(),
        name: None,
        module: "renox".to_owned(),
        middleware: Vec::new(),
    };
    let mut routes = vec![
        route("GET", "/health"),
        route("GET", "/_renox/{asset}"),
        route("GET", "/_renox/files/{*key}"),
        route("GET", "/storage/{*path}"),
    ];
    if config.debug {
        routes.push(route("GET", "/_renox/mail"));
        routes.push(route("GET", "/_renox/mail/{id}"));
    }
    if config.debug && config.env == Environment::Local {
        routes.push(route("GET", "/_renox/live"));
    }
    routes
}

fn print_routes(routes: &[RouteInfo]) {
    let rows: Vec<[String; 5]> = routes
        .iter()
        .map(|r| {
            [
                r.method.clone(),
                r.path.clone(),
                r.name.clone().unwrap_or_default(),
                r.module.clone(),
                r.middleware.join(", "),
            ]
        })
        .collect();
    let header = ["METHOD", "PATH", "NAME", "MODULE", "MIDDLEWARE"].map(str::to_owned);
    let mut widths = header.clone().map(|h| h.len());
    for row in &rows {
        for (width, cell) in widths.iter_mut().zip(row) {
            *width = (*width).max(cell.chars().count());
        }
    }
    for row in std::iter::once(&header).chain(&rows) {
        let line: Vec<String> = row
            .iter()
            .zip(widths)
            .map(|(cell, width)| format!("{cell:<width$}"))
            .collect();
        println!("{}", line.join("  ").trim_end());
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

fn flag_text<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    let i = args.iter().position(|a| a == flag)?;
    args.get(i + 1).map(String::as_str)
}

fn flag_value(args: &[String], flag: &str) -> Result<Option<u32>> {
    if !args.iter().any(|a| a == flag) {
        return Ok(None);
    }
    match flag_text(args, flag).and_then(|v| v.parse().ok()) {
        Some(value) => Ok(Some(value)),
        None => Err(anyhow!("{flag} needs a number").into()),
    }
}

fn build_router(
    router: Router<AppState>,
    state: AppState,
    embedded_public: Option<&'static [(&'static str, &'static [u8])]>,
) -> Router {
    let not_found = || async { Error::NotFound };
    let router = router.merge(crate::storage::router());
    let router = if state.config.debug {
        router.merge(crate::mail::preview_router())
    } else {
        router
    };
    let public = state.config.public_path.clone();
    let router = if let Some(files) = embedded_public {
        let files = crate::embedded::public_map(files);
        router.fallback(move |uri: axum::http::Uri| {
            let files = files.clone();
            async move { crate::embedded::serve(&files, &uri) }
        })
    } else if public.is_dir() {
        router.fallback_service(ServeDir::new(public).not_found_service(not_found.into_service()))
    } else {
        router.fallback(not_found)
    };

    let router: Router = router
        .layer(from_fn_with_state(
            state.clone(),
            crate::maintenance::middleware,
        ))
        .layer(from_fn_with_state(state.clone(), view::middleware))
        .layer(from_fn(csrf::middleware))
        .layer(from_fn_with_state(state.clone(), auth::middleware))
        .layer(from_fn_with_state(state.clone(), crate::i18n::middleware))
        .layer(from_fn_with_state(state.clone(), session::middleware))
        .merge(assets::router())
        .merge(crate::health::router())
        .merge(crate::live::router())
        .merge(public_files(&state))
        .layer(axum::extract::DefaultBodyLimit::max(
            state.config.upload_max_size,
        ))
        .layer(TraceLayer::new_for_http())
        .with_state(state.clone());
    // Method spoofing must change the method before the router matches it.
    let limit = state.config.upload_max_size;
    let spoofing = from_fn(
        move |req: axum::extract::Request, next: axum::middleware::Next| {
            crate::method::middleware(req, next, limit)
        },
    );
    Router::new().fallback_service(tower::Layer::layer(&spoofing, router))
}

/// Public files of the local disk at `/storage/...`, outside sessions.
fn public_files(state: &AppState) -> Router<AppState> {
    match state.storage.public_root() {
        Some(root) => Router::new().nest_service("/storage", ServeDir::new(root)),
        None => Router::new(),
    }
}

/// Long-running commands log at info; others only log warnings and errors.
fn init_tracing(config: &Config, long_running: bool) {
    let default = match (long_running, config.debug) {
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
