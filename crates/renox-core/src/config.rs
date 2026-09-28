use std::env;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, bail};

use crate::mail::MailConfig;
use crate::storage::StorageConfig;

/// The environment the application runs in, from `APP_ENV`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Environment {
    Local,
    Testing,
    Production,
}

impl Environment {
    fn parse(value: &str) -> anyhow::Result<Self> {
        match value.to_ascii_lowercase().as_str() {
            "local" | "dev" | "development" => Ok(Self::Local),
            "testing" | "test" => Ok(Self::Testing),
            "production" | "prod" => Ok(Self::Production),
            other => bail!("APP_ENV must be local, testing or production, got `{other}`"),
        }
    }
}

/// How strict the Content-Security-Policy header is, from `CSP`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum CspMode {
    /// Scripts from this site, inline scripts and `eval` (which Alpine.js's
    /// standard build needs) are allowed; other sites' scripts, framing by
    /// other sites and plugins are not.
    #[default]
    Relaxed,
    /// Scripts only from this site or with `nonce="{{ csp_nonce() }}"`, no
    /// `eval`: Renox switches to Alpine's CSP build (expressions are limited
    /// to properties and methods; put logic in `Alpine.data(...)`) and turns
    /// off htmx's `eval`.
    Strict,
    /// No Content-Security-Policy header.
    Off,
}

impl CspMode {
    fn parse(value: &str) -> anyhow::Result<Self> {
        match value.to_ascii_lowercase().as_str() {
            "relaxed" | "" => Ok(Self::Relaxed),
            "strict" => Ok(Self::Strict),
            "off" | "false" | "none" => Ok(Self::Off),
            other => bail!("CSP must be relaxed, strict or off, got `{other}`"),
        }
    }
}

/// Application configuration, read from the process environment and `.env`.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Config {
    pub name: String,
    pub env: Environment,
    /// Show error details and backtraces in responses. Never enable in production.
    pub debug: bool,
    pub url: String,
    /// Secret used for signing and encryption. Required in production.
    pub key: Option<String>,
    pub host: IpAddr,
    pub port: u16,
    /// Where templates live, from `VIEWS_PATH`.
    pub views_path: PathBuf,
    /// Files served as-is at the site root, from `PUBLIC_PATH`.
    pub public_path: PathBuf,
    /// Minutes of inactivity before a session expires, from `SESSION_LIFETIME`.
    pub session_lifetime: u64,
    /// Name of the session cookie, from `SESSION_COOKIE`.
    pub session_cookie: String,
    /// Minutes a "remember me" login lasts, from `REMEMBER_LIFETIME` (30 days).
    pub remember_lifetime: u64,
    /// SQLite database, from `DATABASE_URL`, e.g. `sqlite://storage/app.db`.
    pub database_url: String,
    /// Maximum open connections, from `DATABASE_POOL_SIZE`.
    pub database_pool_size: u32,
    /// How long a query waits for a free connection before failing, from
    /// `DATABASE_ACQUIRE_TIMEOUT` in seconds (default 5).
    pub database_acquire_timeout: Duration,
    /// Longest a PostgreSQL statement may run, from `DATABASE_STATEMENT_TIMEOUT`
    /// in seconds (default 30; 0 for none).
    pub database_statement_timeout: Option<Duration>,
    /// Longest a request may take to answer, from `REQUEST_TIMEOUT` in
    /// seconds (default 60; 0 for none). Streaming a response isn't counted.
    pub request_timeout: Option<Duration>,
    /// Default language of the app, from `APP_LOCALE`. Built-in messages exist
    /// for `en` and `id`; other locales need a lang file.
    pub locale: String,
    /// Language used for keys missing in the request's locale, from `APP_FALLBACK_LOCALE`.
    pub fallback_locale: String,
    /// Where translation files live, from `LANG_PATH`.
    pub lang_path: PathBuf,
    /// Mail settings, from `MAIL_*`.
    pub mail: MailConfig,
    /// Queue workers `serve` runs in-process, from `QUEUE_WORKERS` (0 turns them off).
    pub queue_workers: usize,
    /// Whether `serve` runs scheduled tasks, from `SCHEDULER`.
    pub scheduler: bool,
    /// `text` (default) or `json` (one object per line), from `LOG_FORMAT`.
    pub log_format: String,
    /// Where logs go instead of stdout, from `LOG_FILE` (appended to).
    pub log_file: Option<PathBuf>,
    /// The zone of scheduled times and the `date` filter, from
    /// `APP_TIMEZONE`: an IANA name (`Asia/Jakarta`), an offset (`+07:00`) or
    /// `UTC`. See [`crate::timezone::Zone`].
    pub timezone: String,
    /// `memory` or `database`, from `CACHE_STORE`.
    pub cache_store: String,
    /// Where the app keeps runtime files (maintenance flag, uploads), from `STORAGE_PATH`.
    pub storage_path: PathBuf,
    /// File storage, from `STORAGE_DISK`, `S3_*` and `STORAGE_URL`.
    pub storage: StorageConfig,
    /// Largest request body in bytes, from `UPLOAD_MAX_SIZE` in megabytes (default 10).
    pub upload_max_size: usize,
    /// The Content-Security-Policy, from `CSP` (`relaxed`, `strict` or `off`).
    pub csp: CspMode,
    /// Values `var()` returns before looking at the environment, e.g. a
    /// webhook secret set in a test with `TestApp::with_config`.
    pub vars: std::collections::HashMap<String, String>,
    /// Search engines and analytics; used only in production.
    pub analytics: AnalyticsConfig,
    /// Reverse proxies whose `X-Forwarded-For` is believed, from `TRUSTED_PROXIES`.
    pub trusted_proxies: crate::TrustedProxies,
}

