use renox::mail::Mail;
use renox::prelude::*;
use renox::queue::Middleware;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use super::{Order, OrderStatus, SendReceipt};

#[derive(Deserialize, Validate)]
pub(super) struct PayForm {
    #[validate(required, max = 100)]
    card_token: String,
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
        gateway::charge(&ctx.state, &order, &self.card_token).await?;
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

/// The payment provider's HTTP API, called with `state.http`: a timeout,
/// an idempotency key, and the answer turned into "retry" or "give up".
/// `PAYMENT_GATEWAY_URL` points at the provider; unset, it's this app's own
/// sandbox (below), so `cargo run` works without an account.
mod gateway {
    use super::Order;
    use renox::prelude::*;
    use std::time::Duration;

    #[derive(Debug)]
    pub enum GatewayError {
        Declined,
        Unavailable(StatusCode),
    }

    impl std::fmt::Display for GatewayError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::Declined => f.write_str("the card was declined"),
                Self::Unavailable(status) => write!(f, "the payment gateway answered {status}"),
            }
        }
    }

    impl std::error::Error for GatewayError {}

    pub fn base_url(state: &AppState) -> String {
        state.config.var("PAYMENT_GATEWAY_URL").unwrap_or_else(|| {
            format!("{}/sandbox/gateway", state.config.url.trim_end_matches('/'))
        })
    }

    /// Charges `order.total`. `order-{id}` is the idempotency key, so a
    /// retry after a lost response can't charge twice.
    pub async fn charge(state: &AppState, order: &Order, card_token: &str) -> Result {
        let secret = state
            .config
            .var("PAYMENT_GATEWAY_KEY")
            .unwrap_or_else(|| "sk_test_sandbox".into());
        let response = state
            .http
            .post(format!("{}/charges", base_url(state)))
            .basic_auth(&secret, "") // as Stripe does
            .header("idempotency-key", format!("order-{}", order.id))
            .json(&json!({ "amount": order.total, "currency": "idr", "source": card_token }))
            .timeout(Duration::from_secs(15))
            .send()
            .await?; // no answer at all: a plain error, retried (MAX_ATTEMPTS)
        match response.status() {
            status if status.is_success() => Ok(()),
            // `Error::permanent`: straight to `failed_jobs`, no retries.
            StatusCode::PAYMENT_REQUIRED => Err(Error::permanent(GatewayError::Declined)),
            // A 5xx or 429 might pass later: retried with a growing wait.
            status => Err(GatewayError::Unavailable(status).into()),
        }
    }
}

/// A pretend payment gateway for trying the example: `tok_declined` is
/// declined (402), `tok_unreachable` finds it down (503), any other token
/// is charged. Called by `ChargePayment` over HTTP like a real provider,
/// so it has no CSRF token.
pub(super) async fn sandbox_charge(Json(charge): Json<renox::serde_json::Value>) -> Response {
    match charge["source"].as_str().unwrap_or_default() {
        "tok_declined" => (
            StatusCode::PAYMENT_REQUIRED,
            Json(json!({ "error": "card_declined" })),
        )
            .into_response(),
        "tok_unreachable" => StatusCode::SERVICE_UNAVAILABLE.into_response(),
        _ => (
            StatusCode::CREATED,
            Json(json!({ "id": "ch_sandbox", "amount": charge["amount"] })),
        )
            .into_response(),
    }
}
