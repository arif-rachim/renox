use std::env;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;

use anyhow::{Context, bail};

use crate::mail::MailConfig;
use crate::storage::StorageConfig;

/// The environment the application runs in, from `APP_ENV`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    /// UTC offset for scheduled times, from `APP_TIMEZONE` (e.g. `+07:00`).
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
}

impl Config {
    /// Loads `.env` (if present) and reads the configuration from the environment.
    pub fn load() -> anyhow::Result<Self> {
        let _ = dotenvy::dotenv();
        Self::from_env()
    }

    /// Reads the configuration from the environment only, without loading `.env`.
    pub fn from_env() -> anyhow::Result<Self> {
        let env = Environment::parse(&var_or("APP_ENV", "local"))?;
        let debug = parse_bool("APP_DEBUG", env == Environment::Local)?;
        let host = var_or("APP_HOST", "127.0.0.1")
            .parse()
            .context("APP_HOST must be an IP address")?;
        let port = var_or("APP_PORT", "3000")
            .parse()
            .context("APP_PORT must be a port number")?;
        let key = env::var("APP_KEY").ok().filter(|k| !k.is_empty());

        if env == Environment::Production && key.is_none() {
            bail!("APP_KEY must be set in production");
        }

        Ok(Self {
            name: var_or("APP_NAME", "Renox"),
            env,
            debug,
            url: var_or("APP_URL", &format!("http://{host}:{port}")),
            key,
            host,
            port,
            views_path: var_or("VIEWS_PATH", "resources/views").into(),
            public_path: var_or("PUBLIC_PATH", "public").into(),
            session_lifetime: var_or("SESSION_LIFETIME", "120")
                .parse()
                .context("SESSION_LIFETIME must be a number of minutes")?,
            session_cookie: var_or("SESSION_COOKIE", "renox_session"),
            remember_lifetime: var_or("REMEMBER_LIFETIME", "43200")
                .parse()
                .context("REMEMBER_LIFETIME must be a number of minutes")?,
            database_url: var_or("DATABASE_URL", "sqlite://storage/app.db"),
            database_pool_size: var_or("DATABASE_POOL_SIZE", "8")
                .parse()
                .context("DATABASE_POOL_SIZE must be a number")?,
            locale: var_or("APP_LOCALE", "en"),
            fallback_locale: var_or("APP_FALLBACK_LOCALE", "en"),
            lang_path: var_or("LANG_PATH", "resources/lang").into(),
            mail: MailConfig {
                mailer: var_or("MAIL_MAILER", "log"),
                host: var_or("MAIL_HOST", "localhost"),
                port: env::var("MAIL_PORT")
                    .ok()
                    .filter(|p| !p.is_empty())
                    .map(|p| p.parse())
                    .transpose()
                    .context("MAIL_PORT must be a port number")?,
                username: env::var("MAIL_USERNAME").ok().filter(|v| !v.is_empty()),
                password: env::var("MAIL_PASSWORD").ok().filter(|v| !v.is_empty()),
                encryption: var_or("MAIL_ENCRYPTION", "starttls"),
                from_address: var_or("MAIL_FROM_ADDRESS", "hello@example.com"),
                from_name: env::var("MAIL_FROM_NAME").ok().filter(|v| !v.is_empty()),
            },
            queue_workers: var_or("QUEUE_WORKERS", "2")
                .parse()
                .context("QUEUE_WORKERS must be a number")?,
            scheduler: parse_bool("SCHEDULER", true)?,
            timezone: var_or("APP_TIMEZONE", "UTC"),
            cache_store: var_or("CACHE_STORE", "memory"),
            storage_path: var_or("STORAGE_PATH", "storage").into(),
            storage: StorageConfig {
                disk: var_or("STORAGE_DISK", "local"),
                bucket: optional("S3_BUCKET"),
                region: optional("S3_REGION"),
                endpoint: optional("S3_ENDPOINT"),
                access_key_id: optional("S3_ACCESS_KEY_ID"),
                secret_access_key: optional("S3_SECRET_ACCESS_KEY"),
                url: optional("STORAGE_URL"),
            },
            upload_max_size: var_or("UPLOAD_MAX_SIZE", "10")
                .parse::<usize>()
                .context("UPLOAD_MAX_SIZE must be a number of megabytes")?
                * 1024
                * 1024,
            csp: CspMode::parse(&var_or("CSP", "relaxed"))?,
        })
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
            locale: "en".into(),
            fallback_locale: "en".into(),
            lang_path: "resources/lang".into(),
            mail: MailConfig {
                mailer: "memory".into(),
                ..MailConfig::default()
            },
            queue_workers: 0,
            scheduler: false,
            timezone: "UTC".into(),
            cache_store: "memory".into(),
            storage_path: "storage".into(),
            storage: StorageConfig::default(),
            upload_max_size: 10 * 1024 * 1024,
            csp: CspMode::Relaxed,
        }
    }
}

fn optional(name: &str) -> Option<String> {
    env::var(name).ok().filter(|v| !v.is_empty())
}

fn var_or(name: &str, default: &str) -> String {
    env::var(name).unwrap_or_else(|_| default.to_owned())
}

fn parse_bool(name: &str, default: bool) -> anyhow::Result<bool> {
    match env::var(name) {
        Err(_) => Ok(default),
        Ok(v) => match v.to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => Ok(true),
            "0" | "false" | "no" | "off" | "" => Ok(false),
            other => bail!("{name} must be true or false, got `{other}`"),
        },
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
}
