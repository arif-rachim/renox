//! Made with `rnx make:module orders`, `rnx make:model Order --module orders -m`,
//! `rnx make:job SendReceipt --module orders` and `rnx make:mail receipt`.

mod new_order;
mod receipt;

use renox::prelude::*;
use serde::{Deserialize, Serialize};

pub use receipt::SendReceipt;

#[derive(Model, Serialize, Deserialize, Default, Debug, Clone)]
#[model(table = "orders")]
pub struct Order {
    pub id: i64,
    pub customer_email: String,
    pub item: String,
    /// In rupiah.
    pub total: i64,
    pub created_at: Option<DateTime>,
    pub updated_at: Option<DateTime>,
}

/// Emitted when an order is placed; listeners decide what follows.
#[derive(Clone)]
pub struct OrderPlaced {
    pub order_id: i64,
}

impl Event for OrderPlaced {}

pub struct Orders;

impl Module for Orders {
    fn name(&self) -> &'static str {
        "orders"
    }

    fn routes(&self) -> Routes {
        Routes::new()
            .get("/", index)
            .name("home")
            .post("/orders", store)
            .name("orders.store")
    }

    fn register(&self, app: &mut Registry) {
        app.job::<SendReceipt>()
            // Listeners run right away, so they only queue the slow parts.
            .listen(|event: OrderPlaced, state| async move {
                state
                    .dispatch(SendReceipt {
                        order_id: event.order_id,
                    })
                    .await?;
                let order = Order::find_or_404(&state.db, event.order_id).await?;
                for admin in User::all(&state.db).await? {
                    state
                        .notify(&admin, &new_order::NewOrder(order.clone()))
                        .await?;
                }
                Ok(())
            });
        app.schedule().daily_at("21:00", "daily-sales", daily_sales);
    }
}

#[derive(Deserialize)]
struct OrderForm {
    customer_email: String,
    item: String,
    total: i64,
}

impl Validate for OrderForm {
    fn rules(&self, v: &mut Validator) {
        v.field("customer_email", &self.customer_email)
            .required()
            .email();
        v.field("item", &self.item).required().max(100);
        v.field("total", &self.total).min(1);
    }
}

async fn index(State(db): State<Db>) -> Result<View> {
    let orders = Order::query().latest().limit(20).get(&db).await?;
    Ok(view("orders/index.html", context! { orders }))
}

async fn store(
    State(state): State<AppState>,
    session: Session,
    Valid(form): Valid<OrderForm>,
) -> Result<Redirect> {
    let order = Order {
        customer_email: form.customer_email,
        item: form.item,
        total: form.total,
        ..Default::default()
    };
    let order = Order::create(&state.db, order).await?;
    state.emit(OrderPlaced { order_id: order.id }).await?;
    session.flash("status", "Thanks! Your receipt is on its way.")?;
    Ok(Redirect::to("/"))
}

/// Mails the day's order count and total to every admin (scheduled at 21:00
/// in APP_TIMEZONE; `my-app schedule:list` shows when it runs next).
pub async fn daily_sales(state: AppState) -> Result {
    let since = renox::db::now() - renox::chrono::TimeDelta::days(1);
    let orders = Order::query()
        .where_op("created_at", ">=", since)
        .get(&state.db)
        .await?;
    let total: i64 = orders.iter().map(|o| o.total).sum();
    for admin in User::all(&state.db).await? {
        let mail = state.mail_view(
            &admin.email,
            "Today's sales",
            "mail/daily-sales",
            context! { count => orders.len(), total },
        )?;
        state.queue_mail(mail).await?;
    }
    Ok(())
}
