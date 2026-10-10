//! Live components: server-rendered pieces of a page whose state travels in a
//! signed snapshot and is changed by actions sent back to the server.

mod snapshot;

mod context;

use std::future::Future;

use serde::Serialize;
use serde::de::DeserializeOwned;

pub use context::LiveContext;

use crate::Result;

/// A live component: a serde struct whose fields are the state, and named
/// actions that change it. The state travels in a signed snapshot.
///
/// `call` is a `match` on the action's name; answer unknown names with
/// `Error::NotFound`. Names starting with `_` are reserved for the framework.
///
/// ```
/// # use renox::prelude::*;
/// # use renox::live_component::LiveContext;
/// #[derive(serde::Serialize, serde::Deserialize)]
/// struct Counter {
///     count: i64,
/// }
///
/// impl LiveComponent for Counter {
///     const NAME: &'static str = "counter";
///     const VIEW: &'static str = "live/counter.html";
///
///     async fn call(
///         &mut self,
///         action: &str,
///         _args: Vec<serde_json::Value>,
///         _ctx: &mut LiveContext,
///     ) -> Result {
///         match action {
///             "increment" => {
///                 self.count += 1;
///                 Ok(())
///             }
///             _ => Err(Error::NotFound),
///         }
///     }
/// }
/// ```
pub trait LiveComponent: Serialize + DeserializeOwned + Send + Sync + 'static {
    /// The component's name, used in its route and its snapshot.
    const NAME: &'static str;

    /// The template that renders the component.
    const VIEW: &'static str;

    /// Extra values for the view, next to the state (as `data`). Empty by default.
    ///
    /// ```
    /// # use renox::prelude::*;
    /// # use renox::live_component::LiveContext;
    /// # #[derive(serde::Serialize, serde::Deserialize)]
    /// # struct Items { limit: i64 }
    /// # impl LiveComponent for Items {
    /// #     const NAME: &'static str = "items";
    /// #     const VIEW: &'static str = "live/items.html";
    /// async fn data(&self, ctx: &LiveContext) -> Result<serde_json::Value> {
    ///     let _ = ctx.state();
    ///     Ok(json!({ "shown": self.limit }))
    /// }
    /// #     async fn call(&mut self, _: &str, _: Vec<serde_json::Value>, _: &mut LiveContext) -> Result {
    /// #         Ok(())
    /// #     }
    /// # }
    /// ```
    fn data(&self, ctx: &LiveContext) -> impl Future<Output = Result<serde_json::Value>> + Send {
        let _ = ctx;
        async { Ok(serde_json::Value::Object(Default::default())) }
    }

    /// Runs the action `action` with its `args`, changing `self` and using
    /// `ctx` for the request, toasts, redirects and events.
    ///
    /// ```
    /// # use renox::prelude::*;
    /// # use renox::live_component::LiveContext;
    /// # #[derive(serde::Serialize, serde::Deserialize)]
    /// # struct Note { text: String }
    /// # impl LiveComponent for Note {
    /// #     const NAME: &'static str = "note";
    /// #     const VIEW: &'static str = "live/note.html";
    /// async fn call(
    ///     &mut self,
    ///     action: &str,
    ///     args: Vec<serde_json::Value>,
    ///     ctx: &mut LiveContext,
    /// ) -> Result {
    ///     match action {
    ///         "clear" => {
    ///             self.text.clear();
    ///             ctx.toast(Toast::success("Cleared"));
    ///             Ok(())
    ///         }
    ///         _ => Err(Error::NotFound),
    ///     }
    /// }
    /// # }
    /// ```
    fn call(
        &mut self,
        action: &str,
        args: Vec<serde_json::Value>,
        ctx: &mut LiveContext,
    ) -> impl Future<Output = Result> + Send;
}
