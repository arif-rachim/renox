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

use crate::auth::{Gate, LoginThrottle, User};
use crate::crypto::parse_key;
use crate::db::{Db, Migration, MigrationStatus, Migrator};
use crate::events::Event;
use crate::mail::Mailer;
use crate::queue::{Handlers, Job, Queue, Worker};
use crate::routing::RouteInfo;
use crate::schedule::Schedule;
use crate::timezone::Zone;
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
  queue:forget <id>         Delete one failed job
  queue:prune-failed [--hours N]
                            Delete failed jobs older than N hours (default 168)
  queue:prune-batches [--hours N]
                            Delete batches finished more than N hours ago (default 24)
  webhook:failed            List webhook calls whose processing failed
  webhook:retry <id>        Process a stored webhook call again
  ui:publish [--force]      Copy the UI kit (renox/ui.html and its CSS) into the app to change it
  cache:prune               Delete expired rows of the database cache store
  session:prune             Delete expired sessions (SESSION_DRIVER=database)
  schedule:list             List scheduled tasks and when they run next
  schedule:run <task>       Run one scheduled task now
  schedule:work             Run scheduled tasks (when SCHEDULER=false for serve)
  route:list                List every route with its name, module and guards
  db:shell                  Run SQL against the database (`.tables`, `.quit`)
  down [--secret S] [--retry N]
                            Maintenance mode: answer 503 (visit /S to bypass it)
  up                        Leave maintenance mode
  help                      Show this message";

/// The application builder.
///
/// ```no_run
/// # use renox::prelude::*;
/// # use serde::{Deserialize, Serialize};
/// # struct Produk;
/// # impl Module for Produk { fn name(&self) -> &'static str { "produk" } }
/// # #[derive(Serialize, Deserialize)] struct SendReceipt;
/// # impl Job for SendReceipt { const NAME: &'static str = "send-receipt"; async fn handle(self, _: JobContext) -> Result { Ok(()) } }
/// # async fn cleanup(_: AppState) -> Result { Ok(()) }
/// # async fn seed(_: Db) -> Result { Ok(()) }
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
    async_gates: HashMap<String, crate::auth::AsyncGate>,
    gate_before: Option<crate::auth::GateBefore>,
    registry: Registry,
    embedded: Option<crate::Embedded>,
    csp: crate::security::Csp,
    provided: HashMap<std::any::TypeId, Arc<dyn std::any::Any + Send + Sync>>,
    layers: Vec<AppLayer>,
    limiters: HashMap<String, crate::rate_limit::LimitRule>,
    detect_locale: bool,
}

/// A layer from `App::layer`, applied to the app's routes at boot.
/// Applied to the app's router and to each `Routes::domain` router.
type AppLayer = Box<dyn Fn(Router<AppState>) -> Router<AppState> + Send + Sync>;

impl App {
    /// Creates an application that loads its configuration from `.env` on start.
    pub fn new() -> Self {
        Self {
            config: None,
            modules: Vec::new(),
            migrations: Vec::new(),
            seeders: Vec::new(),
            gates: HashMap::new(),
            async_gates: HashMap::new(),
            gate_before: None,
            registry: Registry::default(),
            embedded: None,
            csp: crate::security::Csp::default(),
            provided: HashMap::new(),
            layers: Vec::new(),
            limiters: HashMap::new(),
            detect_locale: false,
        }
    }

    /// Picks a visitor's language from their browser (`Accept-Language`)
    /// when they haven't chosen one: the first of their languages this app
    /// has texts for (the built-in `en` and `id`, or a
    /// `resources/lang/<locale>.json`), else `APP_LOCALE`. A language set
    /// with `i18n::set_locale` still wins. Responses then carry
    /// `Vary: Accept-Language`, so caches keep the languages apart.
    ///
    /// ```
    /// # use renox::prelude::*;
    /// # let _ =
    /// App::new().detect_locale()
    /// # ;
    /// ```
    pub fn detect_locale(mut self) -> Self {
        self.detect_locale = true;
        self
    }

    /// Wraps every route of the app's modules in a tower layer, e.g. a
    /// middleware function (framework routes such as `/health` and
    /// `public/` files aren't wrapped). It runs after Renox has loaded the
    /// session and the user, so it can use `AuthUser` or `Session`:
    ///
    /// ```
    /// # use renox::prelude::*;
    /// use renox::axum::extract::Request;
    /// use renox::axum::middleware::{Next, from_fn};
    ///
    /// async fn stamp(user: Option<AuthUser>, req: Request, next: Next) -> Response {
    ///     let mut res = next.run(req).await;
    ///     let who = if user.is_some() { "member" } else { "guest" };
    ///     res.headers_mut().insert("x-visitor", who.parse().unwrap());
    ///     res
    /// }
    ///
    /// # let _ =
    /// App::new().layer(from_fn(stamp))
    /// # ;
    /// ```
    ///
    /// Layers run in the order added: the first one sees the request first.
    pub fn layer<L>(mut self, layer: L) -> Self
    where
        L: tower::Layer<axum::routing::Route> + Clone + Send + Sync + 'static,
        L::Service: tower::Service<axum::extract::Request> + Clone + Send + Sync + 'static,
        <L::Service as tower::Service<axum::extract::Request>>::Response:
            axum::response::IntoResponse + 'static,
        <L::Service as tower::Service<axum::extract::Request>>::Error:
            Into<std::convert::Infallible> + 'static,
        <L::Service as tower::Service<axum::extract::Request>>::Future: Send + 'static,
    {
        self.layers
            .push(Box::new(move |router| router.layer(layer.clone())));
        self
    }