/// Google Search Console, Google Analytics 4 and Google Tag Manager, from
/// `GOOGLE_SITE_VERIFICATION`, `GA4_MEASUREMENT_ID`, `GA4_API_SECRET` and
/// `GTM_CONTAINER_ID`. Tags are added to pages only in production.
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct AnalyticsConfig {
    /// The `content` of Search Console's `google-site-verification` meta tag.
    pub google_site_verification: Option<String>,
    /// e.g. `G-XXXXXXXXXX`: adds the GA4 tag to every page.
    pub ga4_measurement_id: Option<String>,
    /// For events sent from the server (Measurement Protocol).
    pub ga4_api_secret: Option<String>,
    /// e.g. `GTM-XXXXXXX`: adds the Tag Manager container to every page.
    pub gtm_container_id: Option<String>,
}

impl Config {
    /// Loads `.env` (if present) and reads the configuration from the environment.
    pub fn load() -> anyhow::Result<Self> {
        let _ = dotenvy::dotenv();
        Self::from_env()
    }

    /// Reads the configuration from the environment only, without loading `.env`.
    pub fn from_env() -> anyhow::Result<Self> {
        Self::from_vars(|name| env::var(name).ok())
    }

    /// Reads the configuration from `get` (a variable's value by name), e.g.
    /// a map in a test: `Config::from_vars(|name| vars.get(name).cloned())`.
    pub fn from_vars(get: impl Fn(&str) -> Option<String>) -> anyhow::Result<Self> {
        let v = Vars(get);
        let env = Environment::parse(&v.or("APP_ENV", "local"))?;
        let debug = v.bool("APP_DEBUG", env == Environment::Local)?;
        let host = v
            .or("APP_HOST", "127.0.0.1")
            .parse()
            .context("APP_HOST must be an IP address")?;
        let port = v
            .or("APP_PORT", "3000")
            .parse()
            .context("APP_PORT must be a port number")?;
        let key = v.get("APP_KEY").filter(|k| !k.is_empty());

        if env == Environment::Production && key.is_none() {
            bail!("APP_KEY must be set in production");
        }

        Ok(Self {
            name: v.or("APP_NAME", "Renox"),
            env,
            debug,
            url: v.or("APP_URL", &format!("http://{host}:{port}")),
            key,
            host,
            port,
            views_path: v.or("VIEWS_PATH", "resources/views").into(),
            public_path: v.or("PUBLIC_PATH", "public").into(),
            session_lifetime: v
                .or("SESSION_LIFETIME", "120")
                .parse()
                .context("SESSION_LIFETIME must be a number of minutes")?,
            session_cookie: v.or("SESSION_COOKIE", "renox_session"),
            remember_lifetime: v
                .or("REMEMBER_LIFETIME", "43200")
                .parse()
                .context("REMEMBER_LIFETIME must be a number of minutes")?,
            database_url: v.or("DATABASE_URL", "sqlite://storage/app.db"),
            database_pool_size: v
                .or("DATABASE_POOL_SIZE", "8")
                .parse()
                .ok()
                .filter(|n| *n > 0)
                .context("DATABASE_POOL_SIZE must be a number above 0")?,
            database_acquire_timeout: v
                .seconds("DATABASE_ACQUIRE_TIMEOUT", 5)?
                .unwrap_or(Duration::from_secs(5)),
            database_statement_timeout: v.seconds("DATABASE_STATEMENT_TIMEOUT", 30)?,
            request_timeout: v.seconds("REQUEST_TIMEOUT", 60)?,
            locale: v.or("APP_LOCALE", "en"),
            fallback_locale: v.or("APP_FALLBACK_LOCALE", "en"),
            lang_path: v.or("LANG_PATH", "resources/lang").into(),
            mail: MailConfig {
                mailer: v.or("MAIL_MAILER", "log"),
                host: v.or("MAIL_HOST", "localhost"),
                port: v
                    .get("MAIL_PORT")
                    .filter(|p| !p.is_empty())
                    .map(|p| p.parse())
                    .transpose()
                    .context("MAIL_PORT must be a port number")?,
                username: v.get("MAIL_USERNAME").filter(|v| !v.is_empty()),
                password: v.get("MAIL_PASSWORD").filter(|v| !v.is_empty()),
                encryption: v.or("MAIL_ENCRYPTION", "starttls"),
                from_address: v.or("MAIL_FROM_ADDRESS", "hello@example.com"),
                from_name: v.get("MAIL_FROM_NAME").filter(|v| !v.is_empty()),
                timeout: v
                    .seconds("MAIL_TIMEOUT", 10)?
                    .unwrap_or(Duration::from_secs(10)),
            },
            queue_workers: v
                .or("QUEUE_WORKERS", "2")
                .parse()
                .context("QUEUE_WORKERS must be a number")?,
            scheduler: v.bool("SCHEDULER", true)?,
            log_format: match v.or("LOG_FORMAT", "text").as_str() {
                format @ ("text" | "json") => format.to_owned(),
                other => bail!("LOG_FORMAT must be text or json, got `{other}`"),
            },
            log_file: v
                .get("LOG_FILE")
                .filter(|p| !p.is_empty())
                .map(PathBuf::from),
            timezone: v.or("APP_TIMEZONE", "UTC"),
            cache_store: v.or("CACHE_STORE", "memory"),
            storage_path: v.or("STORAGE_PATH", "storage").into(),
            storage: StorageConfig {
                disk: v.or("STORAGE_DISK", "local"),
                bucket: v.optional("S3_BUCKET"),
                region: v.optional("S3_REGION"),
                endpoint: v.optional("S3_ENDPOINT"),
                access_key_id: v.optional("S3_ACCESS_KEY_ID"),
                secret_access_key: v.optional("S3_SECRET_ACCESS_KEY"),
                url: v.optional("STORAGE_URL"),
            },
            upload_max_size: v
                .or("UPLOAD_MAX_SIZE", "10")
                .parse::<usize>()
                .ok()
                .and_then(|mb| mb.checked_mul(1024 * 1024))
                .context("UPLOAD_MAX_SIZE must be a number of megabytes")?,
            csp: CspMode::parse(&v.or("CSP", "relaxed"))?,
            trusted_proxies: crate::TrustedProxies::parse(&v.or("TRUSTED_PROXIES", ""))?,
            vars: Default::default(),
            analytics: AnalyticsConfig {
                google_site_verification: v.optional("GOOGLE_SITE_VERIFICATION"),
                ga4_measurement_id: v.optional("GA4_MEASUREMENT_ID"),
                ga4_api_secret: v.optional("GA4_API_SECRET"),
                gtm_container_id: v.optional("GTM_CONTAINER_ID"),
            },
        })
    }

