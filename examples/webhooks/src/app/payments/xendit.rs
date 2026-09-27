//! Xendit invoice callbacks: the `x-callback-token` header must equal the
//! verification token from the Xendit dashboard.

use renox::prelude::*;
use renox::webhook;
use serde::Deserialize;

#[derive(Deserialize)]
struct Invoice {
    id: String,
    external_id: String,
    status: String,
}

pub struct Xendit;

impl Webhook for Xendit {
    const PROVIDER: &'static str = "xendit";

    fn verify(request: &WebhookRequest, state: &AppState) -> Result {
        let token = webhook::secret(state, "XENDIT_CALLBACK_TOKEN")?;
        let sent = request.header("x-callback-token").unwrap_or_default();
        webhook::ensure(webhook::same(sent, &token))
    }

    fn event_id(request: &WebhookRequest) -> Result<String> {
        let invoice: Invoice = request.json()?;
        Ok(format!("{}:{}", invoice.id, invoice.status))
    }

    async fn handle(call: WebhookCall, ctx: JobContext) -> Result {
        let invoice: Invoice = call.json()?;
        if invoice.status == "PAID" {
            super::mark_paid(&ctx.state.db, &invoice.external_id, "xendit").await?;
        }
        Ok(())
    }
}
