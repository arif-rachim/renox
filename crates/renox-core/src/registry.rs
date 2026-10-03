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
    pub(crate) templates: Vec<crate::view::TemplateHook>,
    pub(crate) shares: Vec<(String, crate::view::ShareFn)>,
    pub(crate) channels: HashMap<String, crate::auth::notifications::ChannelFn>,
    pub(crate) reporters: Vec<crate::report::ReportFn>,
    /// The `Permissions` module is on: load each user's roles.
    pub(crate) permissions: bool,
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
        F: Fn(crate::command::Args, AppState) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result> + Send + 'static,
    {
        self.commands
            .push(crate::command::command(name, about, run));
        self
    }

    /// A command whose arguments are declared with clap; see
    /// [`AppCommand`](crate::command::AppCommand).
    pub fn typed_command<T: crate::command::AppCommand>(&mut self) -> &mut Self {
        self.commands.push(crate::command::typed::<T>());
        self
    }

    /// Adds template functions, filters or globals, e.g. a `rupiah` filter:
    ///
    /// ```
    /// # use renox::prelude::*;
    /// # let _ =
    /// App::new().templates(|env| {
    ///     env.add_filter("rupiah", |n: i64| format!("Rp {}", renox::format_number(n as f64, 0, "de")));
    /// })
    /// # ;
    /// ```
    ///
    /// Built in: `number` (`{{ price | number }}` → `75.000` in German,
    /// `number(2)` for decimals) and `date` (`{{ created_at | date("%d/%m/%Y") }}`,
    /// in `APP_TIMEZONE`).
    pub fn templates(
        &mut self,
        hook: impl Fn(&mut minijinja::Environment<'static>) + Send + Sync + 'static,
    ) -> &mut Self {
        self.templates.push(std::sync::Arc::new(hook));
        self
    }

    /// Gives every view `key`, computed per request, e.g. the categories in
    /// a menu or the number of items in a cart. It runs for every rendered
    /// view, so keep it quick (cache what doesn't change per request).
    ///
    /// ```
    /// # use renox::prelude::*;
    /// # use std::time::Duration;
    /// # let _ =
    /// App::new().share("cart_count", |ctx: renox::view::ViewContext| async move {
    ///     let Some(user) = ctx.user else { return Ok(0) };
    ///     let n: i64 = renox::db::sql("SELECT COUNT(*) FROM cart_items WHERE user_id = ?")
    ///         .bind(user.id)
    ///         .scalar(&ctx.state.db)
    ///         .await?;
    ///     Ok(n)
    /// })
    /// # ;
    /// ```
    ///
    /// Values a handler passes in `context!` win over shared ones.
    pub fn share<F, Fut, T>(&mut self, key: &str, compute: F) -> &mut Self
    where
        F: Fn(crate::view::ViewContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T>> + Send + 'static,
        T: serde::Serialize,
    {
        self.shares
            .push((key.to_owned(), crate::view::share_fn(compute)));
        self
    }

    /// Adds a notification channel, used by notifications that list
    /// `Channel::Custom(name)`: `send` gets the recipient and the message
    /// `Notification::to_channel` built. See [`crate::auth::notifications`].
    pub fn channel<F, Fut>(&mut self, name: &str, send: F) -> &mut Self
    where
        F: Fn(crate::auth::Recipient, serde_json::Value, AppState) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result> + Send + 'static,
    {
        self.channels.insert(
            name.to_owned(),
            crate::auth::notifications::channel_fn(send),
        );
        self
    }

    /// Sends every error that needs a person to `reporter`; see
    /// [`crate::report`].
    pub fn report<F, Fut>(&mut self, reporter: F) -> &mut Self
    where
        F: Fn(crate::report::ErrorReport, AppState) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        self.reporters.push(crate::report::report_fn(reporter));
        self
    }

    /// The schedule, to add tasks to.
    pub fn schedule(&mut self) -> &mut Schedule {
        &mut self.schedule
    }
}
