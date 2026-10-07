//! Made with `rnx make:module orders`, `rnx make:model Order --module orders -m`,
//! `rnx make:job SendReceipt --module orders` (and `ChargePayment`,
//! `NotifyWarehouse`, `RemindUnpaid`, `SendStatement`, `StatementsSent`),
//! `rnx make:event OrderPlaced --module orders`,
//! `rnx make:notification NewOrder --module orders`,
//! `rnx make:migration add_status_to_orders` and `rnx make:mail receipt`.

mod new_order;
mod payment;
mod receipt;
mod remind;
mod statements;

use renox::mail::Mail;
use renox::prelude::*;
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub use payment::{ChargePayment, NotifyWarehouse};
pub use receipt::SendReceipt;
pub use remind::RemindUnpaid;
pub use statements::{SendStatement, StatementsSent};

/// Where an order is. Stored as text (`unpaid`, `processing`, …).
#[derive(DbEnum, Debug, Clone, Copy, PartialEq, Default)]
pub enum OrderStatus {
    #[default]
    Unpaid,
    /// The customer paid; the charge is queued.
    Processing,
    Paid,
    /// The charge failed for good: someone has to look at it.
    NeedsAttention,
}

#[derive(Model, Serialize, Deserialize, Default, Debug, Clone)]
#[model(table = "orders")]
pub struct Order {
    pub id: i64,
    pub customer_email: String,
    pub item: String,
    /// In cents (`APP_CURRENCY`'s smallest unit).
    pub total: i64,
    pub status: OrderStatus,
    pub created_at: Option<DateTime>,
    pub updated_at: Option<DateTime>,
}

/// Emitted when an order is placed; listeners decide what follows.
#[derive(Clone)]
pub struct OrderPlaced {
    pub order_id: i64,
}

impl Event for OrderPlaced {}

pub struct Orders;

impl Module for Orders {
    fn name(&self) -> &'static str {
        "orders"
    }

    fn routes(&self) -> Routes {
        let public = Routes::new()
            .get("/", index)
            .name("home")
            .post("/orders", store)
            .name("orders.store")
            .post("/orders/{id}/pay", payment::pay)
            .name("orders.pay");
        let staff = Routes::new()
            .post("/orders/{id}/remind", remind::remind)
            .name("orders.remind")
            .post("/orders/{id}/reopen", payment::reopen)
            .name("orders.reopen")
            .post("/statements", statements::store)
            .name("statements.store")
            .get("/statements/{id}", statements::show)
            .name("statements.show")
            .require_auth();
        // The pretend payment gateway `ChargePayment` calls when
        // PAYMENT_GATEWAY_URL is unset.
        let sandbox = Routes::new()
            .post("/sandbox/gateway/charges", payment::sandbox_charge)
            .without_csrf();
        public.merge(staff).merge(sandbox)
    }

    fn register(&self, app: &mut Registry) {
        app.job::<SendReceipt>()
            .job::<ChargePayment>()
            .job::<NotifyWarehouse>()
            .job::<RemindUnpaid>()
            .job::<SendStatement>()
            .job::<StatementsSent>()
            // Listeners run right away; the notification's mail is sent here,
            // the receipt waits until the order is paid (`payment::pay`).
            .listen(|event: OrderPlaced, state| async move {
                let order = Order::find_or_404(&state.db, event.order_id).await?;
                // Customers don't log in here, so every user is shop staff. With
                // customer accounts, pick the admins by role (the `Permissions` module).
                for admin in User::all(&state.db).await? {
                    state
                        .notify(&admin, &new_order::NewOrder(order.clone()))
                        .await?;
                }
                Ok(())
            });
        let schedule = app.schedule();
        // The shop is in Jakarta whatever APP_TIMEZONE says, and closed at weekends.
        schedule
            .daily_at("21:00", "daily-sales", daily_sales)
            .weekdays()
            .timezone("Asia/Jakarta")
            .on_failure(report_failed);
        // Min hour day month weekday: Mondays at 07:30, before the shop opens.
        // (`weekly_on(Weekday::Mon, "07:30", …)` says the same.)
        schedule
            .cron("30 7 * * 1", "weekly-sales", weekly_sales)
            .timezone("Asia/Jakarta")
            .on_failure(report_failed);
    }
}