    /// Any other setting, e.g. an API key: `config.vars`, else the
    /// environment (which includes `.env`). Empty values count as missing.
    pub fn var(&self, name: &str) -> Option<String> {
        self.vars
            .get(name)
            .cloned()
            .or_else(|| env::var(name).ok())
            .filter(|v| !v.is_empty())
    }

    pub fn addr(&self) -> SocketAddr {
        SocketAddr::new(self.host, self.port)
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            name: "Renox".into(),
            env: Environment::Local,
            debug: true,
            url: "http://127.0.0.1:3000".into(),
            key: None,
            host: IpAddr::V4(Ipv4Addr::LOCALHOST),
            port: 3000,
            views_path: "resources/views".into(),
            public_path: "public".into(),
            session_lifetime: 120,
            session_cookie: "renox_session".into(),
            remember_lifetime: 43_200,
            database_url: "sqlite::memory:".into(),
            database_pool_size: 8,
            // Generous: parallel test suites open many SQLite files at once.
            // (An in-memory database still fails fast; see `db::connect`.)
            database_acquire_timeout: Duration::from_secs(30),
            database_statement_timeout: Some(Duration::from_secs(30)),
            request_timeout: Some(Duration::from_secs(60)),
            locale: "en".into(),
            fallback_locale: "en".into(),
            lang_path: "resources/lang".into(),
            mail: MailConfig {
                mailer: "memory".into(),
                ..MailConfig::default()
            },
            queue_workers: 0,
            scheduler: false,
            log_format: "text".into(),
            log_file: None,
            timezone: "UTC".into(),
            cache_store: "memory".into(),
            storage_path: "storage".into(),
            storage: StorageConfig::default(),
            upload_max_size: 10 * 1024 * 1024,
            csp: CspMode::Relaxed,
            vars: Default::default(),
            analytics: AnalyticsConfig::default(),
            trusted_proxies: Default::default(),
        }
    }
}

