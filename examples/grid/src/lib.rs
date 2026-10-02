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
//!   (`merge`), with details of the page's own under each row;
//! - exports of every filtered row: CSV, Excel (the `xlsx` feature) and a
//!   print page (`exports`);
//! - a search box (`searchable`), filter chips, rows that open their order
//!   (`row_url`) and an empty state (`empty_state`);
//! - bulk actions on the selected (or all matching) orders and a row menu
//!   (`bulk_action`, `row_action`);
//! - totals in the footer (`Column::summary`) and grouping by region, status
//!   or paid with subtotals (`groups`);
//! - cards on phones (`cards_on_mobile`), badges, avatars, descriptions and
//!   copy buttons;
//! - columns from other tables (`Column::related`, `Column::count_of`), an
//!   advanced filter (`advanced_filter`), remembered filters (`remember`) and
//!   polling every 30 seconds (`poll`).
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
            // Customers in tiers, and a few notes per order, for the grid's
            // relationship columns.
            for i in 0..60 {
                let tier = ["gold", "silver", "bronze", "bronze"][i % 4];
                renox::db::sql("INSERT INTO customers (name, tier) VALUES (?, ?)")
                    .bind(format!("Customer {i}"))
                    .bind(tier)
                    .execute(&db)
                    .await?;
            }
            renox::db::sql("UPDATE orders SET customer_id = (id % 60) + 1")
                .execute(&db)
                .await?;
            for order in 1..=480_i64 {
                for n in 0..(order * 7 % 4) {
                    renox::db::sql("INSERT INTO order_notes (order_id, body) VALUES (?, ?)")
                        .bind(order)
                        .bind(format!("Note {n}"))
                        .execute(&db)
                        .await?;
                }
            }
            Ok(())
        })
}
