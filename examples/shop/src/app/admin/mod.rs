//! The back office under `/admin`: products (with photos) and orders. The
//! group ends with `.require_role("admin")` (the `Permissions` module): a
//! customer gets 403 and a guest the login page. Order status changes are
//! written to the audit log, and the dashboard shows the latest entries.
//! Deleting a product asks for the password again
//! (`.require_password_confirmed()`, the `Auth` module's `/confirm-password`).
//! The dashboard's figures and charts come from `renox::chart` (`Trend` over
//! the `Period` in `?period=`); the orders-by-status chart loads on its own
//! and refreshes every minute (the kit's `widget(url=…, poll=60)`).
//!
//! Made with `rnx make:module admin`.

mod categories;
mod orders;
mod products;

use renox::audit;
use renox::chart::{Period, Series, Trend};
use renox::prelude::*;

use crate::app::catalog::model::Product;
use crate::app::orders::model::{Order, OrderStatus};

pub struct AdminPanel;

impl Module for AdminPanel {
    fn name(&self) -> &'static str {
        "admin"
    }

    fn routes(&self) -> Routes {
        Routes::new().group(
            "/admin",
            "admin.",
            Routes::new()
                .get("/", dashboard)
                .name("dashboard")
                .get("/widgets/statuses", statuses)
                .name("widgets.statuses")
                .get("/products", products::index)
                .name("products.index")
                .get("/products/new", products::create)
                .name("products.create")
                .post("/products", products::store)
                .name("products.store")
                .get("/products/{id}/edit", products::edit)
                .name("products.edit")
                .put("/products/{id}", products::update)
                .name("products.update")
                .put("/products/{id}/stock", products::adjust_stock)
                .name("products.stock")
                // The category select's options: searched, added, renamed.
                .get("/categories/options", categories::options)
                .name("categories.options")
                .post("/categories/options", categories::create_option)
                .put("/categories/options", categories::update_option)
                .get("/orders", orders::index)
                .name("orders.index")
                .put("/orders/{id}/status", orders::update_status)
                .name("orders.status")
                // Deleting a product can't be undone (and removes its
                // photo): ask for the password first, if it wasn't typed in
                // the last three hours. A guard covers only the routes added
                // before it, so this route is its own group, merged in.
                .merge(
                    Routes::new()
                        .delete("/products/{id}", products::destroy)
                        .name("products.destroy")
                        .require_password_confirmed(),
                )
                .require_role(crate::ADMIN),
        )
    }
}

/// Orders that brought money in: paid or shipped.
fn sold() -> renox::db::Query<Order> {
    Order::query().where_in("status", [OrderStatus::Paid, OrderStatus::Shipped])
}

async fn dashboard(State(state): State<AppState>, user: AuthUser, period: Period) -> Result<View> {
    let db = state.db.clone();
    // This period, and the one before it for the deltas.
    let revenue = Trend::of(sold(), "created_at")
        .over(period)
        .sum(&state, "total")
        .await?;
    let revenue_before = Trend::of(sold(), "created_at")
        .over(period.previous())
        .sum(&state, "total")
        .await?;
    let orders = Trend::of(sold(), "created_at")
        .over(period)
        .count(&state)
        .await?;
    let orders_before = Trend::of(sold(), "created_at")
        .over(period.previous())
        .count(&state)
        .await?;
    let customers = Trend::of(User::query(), "created_at")
        .over(period)
        .count(&state)
        .await?;
    let customers_before = Trend::of(User::query(), "created_at")
        .over(period.previous())
        .count(&state)
        .await?;
    let average = if orders.total() > 0.0 {
        revenue.total() / orders.total()
    } else {
        0.0
    };
    let pending = Order::where_eq("status", OrderStatus::Pending)
        .count(&db)
        .await?;
    let low_stock = Product::query()
        .where_op("stock", "<", 5)
        .where_eq("active", true)
        .order_by("stock")
        .limit(10)
        .get(&db)
        .await?;
    let notifications = user.notifications(&db, 10).await?;
    user.mark_all_notifications_read(&db).await?;
    // Who did what lately: logins (recorded by the `Audit` module itself)
    // and order changes (recorded in `orders::update_status`).
    let activity = audit::latest(&db, 10).await?;
    Ok(view(
        "admin/dashboard.html",
        context! {
            period,
            pending,
            low_stock,
            notifications,
            activity,
            revenue_total => revenue.total(),
            revenue_change => revenue.change_from(&revenue_before),
            orders_total => orders.total(),
            orders_change => orders.change_from(&orders_before),
            customers_total => customers.total(),
            customers_change => customers.change_from(&customers_before),
            average,
            // Both periods on one axis, by day (or month) of the period.
            revenue_series => [
                revenue.clone().named("This period"),
                Series::new(revenue.labels.clone(), revenue_before.values).named("The period before"),
            ],
            revenue_labels => revenue.labels.clone(),
            revenue,
            orders,
        },
    ))
}

/// The orders-by-status widget, loaded after the page and every minute.
async fn statuses(State(db): State<Db>) -> Result<View> {
    let counts: Vec<(String, i64)> = Order::query()
        .group_by("status")
        .select_as(&db, "status, COUNT(*)")
        .await?;
    let order = ["pending", "paid", "shipped", "cancelled"];
    let mut counts = counts;
    counts.sort_by_key(|(status, _)| order.iter().position(|s| s == status));
    let labels: Vec<String> = counts.iter().map(|(status, _)| status.clone()).collect();
    let values: Vec<i64> = counts.iter().map(|(_, n)| *n).collect();
    Ok(view("admin/_statuses.html", context! { labels, values }))
}