    /// Allows other sites in the Content-Security-Policy, e.g.
    /// `.csp(|csp| { csp.allow("script-src", "https://www.googletagmanager.com"); })`.
    pub fn csp(mut self, allow: impl FnOnce(&mut crate::security::Csp)) -> Self {
        allow(&mut self.csp);
        self
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

    /// Adds a module: its routes, migrations and registrations.
    pub fn module(mut self, module: impl Module) -> Self {
        self.modules.push(Box::new(module));
        self
    }

    /// Registers app-level migrations, usually `renox::migrations!()`.
    pub fn migrations(mut self, migrations: &[Migration]) -> Self {
        self.migrations.extend_from_slice(migrations);
        self
    }

    /// Registers a seeder for `db:seed`. Seeders run in registration order,
    /// in the app's context: `renox::context::app()` gives the `AppState`.
    ///
    /// ```
    /// # use renox::prelude::*;
    /// # #[derive(Model, serde::Serialize, Default)] struct Produk { id: i64 }
    /// # impl Factory for Produk { fn definition() -> Self { Produk::default() } }
    /// # let _ =
    /// App::new().seeder(|db| async move {
    ///     Produk::create_many(&db, 50).await?;
    ///     Ok(())
    /// })
    /// # ;
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
    /// ```
    /// # use renox::prelude::*;
    /// # let _ =
    /// App::new().gate("admin", |user| user.email.ends_with("@toko.id"))
    /// // in a handler: auth.gate("admin")?;   in a template: {% if can('admin') %}
    /// # ;
    /// ```
    pub fn gate(
        mut self,
        name: &str,
        check: impl Fn(&User) -> bool + Send + Sync + 'static,
    ) -> Self {
        self.gates.insert(name.to_owned(), Arc::new(check));
        self
    }

    /// A gate that may query the database, e.g. whether the user belongs to
    /// a team. Check it in handlers with `auth.gate_async(name).await?`;
    /// templates can't wait for it (`can()` denies it), so pass its answer in
    /// the view's context.
    ///
    /// ```
    /// # use renox::prelude::*;
    /// # let _ =
    /// App::new().gate_async("billing", |user, state| async move {
    ///     let n: i64 = renox::db::sql("SELECT COUNT(*) FROM team_admins WHERE user_id = ?")
    ///         .bind(user.id)
    ///         .scalar(&state.db)
    ///         .await?;
    ///     Ok(n > 0)
    /// })
    /// // in a handler: auth.gate_async("billing").await?;
    /// # ;
    /// ```
    /// Asked before every gate, permission and policy check: `Some(true)`
    /// allows, `Some(false)` denies, `None` goes on to the check itself.
    /// Typically lets super-admins do everything.
    ///
    /// ```
    /// # use renox::prelude::*;
    /// # let _ =
    /// App::new().gate_before(|user, _ability| {
    ///     (user.get::<String>("role").as_deref() == Some("owner")).then_some(true)
    /// })
    /// # ;
    /// ```
    pub fn gate_before(
        mut self,
        check: impl Fn(&User, &str) -> Option<bool> + Send + Sync + 'static,
    ) -> Self {
        self.gate_before = Some(Arc::new(check));
        self
    }

    /// Defines the gate `name` with a check that can await, e.g. to query
    /// the database. `can(name)` and `require_gate` ask it like any gate.
    pub fn gate_async<F, Fut>(mut self, name: &str, check: F) -> Self
    where
        F: Fn(User, AppState) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<bool>> + Send + 'static,
    {
        self.async_gates.insert(
            name.to_owned(),
            Arc::new(move |user, state| Box::pin(check(user, state))),
        );
        self
    }

    /// Receives `W`'s webhooks: see `renox::webhook`. Also add the route
    /// with `Routes::webhook::<W>(path)`.
    pub fn webhook<W: crate::webhook::Webhook>(mut self) -> Self {
        self.registry.webhook::<W>();
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

    /// Adds a command the app binary runs: `my-app <name> [args]`, e.g. to
    /// create the first admin or run an import. See [`crate::command`].
    pub fn command<F, Fut>(mut self, name: &str, about: &str, run: F) -> Self
    where
        F: Fn(AppState, crate::command::Args) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result> + Send + 'static,
    {
        self.registry.command(name, about, run);
        self
    }

    /// A command whose arguments are declared with clap; see
    /// [`AppCommand`](crate::command::AppCommand).
    pub fn typed_command<T: crate::command::AppCommand>(mut self) -> Self {
        self.registry.typed_command::<T>();
        self
    }

    /// A named rate limit for `Routes::throttle_by(name)`, whose `rule`
    /// picks the limit for each request (by user, role, IP, API key…); see
    /// [`crate::rate_limit::Limit`].
    pub fn rate_limiter(
        mut self,
        name: &str,
        rule: impl Fn(&crate::rate_limit::LimitRequest) -> crate::rate_limit::Limit
        + Send
        + Sync
        + 'static,
    ) -> Self {
        self.limiters
            .insert(name.to_owned(), std::sync::Arc::new(rule));
        self
    }

    /// Sends every error that needs a person (a 500, a job that failed for
    /// good, a failed scheduled task) to `reporter`, e.g. an error tracker;
    /// see [`crate::report`].
    pub fn report<F, Fut>(mut self, reporter: F) -> Self
    where
        F: Fn(crate::report::ErrorReport, AppState) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        self.registry.report(reporter);
        self
    }

    /// Adds a notification channel (WhatsApp, SMS, Slack…); see
    /// [`Registry::channel`].
    pub fn channel<F, Fut>(mut self, name: &str, send: F) -> Self
    where
        F: Fn(AppState, crate::auth::Recipient, serde_json::Value) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result> + Send + 'static,
    {
        self.registry.channel(name, send);
        self
    }

    /// Makes `value` available everywhere the app runs: `Provided<T>` in
    /// handlers, `state.provided::<T>()` in jobs, listeners, commands and
    /// scheduled tasks. One value per type; a second one replaces the first.
    /// See [`crate::Provided`].
    pub fn provide<T: Send + Sync + 'static>(mut self, value: T) -> Self {
        self.provided
            .insert(std::any::TypeId::of::<T>(), Arc::new(value));
        self
    }

