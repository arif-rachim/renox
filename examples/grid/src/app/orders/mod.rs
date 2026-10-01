//! The sales dashboard: one data grid over the orders, filled from the
//! server a page at a time. Made with `rnx make:module orders` and
//! `rnx make:model Order --module orders -m`; the grid is
//! `renox::grid` (see `orders_grid`).

pub mod model;

use renox::grid::{Column, Grid, GridRequest};
use renox::prelude::*;

pub use model::Order;
use model::{REGIONS, STATUSES, TAGS};

pub struct Orders;

impl Module for Orders {
    fn name(&self) -> &'static str {
        "orders"
    }

    fn routes(&self) -> Routes {
        Routes::new()
            .get("/", index)
            .name("orders.index")
            .get("/orders/{id}", show)
            .name("orders.show")
    }
}

/// The grid: which columns, how they group and filter, which show on a
/// phone (`mobile`) and which stay put while scrolling (`frozen`).
pub fn orders_grid() -> Grid {
    Grid::new("orders")
        .title("Orders")
        .column(Column::text("number", "Order").frozen().mobile())
        .column(
            Column::text("customer", "Name")
                .under(["Customer"])
                .mobile(),
        )
        .column(Column::text("email", "Email").under(["Customer"]).hidden())
        .column(Column::select("region", "Region", REGIONS).under(["Location"]))
        .column(Column::text("city", "City").under(["Location"]))
        .column(Column::select("status", "Status", STATUSES).mobile())
        .column(Column::tags("tags", "Tags", TAGS))
        .column(Column::number("items", "Items").under(["Amounts"]))
        .column(Column::money("total", "Total (Rp)").under(["Amounts"]))
        .column(
            Column::number("discount", "Discount %")
                .decimals(1)
                .under(["Amounts"]),
        )
        .column(Column::date("ordered_on", "Ordered"))
        .column(Column::bool("paid", "Paid"))
        .column(Column::custom("trend", "Last 7 days").under(["Charts"]))
        .column(
            Column::custom("fulfilled", "Shipped")
                .under(["Charts"])
                .width("9rem"),
        )
        .column(Column::custom("actions", "Actions").frozen_right())
        .sort_by("-ordered_on")
}

async fn index(request: GridRequest) -> Result<View> {
    let page = orders_grid()
        .page(Order::query(), &request)
        .await?
        // Values the template's custom cells read: where the trend went.
        .extend(|order| {
            let trend = &order.trend.0;
            let up = trend.last() >= trend.first();
            json!({ "up": up })
        });
    Ok(view("orders/index.html", context! { orders => page }))
}

async fn show(State(state): State<AppState>, Path(id): Path<i64>) -> Result<View> {
    let order = Order::find_or_404(&state.db, id).await?;
    Ok(view("orders/show.html", context! { order }))
}
