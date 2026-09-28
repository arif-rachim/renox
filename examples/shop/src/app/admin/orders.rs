use renox::prelude::*;
use serde::{Deserialize, Serialize};

use crate::app::orders::checkout;
use crate::app::orders::model::{Order, OrderStatus};
use crate::app::orders::notifications::OrderShipped;

#[derive(Deserialize, Serialize, Default)]
pub struct Filters {
    status: Option<OrderStatus>,
}

pub async fn index(
    State(db): State<Db>,
    Query(filters): Query<Filters>,
    Page(page): Page,
) -> Result<View> {
    let orders = Order::query()
        .when(filters.status.is_some(), |q| {
            q.where_eq("status", filters.status)
        })
        .latest()
        .paginate(&db, page, 20)
        .await?;
    let statuses = OrderStatus::ALL;
    Ok(view(
        "admin/orders/index.html",
        context! { orders, filters, statuses },
    ))
}

#[derive(Deserialize)]
pub struct StatusForm {
    status: OrderStatus,
}

impl Validate for StatusForm {
    fn rules(&self, _: &mut Validator) {}
}

/// Pending → paid → shipped; pending orders can be cancelled (their stock
/// comes back). Shipping tells the customer.
pub async fn update_status(
    State(state): State<AppState>,
    session: Session,
    back: Back,
    Path(id): Path<i64>,
    Valid(form): Valid<StatusForm>,
) -> Result<Back> {
    use OrderStatus::*;
    let mut order = Order::find_or_404(&state.db, id).await?;
    match (order.status, form.status) {
        (Pending, Cancelled) => {
            checkout::cancel(&state.db, &order).await?;
        }
        (Pending, Paid) | (Paid, Shipped) => {
            order.status = form.status;
            order.save(&state.db).await?;
            if form.status == Shipped {
                let customer = User::find_or_404(&state.db, order.user_id).await?;
                state
                    .notify_later(&customer, &OrderShipped(order.clone()))
                    .await?;
            }
        }
        (from, to) => {
            return Err(abort(
                StatusCode::CONFLICT,
                format!("An order can't go from {from} to {to}."),
            ));
        }
    }
    session.flash("status", format!("Order #{} updated.", order.id))?;
    Ok(back)
}
