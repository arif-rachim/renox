//! Made with `rnx make:module payments` and `rnx make:migration create_orders_table`.
//! The `Order` model is written here in the module (its table is `orders`,
//! named with `#[model(table = …)]`), and each provider is one `impl Webhook`
//! in a file of its own (there is no generator for webhooks).

mod midtrans;
mod stripe;
mod xendit;

use renox::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Model, Serialize, Deserialize, Default, Debug, Clone)]
#[model(table = "orders")]
pub struct Order {
    pub id: i64,
    pub code: String,
    /// In rupiah.
    pub amount: i64,
    /// `pending` or `paid`.
    pub status: String,
    pub paid_via: Option<String>,
    pub created_at: Option<DateTime>,
    pub updated_at: Option<DateTime>,
}

impl Order {
    pub fn new(code: &str, amount: i64) -> Self {
        Order {
            code: code.into(),
            amount,
            status: "pending".into(),
            ..Default::default()
        }
    }
}

/// Marks the order paid. Idempotent, like everything a webhook triggers.
async fn mark_paid(db: &Db, code: &str, via: &str) -> Result {
    let Some(mut order) = Order::where_eq("code", code).first(db).await? else {
        tracing::warn!(code, via, "payment for an unknown order");
        return Ok(());
    };
    if order.status != "paid" {
        order.status = "paid".into();
        order.paid_via = Some(via.into());
        order.save(db).await?;
    }
    Ok(())
}

pub struct Payments;

impl Module for Payments {
    fn name(&self) -> &'static str {
        "payments"
    }

    fn routes(&self) -> Routes {
        Routes::new()
            .get("/", index)
            .name("home")
            .webhook::<midtrans::Midtrans>("/webhooks/midtrans")
            .webhook::<xendit::Xendit>("/webhooks/xendit")
            .webhook::<stripe::Stripe>("/webhooks/stripe")
    }

    fn register(&self, app: &mut Registry) {
        app.webhook::<midtrans::Midtrans>()
            .webhook::<xendit::Xendit>()
            .webhook::<stripe::Stripe>();
    }
}

async fn index(State(db): State<Db>) -> Result<View> {
    let orders = Order::query().order_by("id").get(&db).await?;
    Ok(view("orders/index.html", context! { orders }))
}
