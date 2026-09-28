//! Checkout and the customer's orders. Placing an order emits `OrderPlaced`;
//! its listener queues the confirmation mail and tells the admins. A daily
//! task cancels orders nobody paid for.

pub mod checkout;
pub mod model;
pub mod notifications;

use renox::prelude::*;
use serde::{Deserialize, Serialize};

use checkout::Checkout;
use model::{Order, OrderStatus};
use notifications::{NewOrder, OrderConfirmation};

/// Emitted after an order is committed.
#[derive(Clone)]
pub struct OrderPlaced {
    pub order_id: i64,
}

impl Event for OrderPlaced {}

/// Pending orders older than this are cancelled by the daily task.
pub const PAY_WITHIN_DAYS: i64 = 3;

pub struct Orders;

impl Module for Orders {
    fn name(&self) -> &'static str {
        "orders"
    }

    fn routes(&self) -> Routes {
        Routes::new()
            .get("/checkout", checkout_form)
            .name("checkout")
            .post("/checkout", place)
            .name("checkout.place")
            .get("/orders", index)
            .name("orders.index")
            .get("/orders/{id}", show)
            .name("orders.show")
            .require_auth()
    }

    fn register(&self, app: &mut Registry) {
        app.listen(|event: OrderPlaced, state| async move {
            let order = Order::find_or_404(&state.db, event.order_id).await?;
            let items = order.items(&state.db).await?;
            let customer = User::find_or_404(&state.db, order.user_id).await?;
            // Mail goes through the queue, so a slow mail server doesn't
            // slow down checkout.
            state
                .notify_later(
                    &customer,
                    &OrderConfirmation {
                        order: order.clone(),
                        items,
                    },
                )
                .await?;
            for admin in crate::admins(&state.db).await? {
                state.notify(&admin, &NewOrder(order.clone())).await?;
            }
            Ok(())
        });
        app.schedule()
            .daily_at("03:00", "cancel-unpaid-orders", |state| async move {
                let cancelled = cancel_unpaid(&state).await?;
                tracing::info!(cancelled, "unpaid orders cancelled");
                Ok(())
            });
    }
}

/// Cancels pending orders older than `PAY_WITHIN_DAYS` and returns their
/// stock. The daily task; public so tests can run it.
pub async fn cancel_unpaid(state: &AppState) -> Result<usize> {
    let cutoff = renox::db::now() - renox::chrono::TimeDelta::days(PAY_WITHIN_DAYS);
    let stale = Order::where_eq("status", OrderStatus::Pending)
        .where_op("created_at", "<", cutoff)
        .get(&state.db)
        .await?;
    let mut cancelled = 0;
    for order in stale {
        if checkout::cancel(&state.db, &order).await? {
            cancelled += 1;
        }
    }
    Ok(cancelled)
}

async fn checkout_form(State(db): State<Db>, user: AuthUser) -> Result<Response> {
    let lines = crate::app::cart::lines(&db, user.id).await?;
    if lines.is_empty() {
        return Ok(Redirect::to("/cart").into_response());
    }
    let total: i64 = lines.iter().map(|l| l.subtotal).sum();
    Ok(view("orders/checkout.html", context! { lines, total }).into_response())
}

#[derive(Deserialize, Serialize)]
struct CheckoutForm {
    address: String,
}

impl Validate for CheckoutForm {
    fn rules(&self, v: &mut Validator) {
        v.field("address", &self.address)
            .required()
            .between(10, 500);
    }
}

async fn place(
    State(state): State<AppState>,
    user: AuthUser,
    session: Session,
    lang: Lang,
    Valid(form): Valid<CheckoutForm>,
) -> Result<Redirect> {
    match checkout::place(&state.db, user.id, form.address.trim()).await? {
        Checkout::Placed(order) => {
            state.emit(OrderPlaced { order_id: order.id }).await?;
            session.flash("status", lang.t("orders.placed", &[("id", &order.id)]))?;
            Ok(Redirect::to(&format!("/orders/{}", order.id)))
        }
        Checkout::EmptyCart => Ok(Redirect::to("/cart")),
        Checkout::OutOfStock(names) => {
            let names = names.join(", ");
            session.flash("error", lang.t("orders.out_of_stock", &[("names", &names)]))?;
            Ok(Redirect::to("/cart"))
        }
    }
}

async fn index(State(db): State<Db>, user: AuthUser, Page(page): Page) -> Result<View> {
    let orders = Order::where_eq("user_id", user.id)
        .latest()
        .paginate(&db, page, 10)
        .await?;
    Ok(view("orders/index.html", context! { orders }))
}

async fn show(State(db): State<Db>, user: AuthUser, Path(id): Path<i64>) -> Result<View> {
    let order = Order::find_or_404(&db, id).await?;
    user.authorize("view", &order)?;
    let items = order.items(&db).await?;
    Ok(view("orders/show.html", context! { order, items }))
}