    /// Gives every view a value computed per request; see [`Registry::share`].
    pub fn share<F, Fut, T>(mut self, key: &str, compute: F) -> Self
    where
        F: Fn(crate::view::ViewContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T>> + Send + 'static,
        T: serde::Serialize,
    {
        self.registry.share(key, compute);
        self
    }

    /// Adds template functions, filters or globals; see [`Registry::templates`].
    pub fn templates(
        mut self,
        hook: impl Fn(&mut minijinja::Environment<'static>) + Send + Sync + 'static,
    ) -> Self {
        self.registry.templates(hook);
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
        self.registry
            .job::<crate::auth::notifications::SendToChannel>();
        self.registry.job::<crate::webhook::ProcessWebhook>();
        #[cfg(feature = "server-events")]
        self.registry.job::<crate::analytics::ServerEvent>();
        let mut migrations = crate::queue::MIGRATIONS.to_vec();
        migrations.push(crate::cache::MIGRATION);
        migrations.push(crate::session::MIGRATION);
        migrations.push(crate::grid::MIGRATION);
        migrations.extend(crate::webhook::MIGRATIONS);
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
            webhooks,
            commands,
            templates,
            shares,
            channels,
            reporters,
            permissions,
        } = self.registry;
        if let Some(name) = duplicate_job {
            return Err(anyhow!("job `{name}` is registered twice").into());
        }
        check_commands(&commands)?;
        schedule.check()?;
        let zone: Zone = config
            .timezone
            .parse()
            .map_err(|err| anyhow!("APP_TIMEZONE: {err}"))?;

        let migrator = Migrator::new(migrations)?;
        let key = match &config.key {
            Some(key) => parse_key(key)?,
            None => parse_key(&crate::generate_key())?,
        };
        // `Encrypted` model fields are sealed and opened with APP_KEY.
        let db = crate::db::connect(&config).await?.with_key(key.clone());

        let mut router = Router::new();
        let mut fallback: Option<axum::routing::MethodRouter<AppState>> = None;
        // One router per `Routes::domain` pattern, with its own fallback.
        let mut domains: Vec<(
            crate::domain::DomainPattern,
            Router<AppState>,
            Option<axum::routing::MethodRouter<AppState>>,
        )> = Vec::new();
        let mut routes = RouteTable::default();
        let mut listing: Vec<RouteInfo> = Vec::new();
        for module in &self.modules {
            tracing::debug!(module = module.name(), "registering module");
            let parts = module.routes().into_parts();
            check_clashes(&listing, &parts.listing, None, module.name())?;
            router = merge_routes(router, parts.router, module.name())?;
            for (name, path) in parts.names {
                routes.insert(name, path)?;
            }
            listing.extend(parts.listing.into_iter().map(|info| RouteInfo {
                module: module.name().to_owned(),
                ..info
            }));
            if let Some(handler) = parts.fallback {
                if fallback.is_some() {
                    return Err(anyhow!(
                        "two modules set a fallback route (`{}` is the second)",
                        module.name()
                    )
                    .into());
                }
                fallback = Some(handler);
            }
            for (text, domain_routes) in parts.domains {
                let pattern = crate::domain::DomainPattern::parse(&text)?;
                let inner = domain_routes.into_parts();
                if !inner.domains.is_empty() {
                    return Err(anyhow!(
                        "Routes::domain(\"{text}\", …) inside another domain (module `{}`)",
                        module.name()
                    )
                    .into());
                }
                check_clashes(
                    &listing,
                    &inner.listing,
                    Some(pattern.as_str()),
                    module.name(),
                )?;
                let at = match domains.iter().position(|(p, _, _)| *p == pattern) {
                    Some(at) => at,
                    None => {
                        domains.push((pattern.clone(), Router::new(), None));
                        domains.len() - 1
                    }
                };
                let (_, domain_router, domain_fallback) = &mut domains[at];
                *domain_router =
                    merge_routes(std::mem::take(domain_router), inner.router, module.name())?;
                if let Some(handler) = inner.fallback {
                    if domain_fallback.is_some() {
                        return Err(anyhow!(
                            "two fallbacks for the domain `{text}` (module `{}`)",
                            module.name()
                        )
                        .into());
                    }
                    *domain_fallback = Some(handler);
                }
                for (name, path) in inner.names {
                    routes.set_domain(&name, pattern.as_str());
                    routes.insert(name, path)?;
                }
                listing.extend(inner.listing.into_iter().map(|info| RouteInfo {
                    module: module.name().to_owned(),
                    domain: Some(pattern.as_str().to_owned()),
                    ..info
                }));
            }
        }
        // The app's own layers; the first one added ends up outermost.
        for layer in self.layers.iter().rev() {
            router = layer(router);
            for (_, domain_router, _) in &mut domains {
                *domain_router = layer(std::mem::take(domain_router));
            }
        }
        listing.extend(framework_routes(&config));
        listing.sort_by(|a, b| (&a.path, &a.method).cmp(&(&b.path, &b.method)));
        // `throttle_by("name")` needs `App::rate_limiter("name", …)`.
        for route in &listing {
            for mark in &route.middleware {
                if let Some(name) = mark.strip_prefix("throttle:")
                    && !name.contains('/')
                    && !self.limiters.contains_key(name)
                {
                    return Err(anyhow!(
                        "{} {} uses throttle_by(\"{name}\"), but there's no App::rate_limiter(\"{name}\", …)",
                        route.method,
                        route.path
                    )
                    .into());
                }
            }
        }
        for route in &listing {
            for provider in route
                .middleware
                .iter()
                .filter_map(|m| m.strip_prefix("webhook:"))
            {
                if !webhooks.contains_key(provider) {
                    return Err(anyhow!(
                        "the route {} receives `{provider}` webhooks, but they're not registered: \
                         add `app.webhook::<…>()` in the module's `register`",
                        route.path
                    )
                    .into());
                }
            }
        }
        let routes = Arc::new(routes);

        let storage = crate::storage::Storage::from_config(&config)?;
        // Release builds serve what was compiled in; debug builds read the disk.
        let embedded = self.embedded.filter(|_| !config.debug);
        // Rate limits and login locks shared by every server (CACHE_STORE=database).
        let shared_counters = (config.cache_store == "database").then(|| db.clone());
        let versions = Arc::new(crate::embedded::AssetVersions::new(
            &config.public_path,
            embedded.map(|e| e.public),
        ));
        let views = Views::new(
            &config,
            routes.clone(),
            storage.clone(),
            embedded.map(|e| e.views),
            Arc::new(templates),
            zone,
            versions,
        );
        let security = Arc::new(crate::security::Security::new(&config, &self.csp, &listing));
        let state = AppState {
            security,
            webhooks: Arc::new(webhooks),
            mailer: Mailer::from_config(&config)?,
            queue: Queue::new(db.clone(), key.clone()),
            cache: crate::cache::Cache::new(&config.cache_store, db.clone())?,
            storage,
            http: crate::http::Http::default(),
            fakes: Arc::default(),
            translator: Arc::new(match embedded {
                Some(files) => crate::i18n::Translator::embedded(files.lang)?,
                None => crate::i18n::Translator::load(&config.lang_path, config.debug)?,
            }),
            // Tests read database sessions synchronously (`TestApp::session_get`).
            session_mirror: (config.session_driver == "database"
                && config.env == Environment::Testing)
                .then(Default::default),
            live: (config.debug && config.env == Environment::Local).then(|| {
                crate::live::Live::start(vec![
                    config.views_path.clone(),
                    config.public_path.clone(),
                    config.lang_path.clone(),
                ])
            }),
            notification_hub: Arc::new(crate::auth::notifications::Hub::new()),
            listeners: Arc::new(listeners),
            inspector: (config.debug && config.env == Environment::Local)
                .then(|| Arc::new(crate::inspector::Inspector::default())),
            config: Arc::new(config),
            routes,
            views,
            db,
            key,
            gates: Arc::new(crate::auth::Access {
                gates: self.gates,
                before: self.gate_before,
                permissions,
            }),
            async_gates: Arc::new(self.async_gates),
            shares: Arc::new(shares),
            channels: Arc::new(channels),
            reporters: Arc::new(reporters),
            limiters: Arc::new(
                self.limiters
                    .into_iter()
                    .map(|(name, rule)| (name, crate::rate_limit::NamedLimiter::new(rule)))
                    .collect(),
            ),
            provided: Arc::new(self.provided),
            throttle: Arc::new(LoginThrottle::new(shared_counters.clone())),
            detect_locale: self.detect_locale,
        };

        let public = embedded.map(|e| e.public);
        let default = build_router(router, state.clone(), public, fallback);
        let router = if domains.is_empty() {
            default
        } else {
            let hosts = domains
                .into_iter()
                .map(|(pattern, router, fallback)| {
                    (
                        pattern,
                        build_router(router, state.clone(), public, fallback),
                    )
                })
                .collect();
            crate::domain::dispatch(hosts, default)
        };
        Ok(Kernel {
            listing,
            router,
            state,
            migrator,
            seeders: self.seeders,
            handlers: Arc::new(jobs),
            schedule,
            zone,
            commands,
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
            .block_on(self.run_command(&args))
    }

    /// Runs one command as the app binary would, e.g. from a test or a
    /// program that drives the app: `["migrate:status"]`, `["down",
    /// "--secret", "abc"]`, or `[]` for `serve`. Output goes to stdout.
    ///
    /// ```no_run
    /// # use renox::prelude::*;
    /// # async fn demo() -> Result {
    /// App::new().run_args(["migrate"]).await?;
    /// # Ok(()) }
    /// ```
    pub async fn run_args(self, args: impl IntoIterator<Item = impl Into<String>>) -> Result {
        let args: Vec<String> = args.into_iter().map(Into::into).collect();
        self.run_command(&args).await
    }

    /// Starts the server on the current Tokio runtime.
    pub async fn serve(self) -> Result {
        self.run_command(&[]).await
    }

    async fn run_command(self, args: &[String]) -> Result {
        let command = args.first().map(String::as_str).unwrap_or("serve");
        if matches!(command, "help" | "--help" | "-h") {
            // Modules add their commands in `register`, which boot runs.
            let mut modules = Registry::default();
            for module in &self.modules {
                module.register(&mut modules);
            }
            let mut commands = self.registry.commands.clone();
            commands.extend(modules.commands);
            println!("{USAGE}{}", app_commands_help(&commands));
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
                    let note = if m.missing {
                        "  (applied, but its file is gone)"
                    } else if m.changed {
                        "  (edited after it ran; the edit won't run)"
                    } else {
                        ""
                    };
                    match m.batch {
                        Some(batch) => println!("  ran (batch {batch})  {}{note}", m.name),
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
            "webhook:failed" => {
                let failed = crate::webhook::WebhookCall::failed(&kernel.state.db).await?;
                if failed.is_empty() {
                    println!("No failed webhook calls.");
                }
                for call in failed {
                    println!(
                        "  #{} {} {}: {}",
                        call.id,
                        call.provider,
                        call.event_id,
                        call.error
                            .unwrap_or_default()
                            .lines()
                            .next()
                            .unwrap_or_default()
                    );
                }
            }
            "webhook:retry" => {
                let id: i64 = args
                    .get(1)
                    .and_then(|id| id.parse().ok())
                    .ok_or_else(|| anyhow!("usage: webhook:retry <id>"))?;
                if crate::webhook::retry(&kernel.state, id).await? {
                    println!("Webhook call #{id} queued again.");
                } else {
                    return Err(anyhow!("there is no webhook call #{id}").into());
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
            "queue:forget" => {
                let id: i64 = args
                    .get(1)
                    .and_then(|id| id.parse().ok())
                    .ok_or_else(|| anyhow!("usage: queue:forget <id>"))?;
                if kernel.state.queue.forget_failed(id).await? {
                    println!("Deleted failed job {id}.");
                } else {
                    return Err(anyhow!("no failed job {id}").into());
                }
            }
            "queue:prune-failed" => {
                let hours = flag_value(args, "--hours")?.unwrap_or(168);
                let age = Duration::from_secs(u64::from(hours) * 3600);
                println!(
                    "Deleted {} failed job(s) older than {hours} hour(s).",
                    kernel.state.queue.prune_failed(age).await?
                );
            }
            "queue:prune-batches" => {
                let hours = flag_value(args, "--hours")?.unwrap_or(24);
                let age = Duration::from_secs(u64::from(hours) * 3600);
                println!(
                    "Deleted {} batch(es) finished more than {hours} hour(s) ago.",
                    kernel.state.queue.prune_batches(age).await?
                );
            }
            "queue:flush" => println!(
                "Deleted {} failed job(s).",
                kernel.state.queue.flush_failed().await?
            ),
            "ui:publish" => {
                let force = args.iter().any(|a| a == "--force");
                crate::assets::publish_ui(&kernel.state.config, force)?;
            }
            "session:prune" => println!(
                "Deleted {} expired session(s).",
                crate::Session::prune_expired(&kernel.state.db).await?
            ),
            "cache:prune" => println!(
                "Deleted {} expired cache row(s).",
                kernel.state.cache.prune().await?
            ),
            "schedule:list" => {
                if kernel.schedule.is_empty() {
                    println!("No scheduled tasks.");
                }
                for (name, at, zone) in kernel.schedule.upcoming(kernel.zone) {
                    let when = if at == i64::MAX {
                        "never".to_owned()
                    } else {
                        zone.local(at).format("%Y-%m-%d %H:%M").to_string()
                    };
                    println!("  {when:<16}  {zone:<18}  {name}");
                }
            }
            "schedule:run" => {
                let Some(name) = args.get(1).filter(|a| !a.starts_with('-')) else {
                    return Err(anyhow!("usage: schedule:run <task>").into());
                };
                kernel.run_scheduled(name).await?;
                println!("Ran `{name}`.");
            }
            "schedule:work" => {
                let (stop, stopped) = watch::channel(false);
                let running = tokio::spawn(kernel.schedule.clone().run(
                    kernel.state.clone(),
                    kernel.zone,
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
            other if kernel.commands.iter().any(|c| c.name == other) => {
                kernel.call(other, args[1..].iter().cloned()).await?;
            }
            other => {
                return Err(anyhow!(
                    "unknown command `{other}`\n\n{USAGE}{}",
                    app_commands_help(&kernel.commands)
                )
                .into());
            }
        }
        Ok(())
    }
}

/// The listening socket systemd passes with socket activation
/// (`LISTEN_FDS`, a `.socket` unit): it stays open while the service
/// restarts, so connections wait in its queue instead of being refused.
fn inherited_listener() -> Result<Option<TcpListener>> {
    let mut fds = listenfd::ListenFd::from_env();
    let Some(listener) = fds
        .take_tcp_listener(0)
        .map_err(|err| anyhow!("the socket systemd passed isn't a TCP listener: {err}"))?
    else {
        return Ok(None);
    };
    listener
        .set_nonblocking(true)
        .map_err(anyhow::Error::from)?;
    Ok(Some(
        TcpListener::from_std(listener).map_err(anyhow::Error::from)?,
    ))
}

/// Built-in commands; an app command can't take one of these names.
const BUILT_IN_COMMANDS: &[&str] = &[
    "serve",
    "migrate",
    "migrate:rollback",
    "migrate:fresh",
    "migrate:status",
    "db:seed",
    "queue:work",
    "queue:failed",
    "queue:retry",
    "queue:flush",
    "queue:forget",
    "queue:prune-failed",
    "queue:prune-batches",
    "webhook:failed",
    "webhook:retry",
    "cache:prune",
    "session:prune",
    "ui:publish",
    "schedule:list",
    "schedule:run",
    "schedule:work",
    "route:list",
    "db:shell",
    "down",
    "up",
    "help",
];

fn check_commands(commands: &[crate::command::Command]) -> Result {
    let mut seen = std::collections::HashSet::new();
    for command in commands {
        let name = command.name.as_str();
        if name.is_empty() || name.starts_with('-') || name.contains(char::is_whitespace) {
            return Err(anyhow!("`{name}` is not a valid command name").into());
        }
        if BUILT_IN_COMMANDS.contains(&name) {
            return Err(anyhow!("the command `{name}` is built in; choose another name").into());
        }
        if !seen.insert(name) {
            return Err(anyhow!("the command `{name}` is registered twice").into());
        }
    }
    Ok(())
}

fn app_commands_help(commands: &[crate::command::Command]) -> String {
    if commands.is_empty() {
        return String::new();
    }
    let mut out = String::from("\n\nApp commands:\n");
    for command in commands {
        out.push_str(&format!("  {:<26}{}\n", command.name, command.about));
    }
    out.trim_end().to_owned()
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
    zone: Zone,
    commands: Vec<crate::command::Command>,
}

impl Kernel {
    /// Runs the app command `name` (see [`App::command`]), e.g. from a test.
    pub async fn call(
        &self,
        name: &str,
        args: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result {
        let command = self
            .commands
            .iter()
            .find(|c| c.name == name)
            .ok_or_else(|| anyhow!("unknown command `{name}`"))?;
        let run = (command.run)(self.state.clone(), crate::command::Args::new(args));
        crate::context::scope_app(self.state.clone(), run).await
    }

    /// Runs the scheduled task `name` now, with its hooks (what
    /// `schedule:run` does), e.g. from a test.
    pub async fn run_scheduled(&self, name: &str) -> Result {
        self.schedule.run_now(self.state.clone(), name).await
    }

    /// The assembled router, e.g. to drive with `tower::ServiceExt::oneshot`.
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

    /// The database connection pool.
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
        let listener = match inherited_listener()? {
            Some(listener) => {
                tracing::info!("using the socket systemd passed (socket activation)");
                listener
            }
            None => TcpListener::bind(config.addr()).await?,
        };
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
                self.zone,
                stopped,
            )));
        }

        let service = self
            .router
            .into_make_service_with_connect_info::<SocketAddr>();
        let live = self.state.live.clone();
        let notification_hub = self.state.notification_hub.clone();
        axum::serve(listener, service)
            .with_graceful_shutdown(async move {
                shutdown_signal().await;
                // Open live-reload and notification streams would otherwise
                // hold the shutdown.
                if let Some(live) = live {
                    live.stop();
                }
                notification_hub.stop();
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

    /// Every migration and whether it has run, as `migrate:status` prints them.
    pub async fn migration_status(&self) -> Result<Vec<MigrationStatus>> {
        Ok(self.migrator.status(self.db()).await?)
    }

    /// Runs every seeder in registration order.
    pub async fn seed(&self) -> Result {
        for seeder in &self.seeders {
            // In the app's context, so a seeder can reach `renox::context::app()`
            // (config, `encrypt`, the cache) and model hooks see it too.
            crate::context::scope_app(self.state.clone(), seeder(self.db().clone())).await?;
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
        domain: None,
    };
    let mut routes = vec![
        route("GET", "/health"),
        route("GET", "/robots.txt"),
        route("GET", "/favicon.ico"),
        route("GET", "/_renox/{asset}"),
        route("GET", "/_renox/files/{*key}"),
        route("POST", "/_renox/grid/{grid}/prefs"),
        route("DELETE", "/_renox/grid/{grid}/prefs"),
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
    let with_domains = routes.iter().any(|r| r.domain.is_some());
    let rows: Vec<Vec<String>> = routes
        .iter()
        .map(|r| {
            let mut row = vec![
                r.method.clone(),
                r.path.clone(),
                r.name.clone().unwrap_or_default(),
                r.module.clone(),
                r.middleware.join(", "),
            ];
            if with_domains {
                row.insert(0, r.domain.clone().unwrap_or_default());
            }
            row
        })
        .collect();
    let mut header: Vec<String> = ["METHOD", "PATH", "NAME", "MODULE", "MIDDLEWARE"]
        .map(str::to_owned)
        .to_vec();
    if with_domains {
        header.insert(0, "DOMAIN".into());
    }
    let mut widths: Vec<usize> = header.iter().map(String::len).collect();
    for row in &rows {
        for (width, cell) in widths.iter_mut().zip(row) {
            *width = (*width).max(cell.chars().count());
        }
    }
    for row in std::iter::once(&header).chain(&rows) {
        let line: Vec<String> = row
            .iter()
            .zip(&widths)
            .map(|(cell, width)| format!("{cell:<width$}", width = *width))
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

/// Turns a handler that panics or runs past `REQUEST_TIMEOUT` into a 500
/// with the error page, instead of a dropped connection or a hung client.
async fn guard(
    limit: Option<Duration>,
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    let run = futures_util::FutureExt::catch_unwind(std::panic::AssertUnwindSafe(next.run(req)));
    let outcome = match limit {
        Some(limit) => match tokio::time::timeout(limit, run).await {
            Ok(outcome) => outcome,
            Err(_) => {
                return Error::Internal(anyhow!(
                    "the request took longer than REQUEST_TIMEOUT ({}s)",
                    limit.as_secs()
                ))
                .into_response();
            }
        },
        None => run.await,
    };
    outcome.unwrap_or_else(|panic| {
        let message = crate::error::panic_message(&*panic);
        Error::Internal(anyhow!("the handler panicked: {message}")).into_response()
    })
}

/// Adds a module's routes; what axum refuses to merge is a boot error.
fn merge_routes(
    router: Router<AppState>,
    other: Router<AppState>,
    module: &str,
) -> Result<Router<AppState>> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| router.merge(other))).map_err(
        |panic| {
            let message = crate::error::panic_message(&*panic);
            anyhow!("the routes of the `{module}` module clash: {message}").into()
        },
    )
}

/// A route of `module` that another module defines already (on the same
/// domain, or both without one).
fn check_clashes(
    listing: &[RouteInfo],
    new: &[RouteInfo],
    domain: Option<&str>,
    module: &str,
) -> Result {
    for info in new {
        let clash = listing.iter().find(|other| {
            other.domain.as_deref() == domain
                && other.path == info.path
                && (other.method == info.method || other.method == "*" || info.method == "*")
        });
        if let Some(other) = clash {
            let on = domain.map(|d| format!(" on {d}")).unwrap_or_default();
            return Err(anyhow!(
                "{} {}{on} is defined by both the `{}` and the `{module}` module",
                info.method,
                info.path,
                other.module,
            )
            .into());
        }
    }
    Ok(())
}

fn build_router(
    router: Router<AppState>,
    state: AppState,
    embedded_public: Option<&'static [(&'static str, &'static [u8])]>,
    fallback: Option<axum::routing::MethodRouter<AppState>>,
) -> Router {
    // No route and no public file: the app's fallback (`Routes::fallback`), else a 404.
    let fallback = fallback.map(|handler| handler.with_state(state.clone()));
    let not_found = move |req: axum::extract::Request| {
        let fallback = fallback.clone();
        async move {
            match fallback {
                Some(handler) => match tower::ServiceExt::oneshot(handler, req).await {
                    Ok(res) => res,
                    Err(never) => match never {},
                },
                None => axum::response::IntoResponse::into_response(Error::NotFound),
            }
        }
    };
    let router = router
        .merge(crate::storage::router())
        .merge(crate::grid::router());
    let router = if state.config.debug {
        router
            .merge(crate::mail::preview_router())
            .merge(crate::inspector::router())
    } else {
        router
    };
    let public = state.config.public_path.clone();
    let router = if let Some(files) = embedded_public {
        let files = crate::embedded::public_map(files);
        let not_found = not_found.clone();
        router.fallback(move |req: axum::extract::Request| {
            let (files, not_found) = (files.clone(), not_found.clone());
            async move {
                let res = crate::embedded::serve(&files, req.uri());
                if res.status() == axum::http::StatusCode::NOT_FOUND {
                    not_found(req).await
                } else {
                    res
                }
            }
        })
    } else if public.is_dir() {
        // `fallback`, not `not_found_service`: the latter makes every answer
        // a 404, and an app's fallback may redirect or answer 200.
        let files = ServeDir::new(public).fallback(not_found.clone().into_service());
        router.fallback(move |req: axum::extract::Request| {
            let files = files.clone();
            async move {
                // A name no file system accepts (over 255 bytes) can't be a
                // file here; asking the OS would be a "name too long" 500.
                if req
                    .uri()
                    .path()
                    .split('/')
                    .any(|segment| segment.len() > 255)
                {
                    return axum::response::IntoResponse::into_response(Error::NotFound);
                }
                let versioned = crate::embedded::is_versioned(req.uri());
                match tower::ServiceExt::oneshot(files, req).await {
                    Ok(res) => {
                        let mut res = axum::response::IntoResponse::into_response(res);
                        if versioned && res.status().is_success() {
                            res.headers_mut().insert(
                                axum::http::header::CACHE_CONTROL,
                                axum::http::HeaderValue::from_static(
                                    "public, max-age=31536000, immutable",
                                ),
                            );
                        }
                        res
                    }
                    Err(never) => match never {},
                }
            }
        })
    } else {
        router.fallback(not_found)
    };

    let request_timeout = state.config.request_timeout;
    let router: Router = router
        .layer(from_fn(
            move |req: axum::extract::Request, next: axum::middleware::Next| {
                guard(request_timeout, req, next)
            },
        ))
        .layer(from_fn_with_state(
            state.clone(),
            crate::maintenance::middleware,
        ))
        .layer(from_fn_with_state(state.clone(), view::middleware))
        .layer(from_fn(csrf::middleware))
        .layer(from_fn_with_state(state.clone(), auth::middleware))
        .layer(from_fn_with_state(state.clone(), crate::i18n::middleware))
        .layer(from_fn_with_state(state.clone(), session::middleware))
        // Each request's own `renox::context`, around everything the app runs.
        .layer(from_fn_with_state(
            state.clone(),
            crate::context::middleware,
        ))
        // `/_renox/debug`: around it all, so the session's and user's SQL count.
        .layer(from_fn_with_state(
            state.clone(),
            crate::inspector::middleware,
        ))
        .merge(assets::router())
        .merge(crate::health::router())
        .merge(robots(&state, embedded_public))
        .merge(favicon(&state, embedded_public))
        .merge(crate::live::router())
        .merge(public_files(&state))
        .layer(axum::extract::DefaultBodyLimit::max(
            state.config.upload_max_size,
        ))
        .layer(
            TraceLayer::new_for_http().make_span_with(|req: &axum::extract::Request| {
                let ip = crate::ClientIp::of(req).map(|ip| ip.to_string());
                let id = req
                    .headers()
                    .get(crate::request_id::HEADER)
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or_default();
                tracing::info_span!(
                    "request",
                    id = %id,
                    method = %req.method(),
                    uri = %req.uri(),
                    ip = ip.as_deref().unwrap_or("unknown"),
                )
            }),
        )
        .layer(from_fn(crate::request_id::middleware))
        .with_state(state.clone());
    // Method spoofing must change the method before the router matches it.
    let limit = state.config.upload_max_size;
    let spoofing = from_fn(
        move |req: axum::extract::Request, next: axum::middleware::Next| {
            crate::method::middleware(req, next, limit)
        },
    );
    // Security headers outermost, so every response gets them, including
    // the method-spoofing layer's own 413.
    Router::new()
        .fallback_service(tower::Layer::layer(&spoofing, router))
        .layer(from_fn_with_state(state, crate::security::middleware))
}

/// Whether the app ships `name` in `public/` (on disk, or embedded).
fn has_public_file(
    state: &AppState,
    embedded_public: Option<&'static [(&'static str, &'static [u8])]>,
    name: &str,
) -> bool {
    match embedded_public {
        Some(files) => files.iter().any(|(path, _)| *path == name),
        None => state.config.public_path.join(name).is_file(),
    }
}

/// The generated `/robots.txt`, unless the app ships its own in `public/`.
fn robots(
    state: &AppState,
    embedded_public: Option<&'static [(&'static str, &'static [u8])]>,
) -> Router<AppState> {
    if has_public_file(state, embedded_public, "robots.txt") {
        Router::new()
    } else {
        crate::seo::robots_router(state)
    }
}

/// `/favicon.ico` answers 204 (no icon, cached for a day) unless the app
/// ships one in `public/`: browsers ask for it on every site, and a 404
/// would run the whole stack, render the error page and log it each time.
fn favicon(
    state: &AppState,
    embedded_public: Option<&'static [(&'static str, &'static [u8])]>,
) -> Router<AppState> {
    if has_public_file(state, embedded_public, "favicon.ico") {
        return Router::new();
    }
    Router::new().route(
        "/favicon.ico",
        axum::routing::get(|| async {
            (
                axum::http::StatusCode::NO_CONTENT,
                [(axum::http::header::CACHE_CONTROL, "public, max-age=86400")],
            )
        }),
    )
}

/// Public files of the local disk at `/storage/...`, outside sessions.
fn public_files(state: &AppState) -> Router<AppState> {
    match state.storage.public_root() {
        Some(root) => Router::new()
            .nest_service("/storage", ServeDir::new(root))
            .layer(axum::middleware::map_response(user_file_headers)),
        None => Router::new(),
    }
}

/// Uploaded files are other people's content served from the app's origin:
/// a sandbox CSP keeps any HTML or SVG among them from running scripts,
/// `nosniff` stops browsers guessing a type, and documents are downloaded
/// instead of displayed.
pub(crate) async fn user_file_headers(
    mut res: axum::response::Response,
) -> axum::response::Response {
    use axum::http::HeaderValue;
    use axum::http::header::{
        CONTENT_DISPOSITION, CONTENT_SECURITY_POLICY, CONTENT_TYPE, X_CONTENT_TYPE_OPTIONS,
    };

    let content_type = res
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let headers = res.headers_mut();
    headers.insert(
        CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(
            "default-src 'none'; img-src 'self' data:; style-src 'unsafe-inline'; sandbox",
        ),
    );
    headers.insert(X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    let document = ["html", "xml", "javascript", "ecmascript"]
        .iter()
        .any(|kind| content_type.contains(kind))
        && !content_type.contains("svg");
    if document {
        headers.insert(CONTENT_DISPOSITION, HeaderValue::from_static("attachment"));
    }
    res
}

/// Long-running commands log at info; others only log warnings and errors.
fn init_tracing(config: &Config, long_running: bool) {
    let default = match (long_running, config.debug) {
        (true, true) => "info,renox=debug",
        (true, false) => "info",
        (false, _) => "warn",
    };
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default));
    // LOG_FILE appends to a file (rotate it with logrotate, or leave logs to
    // journald/Docker on stdout); LOG_FORMAT=json writes one JSON object per
    // line, with the request span's fields (id, method, uri, ip).
    let file = config.log_file.as_ref().and_then(|path| {
        match std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        {
            Ok(file) => Some(std::sync::Mutex::new(file)),
            Err(err) => {
                eprintln!("LOG_FILE {}: {err}; logging to stdout", path.display());
                None
            }
        }
    });
    let json = config.log_format == "json";
    let builder = tracing_subscriber::fmt().with_env_filter(filter);
    let _ = match (json, file) {
        (true, Some(file)) => builder
            .json()
            .with_current_span(true)
            .with_span_list(false)
            .with_writer(file)
            .try_init(),
        (true, None) => builder
            .json()
            .with_current_span(true)
            .with_span_list(false)
            .try_init(),
        (false, Some(file)) => builder.with_ansi(false).with_writer(file).try_init(),
        (false, None) => builder.try_init(),
    };
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
