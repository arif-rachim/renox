use std::env;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use anyhow::{Context, bail};

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
