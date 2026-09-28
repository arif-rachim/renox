//! The back office under `/admin`: products (with photos) and orders. The
//! group ends with `.require_role("admin")` (the `Permissions` module): a
//! customer gets 403 and a guest the login page. Order status changes are
//! written to the audit log, and the dashboard shows the latest entries.
//!
//! Made with `rnx make:module admin`.

mod orders;
mod products;

use renox::audit;
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
                .delete("/products/{id}", products::destroy)
                .name("products.destroy")
                .get("/orders", orders::index)
                .name("orders.index")
                .put("/orders/{id}/status", orders::update_status)
                .name("orders.status")
                .require_role(crate::ADMIN),
        )
    }
}

async fn dashboard(State(db): State<Db>, user: AuthUser) -> Result<View> {
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
        context! { pending, low_stock, notifications, activity },
    ))
}