/// Variables by name, for `Config::from_vars`.
struct Vars<F>(F);

impl<F: Fn(&str) -> Option<String>> Vars<F> {
    fn get(&self, name: &str) -> Option<String> {
        (self.0)(name)
    }

    fn optional(&self, name: &str) -> Option<String> {
        self.get(name).filter(|v| !v.is_empty())
    }

    fn or(&self, name: &str, default: &str) -> String {
        self.get(name).unwrap_or_else(|| default.to_owned())
    }

    /// A number of seconds from `name`; `None` when it is 0 (no limit).
    fn seconds(&self, name: &str, default: u64) -> anyhow::Result<Option<Duration>> {
        let value: u64 = self
            .or(name, &default.to_string())
            .parse()
            .with_context(|| format!("{name} must be a number of seconds"))?;
        Ok((value > 0).then(|| Duration::from_secs(value)))
    }

    fn bool(&self, name: &str, default: bool) -> anyhow::Result<bool> {
        match self.get(name) {
            None => Ok(default),
            Some(v) => match v.to_ascii_lowercase().as_str() {
                "1" | "true" | "yes" | "on" => Ok(true),
                "0" | "false" | "no" | "off" | "" => Ok(false),
                other => bail!("{name} must be true or false, got `{other}`"),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_environment_aliases() {
        assert_eq!(Environment::parse("local").unwrap(), Environment::Local);
        assert_eq!(Environment::parse("DEV").unwrap(), Environment::Local);
        assert_eq!(Environment::parse("test").unwrap(), Environment::Testing);
        assert_eq!(Environment::parse("prod").unwrap(), Environment::Production);
        assert!(Environment::parse("staging").is_err());
    }

    fn load(pairs: &[(&str, &str)]) -> anyhow::Result<Config> {
        let vars: std::collections::HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        Config::from_vars(|name| vars.get(name).cloned())
    }

    #[test]
    fn defaults_without_any_variable() {
        let c = load(&[]).unwrap();
        assert_eq!(c.env, Environment::Local);
        assert!(c.debug, "local defaults to debug");
        assert_eq!(
            (c.port, c.database_pool_size, c.upload_max_size),
            (3000, 8, 10 * 1024 * 1024)
        );
        assert_eq!(c.database_acquire_timeout, Duration::from_secs(5));
        assert_eq!(c.request_timeout, Some(Duration::from_secs(60)));
        assert_eq!(c.cache_store, "memory");
        assert_eq!(c.csp, CspMode::Relaxed);
        assert!(c.key.is_none() && c.mail.port.is_none());
    }

    #[test]
    fn log_format_and_file() {
        let c = load(&[]).unwrap();
        assert_eq!((c.log_format.as_str(), c.log_file), ("text", None));
        let c = load(&[("LOG_FORMAT", "json"), ("LOG_FILE", "storage/logs/app.log")]).unwrap();
        assert_eq!(c.log_format, "json");
        assert_eq!(c.log_file, Some(PathBuf::from("storage/logs/app.log")));
        let err = load(&[("LOG_FORMAT", "xml")]).unwrap_err();
        assert!(err.to_string().contains("LOG_FORMAT"), "{err}");
    }

    #[test]
    fn reads_every_kind_of_value() {
        let c = load(&[
            ("APP_ENV", "production"),
            ("APP_KEY", "base64:abc"),
            ("APP_DEBUG", "off"),
            ("APP_PORT", "8080"),
            ("DATABASE_POOL_SIZE", "3"),
            ("REQUEST_TIMEOUT", "0"),
            ("DATABASE_STATEMENT_TIMEOUT", "5"),
            ("UPLOAD_MAX_SIZE", "2"),
            ("MAIL_PORT", "2525"),
            ("MAIL_USERNAME", ""),
            ("CSP", "strict"),
            ("TRUSTED_PROXIES", "10.0.0.0/8"),
            ("S3_BUCKET", "files"),
        ])
        .unwrap();
        assert_eq!(c.env, Environment::Production);
        assert!(!c.debug);
        assert_eq!((c.port, c.database_pool_size), (8080, 3));
        assert_eq!(c.request_timeout, None, "0 means no limit");
        assert_eq!(c.database_statement_timeout, Some(Duration::from_secs(5)));
        assert_eq!(c.upload_max_size, 2 * 1024 * 1024);
        assert_eq!(c.mail.port, Some(2525));
        assert_eq!(c.mail.username, None, "empty means unset");
        assert_eq!(c.csp, CspMode::Strict);
        assert!(c.trusted_proxies.contains("10.1.2.3".parse().unwrap()));
        assert_eq!(c.storage.bucket.as_deref(), Some("files"));
    }

    #[test]
    fn refuses_bad_values_with_the_variable_name() {
        for (name, value) in [
            ("APP_ENV", "staging"),
            ("APP_DEBUG", "maybe"),
            ("APP_PORT", "http"),
            ("APP_HOST", "localhost"),
            ("DATABASE_POOL_SIZE", "0"),
            ("REQUEST_TIMEOUT", "-1"),
            ("UPLOAD_MAX_SIZE", "99999999999999999"),
            ("MAIL_PORT", "99999"),
            ("CSP", "loose"),
            ("TRUSTED_PROXIES", "proxy.local"),
        ] {
            let err = load(&[(name, value)])
                .err()
                .map(|e| format!("{e:#}"))
                .unwrap_or_default();
            assert!(
                err.contains(name) || err.contains(value),
                "{name}={value}: {err}"
            );
        }
        let err = load(&[("APP_ENV", "production")]).unwrap_err();
        assert!(
            format!("{err}").contains("APP_KEY"),
            "production needs a key"
        );
    }
}
