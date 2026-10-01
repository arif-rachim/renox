//! Example: a data grid as a dashboard (`renox::grid` and the
//! `renox/grid.html` macro).
//!
//! - fills the screen: the toolbar and the pagination stay put, only rows
//!   scroll (sideways too, under the frozen columns);
//! - three columns on a phone, all on a desktop; the column menu (top right)
//!   picks them per screen size, reorders them and freezes them left or right,
//!   remembered per user (or in the session for guests);
//! - a filter in each heading by the column's kind: text (contains, starts
//!   with, `%` patterns), number ranges, a date range calendar, choices;
//! - grouped headings (Customer, Location, Amounts, Charts);
//! - custom cells: a sparkline, a progress bar and buttons;
//! - pages, sorting and filters from the server, in the URL;
//! - a click on a row opens who created and last changed it (`audit`);
//! - cells edited in place or a whole row in edit mode, validated by
//!   `Valid<OrderEdit>` (`edit_url`);
//! - rows dragged into order while sorted by # (`reorder`);
//! - a second grid (`/regions`) where equal regions and cities share one cell
//!   (`merge`), with details of the page's own under each row.
//!
//! ```text
//! cargo run -- migrate
//! cargo run -- db:seed    # 480 orders and demo@example.com / password
//! cargo run
//! ```

pub mod app;

use renox::prelude::*;

use app::orders::Order;

pub fn app() -> App {
    App::new()
        .embed(renox::embedded!())
        .migrations(renox::migrations!())
        .module(Auth::new())
        .module(app::orders::Orders)
        .seeder(|db| async move {
            User::register(&db, "Demo", "demo@example.com", "password").await?;
            Order::create_many(&db, 480).await?;
            // Hand-sorted order starts as the order they were made in.
            renox::db::sql("UPDATE orders SET position = id")
                .execute(&db)
                .await?;
            Ok(())
        })
}
