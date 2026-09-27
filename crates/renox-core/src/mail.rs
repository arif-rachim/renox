//! Sending email. M4 ships the `log` and `memory` drivers; SMTP arrives in M5.
//!
//! `MAIL_MAILER=log` (the default) writes each message to the log, which is
//! enough to click password reset and verification links in development.
//! `memory` keeps messages for tests: `kernel.mailer().sent()`.

use std::sync::{Arc, Mutex};

use anyhow::bail;
use serde::Serialize;

use crate::Result;

/// One email message.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Mail {
    pub to: String,
    pub subject: String,
    pub text: String,
}

impl Mail {
    pub fn new(to: impl Into<String>, subject: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            to: to.into(),
            subject: subject.into(),
            text: text.into(),
        }
    }
}

/// Where mail goes, from `MAIL_MAILER`.
#[derive(Clone)]
pub enum Mailer {
    Log,
    Memory(Arc<Mutex<Vec<Mail>>>),
}

impl Mailer {
    pub(crate) fn from_name(name: &str) -> anyhow::Result<Self> {
        match name {
            "log" => Ok(Self::Log),
            "memory" => Ok(Self::Memory(Arc::default())),
            other => {
                bail!("MAIL_MAILER must be `log` or `memory`, got `{other}` (SMTP arrives in M5)")
            }
        }
    }

    pub async fn send(&self, mail: Mail) -> Result {
        match self {
            Self::Log => tracing::info!(
                "mail (log driver)\nTo: {}\nSubject: {}\n\n{}\n",
                mail.to,
                mail.subject,
                mail.text
            ),
            Self::Memory(sent) => sent.lock().unwrap_or_else(|e| e.into_inner()).push(mail),
        }
        Ok(())
    }

    /// Messages sent so far with the `memory` driver.
    pub fn sent(&self) -> Vec<Mail> {
        match self {
            Self::Memory(sent) => sent.lock().unwrap_or_else(|e| e.into_inner()).clone(),
            Self::Log => Vec::new(),
        }
    }
}