#[derive(Deserialize, Validate)]
struct OrderForm {
    #[validate(required, email)]
    customer_email: String,
    #[validate(required, max = 100)]
    item: String,
    /// In dollars as typed (`4.50`); stored in cents.
    #[validate(min = 0.01, decimal(0, 2))]
    total: f64,
}

async fn index(State(db): State<Db>) -> Result<View> {
    let orders = Order::query().latest().limit(20).get(&db).await?;
    Ok(view("orders/index.html", context! { orders }))
}

async fn store(
    State(state): State<AppState>,
    session: Session,
    Valid(form): Valid<OrderForm>,
) -> Result<Redirect> {
    let order = Order {
        customer_email: form.customer_email,
        item: form.item,
        total: (form.total * 100.0).round() as i64,
        ..Default::default()
    };
    let order = Order::create(&state.db, order).await?;
    state.emit(OrderPlaced { order_id: order.id }).await?;
    session.flash(
        "status",
        "Thanks! Pay below and your receipt is on its way.",
    )?;
    Ok(Redirect::to("/"))
}

/// Mails the day's order count and total to every admin (weekdays at 21:00
/// in Jakarta; `my-app schedule:list` shows when it runs next).
pub async fn daily_sales(state: AppState) -> Result {
    sales_report(state, 1, "Today's sales", "today").await
}

/// The same for the last seven days, on Monday mornings.
pub async fn weekly_sales(state: AppState) -> Result {
    sales_report(state, 7, "This week's sales", "in the last 7 days").await
}

async fn sales_report(state: AppState, days: i64, subject: &str, period: &str) -> Result {
    // Each scheduled run is claimed once, even with several servers on one
    // database. The lock also covers `my-app schedule:run daily-sales` typed
    // while the scheduled run is still going: the second one skips, so
    // nobody gets the report twice. (Across servers it needs
    // CACHE_STORE=database.)
    let lock = state
        .cache
        .lock(&format!("sales-report:{days}"), Duration::from_secs(300));
    let Some(guard) = lock.try_acquire().await? else {
        return Ok(());
    };

    let since = renox::db::now() - renox::chrono::TimeDelta::days(days);
    let recent = || Order::query().where_op("created_at", ">=", since);
    // Counted and summed in SQL: no rows are loaded.
    let count = recent().count(&state.db).await?;
    let total: i64 = recent().sum(&state.db, "total").await?;
    for admin in User::all(&state.db).await? {
        let mail = state.mail_view(
            &admin.email,
            subject,
            "mail/sales",
            context! { count, total, period },
        )?;
        // Reports go out through their own mailer (`App::mailer("reports", …)`
        // in lib.rs), e.g. an account kept apart from customers' receipts.
        state.queue_mail_via("reports", mail).await?;
    }
    guard.release().await?;
    Ok(())
}

/// Runs when a report fails (the error is logged anyway): tells a person.
async fn report_failed(err: Error, state: AppState) {
    let to = state
        .config
        .var("ALERT_EMAIL")
        .unwrap_or_else(|| "admin@example.com".into());
    let mail = Mail::new(to, "A sales report failed", format!("{err:?}"));
    if let Err(err) = state.queue_mail(mail).await {
        eprintln!("could not queue the failure alert: {err:?}");
    }
}

/// An amount in the smallest unit (cents) the way the `money` template
/// filter writes it (`APP_CURRENCY`: 450 is `$4.50`), for mail written in
/// Rust. `format_money` takes whole units, hence the division.
pub fn money(state: &AppState, amount: i64) -> String {
    let unit = 10f64.powi(renox::currency_decimals(&state.config.currency) as i32);
    renox::format_money(
        amount as f64 / unit,
        &state.config.currency,
        None,
        &state.config.locale,
    )
}
