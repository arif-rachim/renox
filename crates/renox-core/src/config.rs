use std::env;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;

use anyhow::{Context, bail};

use crate::mail::MailConfig;

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
    /// Language of built-in messages, `en` or `id`, from `APP_LOCALE`.
    pub locale: String,
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
            mail: MailConfig {
                mailer: "memory".into(),
                ..MailConfig::default()
            },
            queue_workers: 0,
            scheduler: false,
            timezone: "UTC".into(),
            cache_store: "memory".into(),
            storage_path: "storage".into(),
        }
    }
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
