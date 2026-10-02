//! The sales dashboard: data grids over the orders, filled from the server
//! a page at a time. Made with `rnx make:module orders` and
//! `rnx make:model Order --module orders -m`; the grids are `renox::grid`
//! (see `orders_grid` and `regions_grid`).

pub mod model;

use renox::Toast;
use renox::chrono::NaiveDate;
use renox::grid::{Action, Column, Grid, GridRequest, RowOrder, Selection, Summary};
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
            .delete("/orders/{id}", destroy)
            .name("orders.destroy")
            .post("/orders/bulk/status/{status}", bulk_status)
            .name("orders.bulk_status")
            .post("/orders/bulk/delete", bulk_delete)
            .name("orders.bulk_delete")
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
                .searchable()
                .copyable(),
        )
        // Drawn from a value `extend` adds (an SVG with the initials).
        .column(Column::image("avatar", "").round().under(["Customer"]))
        .column(
            Column::text("customer", "Name")
                .under(["Customer"])
                .mobile()
                .editable()
                .searchable()
                // The email, small under the name.
                .description("email"),
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
                .editable()
                .badges(&[
                    ("new", "info"),
                    ("paid", "success"),
                    ("shipped", "neutral"),
                    ("cancelled", "danger"),
                ]),
        )
        .column(Column::tags("tags", "Tags", TAGS).editable())
        .column(
            Column::number("items", "Items")
                .under(["Amounts"])
                .editable()
                .summary(Summary::Sum),
        )
        .column(
            Column::money("total", "Total (Rp)")
                .under(["Amounts"])
                .editable()
                // In the footer (every filtered row) and under each group.
                .summary(Summary::Sum)
                .summary(Summary::Average),
        )
        .column(
            Column::number("discount", "Discount %")
                .decimals(1)
                .under(["Amounts"])
                .editable()
                .summary(Summary::Range),
        )
        .column(Column::date("ordered_on", "Ordered").editable())
        .column(Column::bool("paid", "Paid").editable().icons())
        .column(Column::custom("trend", "Last 7 days").under(["Charts"]))
        .column(
            Column::custom("fulfilled", "Shipped")
                .under(["Charts"])
                .width("9rem"),
        )
        .column(Column::custom("actions", "Actions").frozen_right())
        .sort_by("-ordered_on")
        // A "Group" choice in the toolbar.
        .groups(&["region", "status", "paid"])
        // On phones each order is a card.
        .cards_on_mobile()
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
        // Checkboxes, and these over the selected rows (or all matching).
        .bulk_action(Action::new("Mark paid", "/orders/bulk/status/paid"))
        .bulk_action(Action::new("Mark shipped", "/orders/bulk/status/shipped"))
        .bulk_action(
            Action::new("Delete", "/orders/bulk/delete")
                .confirm("Delete the selected orders? This can't be undone.")
                .danger(),
        )
        // Each row's ⋯ menu.
        .row_action(Action::link("Open", "/orders/{id}"))
        .row_action(
            Action::new("Delete", "/orders/{id}")
                .method("DELETE")
                .confirm("Delete this order?")
                .danger(),
        )
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
            json!({ "up": up, "avatar": avatar(&order.customer) })
        });
    Ok(view("orders/index.html", context! { orders => page }).into_response())
}

/// A small SVG with the customer's initials, as a data URL.
fn avatar(name: &str) -> String {
    let initials: String = name
        .split_whitespace()
        .filter_map(|w| w.chars().next())
        .take(2)
        .collect();
    let hue = name
        .bytes()
        .fold(0u32, |h, b| h.wrapping_mul(31).wrapping_add(u32::from(b)))
        % 360;
    let svg = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 36 36'><rect width='36' height='36' fill='hsl({hue},55%,55%)'/><text x='18' y='23' font-family='sans-serif' font-size='14' fill='white' text-anchor='middle'>{initials}</text></svg>"
    );
    let encoded = svg
        .replace('%', "%25")
        .replace('#', "%23")
        .replace('<', "%3C")
        .replace('>', "%3E")
        .replace(' ', "%20");
    format!("data:image/svg+xml,{encoded}")
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

async fn destroy(State(state): State<AppState>, Path(id): Path<i64>) -> Result<Toast> {
    let mut order = Order::find_or_404(&state.db, id).await?;
    order.delete(&state.db).await?;
    Ok(Toast::success(format!("{} deleted.", order.number)))
}

/// A bulk action: the selected orders, or all the filters match.
async fn bulk_status(
    State(state): State<AppState>,
    user: Option<AuthUser>,
    Path(status): Path<String>,
    request: GridRequest,
    Form(selection): Form<Selection>,
) -> Result<Toast> {
    if !STATUSES.iter().any(|(s, _)| *s == status) {
        return Err(Error::NotFound);
    }
    let by = user.map_or_else(|| "Guest".to_owned(), |u| u.name.clone());
    let changed = orders_grid()
        .selected(Order::query(), &request, &selection)?
        .update(&state.db, &[("status", &status), ("updated_by", &by)])
        .await?;
    Ok(Toast::success(format!("{changed} orders marked {status}.")))
}

async fn bulk_delete(
    State(state): State<AppState>,
    request: GridRequest,
    Form(selection): Form<Selection>,
) -> Result<Toast> {
    let deleted = orders_grid()
        .selected(Order::query(), &request, &selection)?
        .delete(&state.db)
        .await?;
    Ok(Toast::success(format!("{deleted} orders deleted.")))
}

async fn reorder(State(state): State<AppState>, Form(order): Form<RowOrder>) -> Result<StatusCode> {
    order.save::<Order>(&state.db, "position").await?;
    Ok(StatusCode::NO_CONTENT)
}
