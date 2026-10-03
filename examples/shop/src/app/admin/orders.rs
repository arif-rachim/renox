//! The admin's orders: a list by status, the order page with its audit
//! trail, and status changes (pay, ship, cancel) that are audited and tell
//! the customer.

use renox::Toast;
use renox::audit::{self, Entry};
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
/// comes back). Shipping tells the customer. Every change is written to the
/// audit log: who, which order, from and to, and from which address.
pub async fn update_status(
    State(state): State<AppState>,
    admin: AuthUser,
    ClientIp(ip): ClientIp,
    back: Back,
    Path(id): Path<i64>,
    Valid(form): Valid<StatusForm>,
) -> Result<(Toast, Back)> {
    use OrderStatus::*;
    let mut order = Order::find_or_404(&state.db, id).await?;
    let from = order.status;
    match (from, form.status) {
        (Pending, Cancelled) => {
            // `false`: the customer paid or cancelled it meanwhile; nothing
            // changed, so nothing is audited.
            if !checkout::cancel(&state.db, &order).await? {
                return Err(abort(
                    StatusCode::CONFLICT,
                    format!("Order #{} changed meanwhile; reload the page.", order.id),
                ));
            }
        }
        (Pending, Paid) | (Paid, Shipped) => {
            order.status = form.status;
            // Only the status: a checkout running at the same time can't
            // have its columns overwritten by this (older) copy.
            order.save_only(&state.db, &["status"]).await?;
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
    audit::record(
        &state.db,
        Entry::new("order.status_changed")
            .user(admin.id)
            .subject("orders", order.id)
            .data(json!({ "from": from, "to": form.status }))
            .ip(ip),
    )
    .await?;
    Ok((
        Toast::success(format!("Order #{} updated.", order.id)),
        back,
    ))
}
