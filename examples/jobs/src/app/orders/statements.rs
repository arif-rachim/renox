use renox::mail::Mail;
use renox::prelude::*;
use serde::{Deserialize, Serialize};

use super::Order;

/// Queues one statement per customer with an order in the last 30 days, as
/// a batch, and shows its progress.
pub(super) async fn store(State(state): State<AppState>, user: AuthUser) -> Result<Redirect> {
    let since = renox::db::now() - renox::chrono::TimeDelta::days(30);
    let customers: Vec<String> = renox::db::sql(
        "SELECT DISTINCT customer_email FROM orders WHERE created_at >= ? ORDER BY customer_email",
    )
    .bind(since)
    .scalars(&state.db)
    .await?;
    let mut batch = state.queue.batch("monthly-statements");
    for customer_email in customers {
        batch = batch.push(SendStatement { customer_email });
    }
    let id = batch
        // One bad address shouldn't stop the others (by default the first
        // job that fails for good cancels the rest).
        .allow_failures()
        // Queued once every statement went out.
        .then(StatementsSent {
            admin_email: user.email.clone(),
        })
        .dispatch()
        .await?;
    Ok(Redirect::to(&format!("/statements/{id}")))
}

/// The progress page; htmx polls it and gets only the `progress` block.
pub(super) async fn show(State(state): State<AppState>, Path(id): Path<i64>) -> Result<View> {
    let Some(batch) = state.queue.batch_status(id).await? else {
        return Err(Error::NotFound);
    };
    let progress = batch.progress(); // 0–100: jobs that ran, failed ones included
    Ok(view("orders/statements.html", context! { batch, progress }).fragment("progress"))
}

/// One customer's order count and total for the last 30 days.
#[derive(Serialize, Deserialize)]
pub struct SendStatement {
    pub customer_email: String,
}

impl Job for SendStatement {
    const NAME: &'static str = "send-statement";

    async fn handle(self, ctx: JobContext) -> Result {
        let db = &ctx.state.db;
        let since = renox::db::now() - renox::chrono::TimeDelta::days(30);
        let orders = || {
            Order::where_eq("customer_email", &self.customer_email).where_op(
                "created_at",
                ">=",
                since,
            )
        };
        let count = orders().count(db).await?;
        let total: i64 = orders().sum(db, "total").await?;
        // The orders themselves, as a spreadsheet the customer can open.
        let mut csv = String::from("order,date,item,total\n");
        for order in orders().order_by("id").get(db).await? {
            let date = order
                .created_at
                .map(|at| at.format("%Y-%m-%d").to_string())
                .unwrap_or_default();
            csv.push_str(&format!(
                "{},{date},\"{}\",{}\n",
                order.id,
                order.item.replace('"', "\"\""),
                order.total
            ));
        }
        let month = renox::db::now().format("%Y-%m");
        let mut mail = Mail::new(
            &self.customer_email,
            "Your monthly statement",
            format!(
                "{count} order(s) in the last 30 days, {}. The orders are attached.",
                super::money(&ctx.state, total)
            ),
        )
        .attach(format!("statement-{month}.csv"), "text/csv", csv);
        // A copy for the books, without the customer seeing it.
        if let Some(accounts) = ctx.state.config.var("ACCOUNTS_EMAIL") {
            mail = mail.bcc(accounts);
        }
        ctx.state.mailer.send(mail).await
    }
}

/// The batch's `then` job: tells the staff member who started it.
#[derive(Serialize, Deserialize)]
pub struct StatementsSent {
    pub admin_email: String,
}

impl Job for StatementsSent {
    const NAME: &'static str = "statements-sent";

    async fn handle(self, ctx: JobContext) -> Result {
        let mail = Mail::new(
            &self.admin_email,
            "Monthly statements sent",
            "Every customer's statement went out.",
        );
        ctx.state.mailer.send(mail).await
    }
}
