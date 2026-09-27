use std::any::TypeId;
use std::collections::HashMap;
use std::future::Future;

use crate::events::{Event, listener};
use crate::queue::{Job, JobHandler, handler};
use crate::schedule::Schedule;
use crate::{AppState, Result};

/// Where the app and its modules register jobs, listeners, scheduled tasks
/// and commands. Modules get it in `Module::register`.
#[derive(Default)]
pub struct Registry {
    pub(crate) jobs: HashMap<&'static str, JobHandler>,
    pub(crate) listeners: HashMap<TypeId, Vec<crate::events::ListenerFn>>,
    pub(crate) schedule: Schedule,
    pub(crate) duplicate_job: Option<&'static str>,
    pub(crate) webhooks: HashMap<&'static str, crate::webhook::HandleFn>,
    pub(crate) commands: Vec<crate::command::Command>,
}

impl Registry {
    /// Lets workers run jobs of type `J`.
    pub fn job<J: Job>(&mut self) -> &mut Self {
        if self.jobs.insert(J::NAME, handler::<J>()).is_some() {
            self.duplicate_job.get_or_insert(J::NAME);
        }
        self
    }

    /// Lets queue workers process `W`'s webhooks; pair it with
    /// `Routes::webhook::<W>(path)`.
    pub fn webhook<W: crate::webhook::Webhook>(&mut self) -> &mut Self {
        if self
            .webhooks
            .insert(W::PROVIDER, crate::webhook::handler::<W>())
            .is_some()
        {
            self.duplicate_job.get_or_insert(W::PROVIDER);
        }
        self
    }

    /// Runs `listener` whenever an `E` is emitted.
    pub fn listen<E, F, Fut>(&mut self, listener_fn: F) -> &mut Self
    where
        E: Event,
        F: Fn(E, AppState) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result> + Send + 'static,
    {
        let (type_id, run) = listener(listener_fn);
        self.listeners.entry(type_id).or_default().push(run);
        self
    }

    /// Adds a command the app binary runs: `my-app <name> [args]`. See
    /// [`crate::command`].
    pub fn command<F, Fut>(&mut self, name: &str, about: &str, run: F) -> &mut Self
    where
        F: Fn(AppState, crate::command::Args) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result> + Send + 'static,
    {
        self.commands
            .push(crate::command::command(name, about, run));
        self
    }

    pub fn schedule(&mut self) -> &mut Schedule {
        &mut self.schedule
    }
}
