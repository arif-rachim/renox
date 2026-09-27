//! Stripe events: the `Stripe-Signature` header holds a timestamp and an
//! HMAC-SHA256 of `"{timestamp}.{body}"` with the endpoint's signing secret.

use std::time::Duration;

use renox::prelude::*;
use renox::webhook;
use serde::Deserialize;

#[derive(Deserialize)]
struct Event {
    id: String,
    #[serde(rename = "type")]
    kind: String,
    data: Data,
}

#[derive(Deserialize)]
struct Data {
    object: Session,
}

/// The parts of a Checkout Session this app uses.
#[derive(Deserialize)]
struct Session {
    client_reference_id: Option<String>,
    payment_status: Option<String>,
}

pub struct Stripe;

impl Webhook for Stripe {
    const PROVIDER: &'static str = "stripe";

    fn verify(request: &WebhookRequest, state: &AppState) -> Result {
        let secret = webhook::secret(state, "STRIPE_WEBHOOK_SECRET")?;
        let header = request.header("stripe-signature").unwrap_or_default();
        let five_minutes = Duration::from_secs(300);
        webhook::ensure(webhook::verify_timestamped(
            secret,
            &request.body,
            header,
            five_minutes,
        ))
    }

    fn event_id(request: &WebhookRequest) -> Result<String> {
        Ok(request.json::<Event>()?.id)
    }

    async fn handle(call: WebhookCall, ctx: JobContext) -> Result {
        let event: Event = call.json()?;
        let session = event.data.object;
        if event.kind == "checkout.session.completed"
            && session.payment_status.as_deref() == Some("paid")
            && let Some(code) = session.client_reference_id
        {
            super::mark_paid(&ctx.state.db, &code, "stripe").await?;
        }
        Ok(())
    }
}
