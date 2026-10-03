//! Paying online, end to end. "Payment link" asks the gateway chosen in the
//! settings for a payment page (Midtrans Snap or a Xendit invoice, through
//! `state.http`), and keeps its URL on the invoice; the customer pays there;
//! the gateway calls `/webhooks/midtrans` or `/webhooks/xendit`, verified
//! by its signature, stored once per event and handled by the queue, which
//! marks the invoice paid and tells the cashiers (the bell).
//!
//! Keys come from `.env`: `MIDTRANS_SERVER_KEY`, `XENDIT_SECRET_KEY` and
//! `XENDIT_CALLBACK_TOKEN`. `MIDTRANS_URL` and `XENDIT_URL` point at the
//! sandbox by default.

use renox::auth::{Channel, DatabaseMessage, Notification, Recipient, permissions};
use renox::prelude::*;
use renox::webhook;
use renox::{HxRefresh, Toast};
use serde::Deserialize;
use std::time::Duration;

use super::Invoice;
use crate::Settings;
use crate::app::customers::Customer;

/// The gateway invoices can be paid through: the one chosen in the
/// settings, if its key is in the environment.
pub fn configured(state: &AppState, settings: &Settings) -> Option<&'static str> {
    match settings.payment_gateway.as_str() {
        "midtrans" if state.config.var("MIDTRANS_SERVER_KEY").is_some() => Some("midtrans"),
        "xendit" if state.config.var("XENDIT_SECRET_KEY").is_some() => Some("xendit"),
        _ => None,
    }
}

/// Makes the payment page for an issued invoice.
pub(crate) async fn link(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<(Toast, HxRefresh)> {
    let mut invoice = Invoice::find_or_404(&state.db, id).await?;
    abort_if(
        invoice.status != "issued",
        StatusCode::CONFLICT,
        "Only an issued invoice can be paid.",
    )?;
    let settings = Settings::load(&state.db).await?;
    let customer = Customer::find_or_404(&state.db, invoice.customer_id).await?;
    let url = match configured(&state, &settings) {
        Some("midtrans") => midtrans_page(&state, &invoice, &customer).await?,
        Some(_) => xendit_page(&state, &invoice, &customer).await?,
        None => {
            return Err(abort(
                StatusCode::UNPROCESSABLE_ENTITY,
                "Choose a payment gateway in the settings and put its key in .env.",
            ));
        }
    };
    invoice.payment_url = Some(url.clone());
    invoice.save_only(&state.db, &["payment_url"]).await?;
    Ok((
        Toast::success("The payment page is ready.")
            .body("Send the link to the customer.")
            .link("Open", url),
        HxRefresh,
    ))
}

/// The gateway answered something we can't use.
fn unusable(gateway: &str, status: StatusCode) -> Error {
    abort(
        StatusCode::BAD_GATEWAY,
        format!("{gateway} answered {status}; try again in a minute."),
    )
}

#[derive(Deserialize)]
struct SnapPage {
    redirect_url: String,
}

async fn midtrans_page(state: &AppState, invoice: &Invoice, customer: &Customer) -> Result<String> {
    let key = state.config.var("MIDTRANS_SERVER_KEY").unwrap_or_default();
    let base = state
        .config
        .var("MIDTRANS_URL")
        .unwrap_or_else(|| "https://app.sandbox.midtrans.com".into());
    let response = state
        .http
        .post(format!("{base}/snap/v1/transactions"))
        .basic_auth(&key, "")
        .json(&json!({
            "transaction_details": { "order_id": invoice.number, "gross_amount": invoice.total },
            "customer_details": { "first_name": customer.name, "email": customer.email },
        }))
        .timeout(Duration::from_secs(15))
        .send()
        .await?;
    if !response.ok() {
        return Err(unusable("Midtrans", response.status()));
    }
    Ok(response.json::<SnapPage>()?.redirect_url)
}

#[derive(Deserialize)]
struct XenditPage {
    invoice_url: String,
}

async fn xendit_page(state: &AppState, invoice: &Invoice, customer: &Customer) -> Result<String> {
    let key = state.config.var("XENDIT_SECRET_KEY").unwrap_or_default();
    let base = state
        .config
        .var("XENDIT_URL")
        .unwrap_or_else(|| "https://api.xendit.co".into());
    let response = state
        .http
        .post(format!("{base}/v2/invoices"))
        .basic_auth(&key, "")
        // A retry after a lost answer finds the same invoice.
        .header("idempotency-key", invoice.number.clone())
        .json(&json!({
            "external_id": invoice.number,
            "amount": invoice.total,
            "payer_email": customer.email,
            "description": format!("Invoice {}", invoice.number),
        }))
        .timeout(Duration::from_secs(15))
        .send()
        .await?;
    if !response.ok() {
        return Err(unusable("Xendit", response.status()));
    }
    Ok(response.json::<XenditPage>()?.invoice_url)
}

/// Marks the issued invoice `number` paid; `false` when it isn't issued
/// (already paid, void, unknown). Safe to call twice, as webhooks do.
pub async fn mark_paid(state: &AppState, number: &str, via: &str, by: &str) -> Result<bool> {
    let now = renox::db::now();
    let moved = Invoice::where_eq("number", number)
        .where_eq("status", "issued")
        .update(
            &state.db,
            &[
                ("status", &"paid"),
                ("paid_at", &now),
                ("paid_via", &via),
                ("updated_by", &by),
            ],
        )
        .await?;
    if moved == 0 {
        return Ok(false);
    }
    let Some(invoice) = Invoice::where_eq("number", number).first(&state.db).await? else {
        return Ok(true);
    };
    // The cashiers and admins hear of it (the bell, live).
    let mut told = Vec::new();
    for role in ["admin", "cashier"] {
        for user in permissions::users_with_role(&state.db, role).await? {
            if !told.contains(&user.id) {
                told.push(user.id);
                state
                    .notify(&user, &PaymentReceived(invoice.clone()))
                    .await?;
            }
        }
    }
    Ok(true)
}

/// An amount the way the `money` filter writes it (`APP_CURRENCY`).
pub fn money(amount: i64) -> String {
    match renox::context::app() {
        Some(state) => renox::format_money(
            amount as f64,
            &state.config.currency,
            None,
            &state.current_lang().locale,
        ),
        None => amount.to_string(),
    }
}

/// To cashiers and admins, when an invoice is paid.
pub struct PaymentReceived(pub Invoice);

impl Notification for PaymentReceived {
    fn kind(&self) -> &'static str {
        "payment-received"
    }

    fn channels(&self) -> Vec<Channel> {
        vec![Channel::Database]
    }

    fn to_database(&self, _: &Recipient) -> renox::serde_json::Value {
        let invoice = &self.0;
        let via = invoice.paid_via.as_deref().unwrap_or("cash");
        DatabaseMessage::success(format!("{} is paid", invoice.number))
            .body(format!("{} via {via}", money(invoice.total)))
            .url(format!("/invoices/{}", invoice.id))
            .with("invoice_id", invoice.id)
            .into()
    }
}

