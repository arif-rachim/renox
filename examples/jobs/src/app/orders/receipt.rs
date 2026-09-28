use renox::prelude::*;
use serde::{Deserialize, Serialize};

use super::Order;

/// Mails the receipt. Jobs are stored as JSON, so it keeps only the id.
#[derive(Serialize, Deserialize)]
pub struct SendReceipt {
    pub order_id: i64,
}

impl Job for SendReceipt {
    const NAME: &'static str = "send-receipt";
    // A customer is waiting for it: `queue:work --queue high,default` runs it
    // before the reports and statements on `default`. (`state.queue.dispatch_on("high", job)`
    // picks the queue for one dispatch instead.)
    const QUEUE: &'static str = "high";
    // A mail server can be down for a while: retry five times, slower each time.
    const MAX_ATTEMPTS: u32 = 5;

    async fn handle(self, ctx: JobContext) -> Result {
        let order = Order::find_or_404(&ctx.state.db, self.order_id).await?;
        let mail = ctx.state.mail_view(
            &order.customer_email,
            format!("Your receipt for order #{}", order.id),
            "mail/receipt",
            context! { order },
        )?;
        ctx.state.mailer.send(mail).await
    }
}
