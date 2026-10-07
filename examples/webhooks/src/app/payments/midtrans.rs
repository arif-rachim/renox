//! Midtrans HTTP notifications: the JSON body carries `signature_key`, the
//! SHA-512 of order id + status code + gross amount + your server key.
//! Midtrans charges rupiah only: `gross_amount` is in IDR (`"150000.00"`),
//! whatever the app's `APP_CURRENCY`; it is only part of the signature here.

use renox::prelude::*;
use renox::webhook;
use serde::Deserialize;

#[derive(Deserialize)]
struct Notification {
    order_id: String,
    status_code: String,
    gross_amount: String,
    signature_key: String,
    transaction_id: String,
    transaction_status: String,
    fraud_status: Option<String>,
}

pub struct Midtrans;

impl Webhook for Midtrans {
    const PROVIDER: &'static str = "midtrans";

    fn verify(request: &WebhookRequest, state: &AppState) -> Result {
        let n: Notification = request.json()?;
        let key = webhook::secret(state, "MIDTRANS_SERVER_KEY")?;
        let expected = webhook::sha512_hex(format!(
            "{}{}{}{key}",
            n.order_id, n.status_code, n.gross_amount
        ));
        webhook::ensure(webhook::same(&n.signature_key, &expected))
    }

    // Midtrans calls once per status change of a transaction.
    fn event_id(request: &WebhookRequest) -> Result<String> {
        let n: Notification = request.json()?;
        Ok(format!("{}:{}", n.transaction_id, n.transaction_status))
    }

    async fn handle(call: WebhookCall, ctx: JobContext) -> Result {
        let n: Notification = call.json()?;
        let paid = n.transaction_status == "settlement"
            || (n.transaction_status == "capture" && n.fraud_status.as_deref() == Some("accept"));
        if paid {
            super::mark_paid(&ctx.state.db, &n.order_id, "midtrans").await?;
        }
        Ok(())
    }
}