/// Midtrans HTTP notifications: `signature_key` is the SHA-512 of order id,
/// status code, gross amount and the server key.
#[derive(Deserialize)]
struct MidtransNotification {
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
        let n: MidtransNotification = request.json()?;
        let key = webhook::secret(state, "MIDTRANS_SERVER_KEY")?;
        let expected = webhook::sha512_hex(format!(
            "{}{}{}{key}",
            n.order_id, n.status_code, n.gross_amount
        ));
        webhook::ensure(webhook::same(&n.signature_key, &expected))
    }

    fn event_id(request: &WebhookRequest) -> Result<String> {
        let n: MidtransNotification = request.json()?;
        Ok(format!("{}:{}", n.transaction_id, n.transaction_status))
    }

    async fn handle(call: WebhookCall, ctx: JobContext) -> Result {
        let n: MidtransNotification = call.json()?;
        let paid = n.transaction_status == "settlement"
            || (n.transaction_status == "capture" && n.fraud_status.as_deref() == Some("accept"));
        if paid {
            mark_paid(&ctx.state, &n.order_id, "midtrans", "Midtrans").await?;
        }
        Ok(())
    }
}

/// Xendit invoice callbacks: the `x-callback-token` header is the token
/// from the Xendit dashboard.
#[derive(Deserialize)]
struct XenditInvoice {
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
        let invoice: XenditInvoice = request.json()?;
        Ok(format!("{}:{}", invoice.id, invoice.status))
    }

    async fn handle(call: WebhookCall, ctx: JobContext) -> Result {
        let invoice: XenditInvoice = call.json()?;
        if invoice.status == "PAID" {
            mark_paid(&ctx.state, &invoice.external_id, "xendit", "Xendit").await?;
        }
        Ok(())
    }
}
