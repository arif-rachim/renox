//! The sales dashboard: data grids over the orders, filled from the server
//! a page at a time. Made with `rnx make:module orders` and
//! `rnx make:model Order --module orders -m`; the grids are `renox::grid`
//! (see `orders_grid` and `regions_grid`).

pub mod model;

use renox::Toast;
use renox::chrono::NaiveDate;
use renox::grid::{Column, Grid, GridRequest, RowOrder};
use renox::prelude::*;
use serde::Deserialize;

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
            .get("/regions", regions)
            .name("orders.regions")
            .get("/orders/{id}", show)
            .name("orders.show")
            .patch("/orders/{id}", update)
            .name("orders.update")
            .post("/orders/reorder", reorder)
            .name("orders.reorder")
    }
}

/// The orders grid: which columns, how they group and filter, which show
/// on a phone (`mobile`), which stay put while scrolling (`frozen`), which
/// can be edited in place, and the details under a row (`audit`).
pub fn orders_grid() -> Grid {
    Grid::new("orders")
        .title("Orders")
        .column(Column::number("position", "#").hidden())
        // `searchable`: the toolbar's search box looks in these.
        .column(
            Column::text("number", "Order")
                .frozen()
                .mobile()
                .searchable(),
        )
        .column(
            Column::text("customer", "Name")
                .under(["Customer"])
                .mobile()
                .editable()
                .searchable(),
        )
        .column(
            Column::text("email", "Email")
                .under(["Customer"])
                .hidden()
                .searchable(),
        )
        .column(Column::select("region", "Region", REGIONS).under(["Location"]))
        .column(
            Column::text("city", "City")
                .under(["Location"])
                .searchable(),
        )
        .column(
            Column::select("status", "Status", STATUSES)
                .mobile()
                .editable(),
        )
        .column(Column::tags("tags", "Tags", TAGS).editable())
        .column(
            Column::number("items", "Items")
                .under(["Amounts"])
                .editable(),
        )
        .column(
            Column::money("total", "Total (Rp)")
                .under(["Amounts"])
                .editable(),
        )
        .column(
            Column::number("discount", "Discount %")
                .decimals(1)
                .under(["Amounts"])
                .editable(),
        )
        .column(Column::date("ordered_on", "Ordered").editable())
        .column(Column::bool("paid", "Paid").editable())
        .column(Column::custom("trend", "Last 7 days").under(["Charts"]))
        .column(
            Column::custom("fulfilled", "Shipped")
                .under(["Charts"])
                .width("9rem"),
        )
        .column(Column::custom("actions", "Actions").frozen_right())
        .sort_by("-ordered_on")
        // The chevron opens the audit details; the link icon opens the order.
        .row_url("/orders/{id}")
        .empty_state(
            "No orders yet",
            Some("Orders show up here as customers buy."),
        )
        .audit()
        // CSV, Excel and a print page of every filtered row (`export` below).
        .exports()
        .edit_url("/orders/{id}")
        // Sorted by # (ascending), rows can be dragged into order.
        .reorder("position", "/orders/reorder")
}

/// The same orders by region and city: equal neighbours share one cell
/// (`merge`), cities nested in their region.
pub fn regions_grid() -> Grid {
    Grid::new("regions")
        .title("Orders by region")
        .column(
            Column::select("region", "Region", REGIONS)
                .merge()
                .frozen()
                .mobile(),
        )
        .column(Column::text("city", "City").merge().frozen().mobile())
        .column(Column::text("number", "Order").mobile().searchable())
        .column(Column::text("customer", "Customer").searchable())
        .column(Column::select("status", "Status", STATUSES))
        .column(Column::money("total", "Total (Rp)"))
        .column(Column::date("ordered_on", "Ordered"))
        .sort_by("region,city,-total")
        .row_url("/orders/{id}")
        .audit()
        .details()
        .exports()
}

async fn index(request: GridRequest) -> Result<Response> {
    // `?export=csv|xlsx|print`: the file, not the page.
    if let Some(file) = orders_grid().export(Order::query(), &request).await? {
        return Ok(file);
    }
    let page = orders_grid()
        .page(Order::query(), &request)
        .await?
        // Values the template's custom cells read: where the trend went.
        .extend(|order| {
            let trend = &order.trend.0;
            let up = trend.last() >= trend.first();
            json!({ "up": up })
        });
    Ok(view("orders/index.html", context! { orders => page }).into_response())
}

async fn regions(request: GridRequest) -> Result<Response> {
    if let Some(file) = regions_grid().export(Order::query(), &request).await? {
        return Ok(file);
    }
    let page = regions_grid().page(Order::query(), &request).await?;
    Ok(view("orders/regions.html", context! { orders => page }).into_response())
}

async fn show(State(state): State<AppState>, Path(id): Path<i64>) -> Result<View> {
    let order = Order::find_or_404(&state.db, id).await?;
    Ok(view("orders/show.html", context! { order }))
}

/// What the grid sends when cells are edited: only the changed fields.
#[derive(Deserialize, Validate)]
pub struct OrderEdit {
    #[validate(max = 100)]
    pub customer: Option<String>,
    #[validate(one_of(&["new", "paid", "shipped", "cancelled"]))]
    pub status: Option<String>,
    /// Unknown tags are dropped (`update`).
    pub tags: Option<Vec<String>>,
    #[validate(between(1, 999))]
    pub items: Option<i64>,
    #[validate(min = 0)]
    pub total: Option<i64>,
    #[validate(between(0, 100))]
    pub discount: Option<f64>,
    pub ordered_on: Option<NaiveDate>,
    pub paid: Option<bool>,
}

async fn update(
    State(state): State<AppState>,
    user: Option<AuthUser>,
    Path(id): Path<i64>,
    Valid(edit): Valid<OrderEdit>,
) -> Result<Toast> {
    let mut order = Order::find_or_404(&state.db, id).await?;
    if let Some(v) = edit.customer {
        order.customer = v;
    }
    if let Some(v) = edit.status {
        order.status = v;
    }
    if let Some(mut v) = edit.tags {
        v.retain(|tag| TAGS.iter().any(|(known, _)| known == tag));
        order.tags.0 = v;
    }
    if let Some(v) = edit.items {
        order.items = v;
    }
    if let Some(v) = edit.total {
        order.total = v;
    }
    if let Some(v) = edit.discount {
        order.discount = v;
    }
    if let Some(v) = edit.ordered_on {
        order.ordered_on = v;
    }
    if let Some(v) = edit.paid {
        order.paid = v;
    }
    order.updated_by = user.map_or_else(|| "Guest".to_owned(), |u| u.name.clone());
    order.save(&state.db).await?;
    Ok(Toast::success(format!("{} saved.", order.number)))
}

async fn reorder(State(state): State<AppState>, Form(order): Form<RowOrder>) -> Result<StatusCode> {
    order.save::<Order>(&state.db, "position").await?;
    Ok(StatusCode::NO_CONTENT)
}
