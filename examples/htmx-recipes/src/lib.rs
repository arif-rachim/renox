//! Example: htmx and Alpine recipes on one task list, each a few lines of
//! HTML and a handler that answers with a fragment:
//!
//! - a modal form (Alpine) that adds a row without a page load;
//! - inline edit: double-click a title, save with `hx-patch`, Escape cancels;
//! - a checkbox that toggles a task in place;
//! - a row menu (Alpine dropdown) with delete, removing just that row;
//! - infinite scroll: the last row loads the next page when it scrolls into view;
//! - tabs (Alpine) that filter the rows without a request;
//! - `HxRefresh` (reload the page) and `HxRedirect` (go elsewhere) after an action.
//!
//! Every action also works as a plain form post, and the tests check both.
//!
//! ```text
//! cargo run -- migrate
//! cargo run -- db:seed    # 40 tasks, to see infinite scroll
//! cargo run
//! ```

pub mod app;

use renox::prelude::*;

use app::tasks::Task;

pub fn app() -> App {
    App::new()
        .embed(renox::embedded!())
        .migrations(renox::migrations!())
        .module(app::tasks::Tasks)
        .seeder(|state| async move {
            let db = state.db;
            Task::factory().count(40).create(&db).await?;
            Ok(())
        })
}
