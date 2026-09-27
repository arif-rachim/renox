//! The app's own commands, run like the built-in ones: `my-app admin:create
//! --email a@b.c` (or `rnx admin:create …` during development).
//!
//! ```
//! # use renox::prelude::*;
//! use renox::command::Args;
//!
//! async fn create_admin(state: AppState, args: Args) -> Result {
//!     let (Some(email), Some(password)) = (args.value("--email"), args.value("--password")) else {
//!         return Err(Error::BadRequest("usage: admin:create --email E --password P".into()));
//!     };
//!     User::register(&state.db, "Admin", email, password).await?;
//!     println!("Created {email}.");
//!     Ok(())
//! }
//!
//! # let _ =
//! App::new().command("admin:create", "Create an admin user (--email, --password)", create_admin)
//! # ;
//! ```
//!
//! Commands run after the app boots (the database is connected, migrations
//! are not run for you) and appear in `my-app help`.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::{AppState, Result};

/// The words after the command's name.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Args(Vec<String>);

impl Args {
    pub fn new(args: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self(args.into_iter().map(Into::into).collect())
    }

    /// Every word, as given.
    pub fn all(&self) -> &[String] {
        &self.0
    }

    /// The words that aren't flags or flag values, e.g. `["report.csv"]` for
    /// `import report.csv --dry-run`.
    pub fn positional(&self) -> Vec<&str> {
        let mut out = Vec::new();
        let mut words = self.0.iter().peekable();
        while let Some(word) = words.next() {
            if word.starts_with("--") {
                // `--flag value`: skip the value too.
                if !word.contains('=') && words.peek().is_some_and(|next| !next.starts_with("--")) {
                    words.next();
                }
            } else {
                out.push(word.as_str());
            }
        }
        out
    }

    /// The value of `--flag value` or `--flag=value`.
    pub fn value(&self, flag: &str) -> Option<&str> {
        let mut words = self.0.iter();
        while let Some(word) = words.next() {
            if word == flag {
                return words
                    .next()
                    .map(String::as_str)
                    .filter(|v| !v.starts_with("--"));
            }
            if let Some(value) = word
                .strip_prefix(flag)
                .and_then(|rest| rest.strip_prefix('='))
            {
                return Some(value);
            }
        }
        None
    }

    /// Whether `--flag` was given (with or without a value).
    pub fn has(&self, flag: &str) -> bool {
        self.0
            .iter()
            .any(|word| word == flag || word.starts_with(&format!("{flag}=")))
    }
}

pub(crate) type CommandFn =
    Arc<dyn Fn(AppState, Args) -> Pin<Box<dyn Future<Output = Result> + Send>> + Send + Sync>;

#[derive(Clone)]
pub(crate) struct Command {
    pub name: String,
    pub about: String,
    pub run: CommandFn,
}

pub(crate) fn command<F, Fut>(name: &str, about: &str, run: F) -> Command
where
    F: Fn(AppState, Args) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result> + Send + 'static,
{
    Command {
        name: name.to_owned(),
        about: about.to_owned(),
        run: Arc::new(move |state, args| Box::pin(run(state, args))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_flags_and_positional_words() {
        let args = Args::new(["report.csv", "--email", "a@b.c", "--dry-run", "--limit=5"]);
        assert_eq!(args.value("--email"), Some("a@b.c"));
        assert_eq!(args.value("--limit"), Some("5"));
        assert_eq!(args.value("--dry-run"), None);
        assert!(args.has("--dry-run") && args.has("--limit") && !args.has("--force"));
        assert_eq!(args.positional(), ["report.csv"]);
    }
}
