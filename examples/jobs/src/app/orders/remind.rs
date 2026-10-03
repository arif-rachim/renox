use renox::mail::Mail;
use renox::prelude::*;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use super::{Order, OrderStatus};

/// A staff member asks the customer to pay.
pub(super) async fn remind(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<i64>,
) -> Result<Redirect> {
    Order::find_or_404(&state.db, id).await?;
    // Pressed twice (or by two people): still one reminder, see UNIQUE_FOR.
    state.dispatch(RemindUnpaid { order_id: id }).await?;
    session.flash("status", format!("Order #{id}: reminder queued."))?;
    Ok(Redirect::to("/"))
}

/// Mails the customer about an unpaid order.
#[derive(Serialize, Deserialize)]
pub struct RemindUnpaid {
    pub order_id: i64,
}

impl Job for RemindUnpaid {
    const NAME: &'static str = "remind-unpaid";
    // While one reminder for an order is queued or running, dispatching
    // another returns the queued one's id instead of queueing a second
    // (for at most an hour; the claim ends when the job finishes).
    const UNIQUE_FOR: Option<Duration> = Some(Duration::from_secs(3600));

    /// "The same job" means the same order.
    fn unique_id(&self) -> String {
        self.order_id.to_string()
    }

    async fn handle(self, ctx: JobContext) -> Result {
        let order = Order::find_or_404(&ctx.state.db, self.order_id).await?;
        if order.status != OrderStatus::Unpaid {
            return Ok(()); // paid while the reminder waited
        }
        let mail = Mail::new(
            &order.customer_email,
            format!("Order #{} is waiting for payment", order.id),
            format!(
                "Your {} ({}) is ready to ship once paid.",
                order.item,
                super::money(&ctx.state, order.total)
            ),
        );
        ctx.state.mailer.send(mail).await
    }
}
