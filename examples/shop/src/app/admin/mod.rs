//! The back office under `/admin`: products (with photos) and orders. The
//! group is guarded by the `admin` gate: a customer gets 403 and a guest the
//! login page.

mod orders;
mod products;

use renox::prelude::*;

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
                .require_gate("admin"),
        )
    }
}

async fn dashboard(State(db): State<Db>, user: AuthUser) -> Result<View> {
    let pending: i64 = renox::db::sql("SELECT COUNT(*) FROM orders WHERE status = 'pending'")
        .scalar(&db)
        .await?;
    let low_stock = crate::app::catalog::model::Product::query()
        .where_op("stock", "<", 5)
        .where_eq("active", true)
        .order_by("stock")
        .limit(10)
        .get(&db)
        .await?;
    let notifications = user.notifications(&db, 10).await?;
    user.mark_all_notifications_read(&db).await?;
    Ok(view(
        "admin/dashboard.html",
        context! { pending, low_stock, notifications },
    ))
}
