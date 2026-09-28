use renox::mail::Mail;
use renox::prelude::*;
use renox::queue::Middleware;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use super::{Order, OrderStatus, SendReceipt};

#[derive(Deserialize)]
pub(super) struct PayForm {
    card_token: String,
}

impl Validate for PayForm {
    fn rules(&self, v: &mut Validator) {
        v.field("card_token", &self.card_token).required().max(100);
    }
}

/// The customer pays: the card is charged in the background, then the
/// receipt goes out, then the warehouse hears about it, in that order.
pub(super) async fn pay(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<i64>,
    Valid(form): Valid<PayForm>,
) -> Result<Redirect> {
    Order::find_or_404(&state.db, id).await?;
    // Only an unpaid order moves on, even when the button is pressed twice.
    let claimed = Order::where_eq("id", id)
        .where_eq("status", OrderStatus::Unpaid)
        .update(&state.db, &[("status", &OrderStatus::Processing)])
        .await?;
    abort_if(
        claimed == 0,
        StatusCode::CONFLICT,
        "This order is already being paid.",
    )?;
    // A chain: each job is queued when the one before it succeeds, so no
    // receipt goes out for a card that was declined.
    state
        .queue
        .chain()
        .then(ChargePayment {
            order_id: id,
            card_token: form.card_token,
        })
        .then(SendReceipt { order_id: id })
        .then(NotifyWarehouse { order_id: id })
        .dispatch()
        .await?;
    session.flash("status", "Thanks! We're charging your card.")?;
    Ok(Redirect::to("/"))
}

/// Charges the card through the payment gateway.
#[derive(Serialize, Deserialize)]
pub struct ChargePayment {
    pub order_id: i64,
    pub card_token: String,
}

impl Job for ChargePayment {
    const NAME: &'static str = "charge-payment";
    // The customer is waiting on this one too.
    const QUEUE: &'static str = "high";
    const MAX_ATTEMPTS: u32 = 3;
    // The card token is as good as the card: the payload is sealed with
    // APP_KEY in `jobs` (and `failed_jobs`), and opened by the worker.
    const ENCRYPTED: bool = true;

    fn middleware(&self) -> Vec<Middleware> {
        vec![
            // The gateway allows 100 calls a minute; jobs over that wait for
            // the next minute without using up an attempt.
            Middleware::rate_limited("payment-gateway", 100, Duration::from_secs(60)),
            // Never two charges for one order at once, whoever queued them
            // (`pay`, a `queue:retry`, a script).
            Middleware::without_overlapping(format!("charge:{}", self.order_id)),
        ]
    }

    async fn handle(self, ctx: JobContext) -> Result {
        let db = &ctx.state.db;
        let mut order = Order::find_or_404(db, self.order_id).await?;
        if order.status == OrderStatus::Paid {
            return Ok(()); // charged by an earlier attempt that failed afterwards
        }
        gateway::charge(&order, &self.card_token).await?;
        order.status = OrderStatus::Paid;
        order.save_only(db, &["status"]).await
    }

    /// Runs once, after the last attempt (or at once for a declined card):
    /// the chain stops here, so tell the staff.
    async fn failed(self, state: AppState, error: String) {
        if let Err(err) = needs_attention(&state, self.order_id, &error).await {
            eprintln!("could not flag order #{}: {err:?}", self.order_id);
        }
    }
}

async fn needs_attention(state: &AppState, order_id: i64, error: &str) -> Result {
    Order::where_eq("id", order_id)
        .update(&state.db, &[("status", &OrderStatus::NeedsAttention)])
        .await?;
    for admin in User::all(&state.db).await? {
        let mail = Mail::new(
            &admin.email,
            format!("Payment for order #{order_id} failed"),
            format!("The card could not be charged: {error}"),
        );
        state.queue_mail(mail).await?;
    }
    Ok(())
}

/// Tells the warehouse to pack a paid order.
#[derive(Serialize, Deserialize)]
pub struct NotifyWarehouse {
    pub order_id: i64,
}

impl Job for NotifyWarehouse {
    const NAME: &'static str = "notify-warehouse";

    async fn handle(self, ctx: JobContext) -> Result {
        let order = Order::find_or_404(&ctx.state.db, self.order_id).await?;
        let to = ctx
            .state
            .config
            .var("WAREHOUSE_EMAIL")
            .unwrap_or_else(|| "warehouse@example.com".into());
        let mail = Mail::new(
            to,
            format!("Pack order #{}", order.id),
            format!("{} for {}", order.item, order.customer_email),
        );
        ctx.state.mailer.send(mail).await
    }
}

/// Stands in for the payment provider's HTTP API. Like a provider's test
/// mode, two test tokens fail: `tok_declined` (retrying won't help) and
/// `tok_unreachable` (it might).
mod gateway {
    use super::Order;
    use renox::prelude::*;

    #[derive(Debug)]
    pub enum GatewayError {
        Declined,
        Unreachable,
    }

    impl std::fmt::Display for GatewayError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str(match self {
                Self::Declined => "the card was declined",
                Self::Unreachable => "the payment gateway timed out",
            })
        }
    }

    impl std::error::Error for GatewayError {}

    /// Charges `order.total`; a real call sends `order-{id}` as the
    /// idempotency key, so a retry after a lost response can't charge twice.
    pub async fn charge(_order: &Order, card_token: &str) -> Result {
        match card_token {
            // `Error::permanent`: straight to `failed_jobs`, no retries.
            "tok_declined" => Err(Error::permanent(GatewayError::Declined)),
            // A plain error: retried (MAX_ATTEMPTS) with a growing wait.
            "tok_unreachable" => Err(GatewayError::Unreachable.into()),
            _ => Ok(()),
        }
    }
}
