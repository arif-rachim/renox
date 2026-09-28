//! The back office under `/admin`: products (with photos) and orders. The
//! group ends with `.require_role("admin")` (the `Permissions` module): a
//! customer gets 403 and a guest the login page. Order status changes are
//! written to the audit log, and the dashboard shows the latest entries.
//! Deleting a product asks for the password again
//! (`.require_password_confirmed()`, the `Auth` module's `/confirm-password`).
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
