//! The dashboard: what was billed and collected in the period chosen
//! (`?period=`, `renox::chart::Period`) against the period before, what is
//! still unpaid, invoices past their due date and products running low.

use renox::chart::{Period, Series, Trend};
use renox::prelude::*;

use super::invoices::{self, Invoice};
use super::products;

/// Issued or paid: money billed.
fn billed() -> renox::db::Query<Invoice> {
    Invoice::query().where_in("status", ["issued", "paid"])
}

pub(super) async fn show(State(state): State<AppState>, period: Period) -> Result<View> {
    let db = &state.db;
    let billed_now = Trend::of(billed(), "issued_on")
        .over(period)
        .sum(&state, "total")
        .await?;
    let billed_before = Trend::of(billed(), "issued_on")
        .over(period.previous())
        .sum(&state, "total")
        .await?;
    let collected = Trend::of(Invoice::where_eq("status", "paid"), "paid_at")
        .over(period)
        .sum(&state, "total")
        .await?;
    let collected_before = Trend::of(Invoice::where_eq("status", "paid"), "paid_at")
        .over(period.previous())
        .sum(&state, "total")
        .await?;
    let unpaid: i64 = Invoice::where_eq("status", "issued")
        .sum(db, "total")
        .await?;
    let today = invoices::today(&state.config);
    let overdue = Invoice::where_eq("status", "issued")
        .where_op("due_on", "<", today)
        .order_by("due_on")
        .limit(8)
        .get(db)
        .await?;
    let overdue_count = Invoice::where_eq("status", "issued")
        .where_op("due_on", "<", today)
        .count(db)
        .await?;
    let low = products::running_low().limit(8).get(db).await?;
    let low_count = products::running_low().count(db).await?;
    Ok(view(
        "dashboard/show.html",
        context! {
            period,
            billed_total => billed_now.total(),
            billed_change => billed_now.change_from(&billed_before),
            collected_total => collected.total(),
            collected_change => collected.change_from(&collected_before),
            unpaid,
            overdue,
            overdue_count,
            low,
            low_count,
            billed_series => [
                billed_now.clone().named("This period"),
                Series::new(billed_now.labels.clone(), billed_before.values).named("The period before"),
            ],
            billed_labels => billed_now.labels.clone(),
            billed => billed_now,
            collected,
        },
    ))
}
