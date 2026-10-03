//! The activity log: the `Audit` module's `audit_logs` table in a grid, for
//! admins (`activity.view`). Logins and account changes are recorded by
//! Renox; staff changes, settings and voided invoices by this app
//! (`renox::audit::record`). `Activity` is a model of its own over that
//! table, read-only, so the grid can page, filter and export it.

use renox::grid::{Column, Grid, GridRequest};
use renox::prelude::*;
use serde::Serialize;

#[derive(Model, Serialize, Default, Debug, Clone)]
#[model(table = "audit_logs")]
pub struct Activity {
    pub id: i64,
    pub user_id: Option<i64>,
    pub action: String,
    pub subject_type: Option<String>,
    pub subject_id: Option<i64>,
    /// JSON, as text.
    pub data: String,
    pub ip: Option<String>,
    pub created_at: Option<DateTime>,
}

pub fn grid() -> Grid {
    Grid::new("activity")
        .title("Activity")
        .per_page(25)
        .column(Column::datetime("created_at", "When").mobile())
        .column(
            Column::related("user", "Who", "users", "user_id", "name")
                .mobile()
                .searchable(),
        )
        .column(
            Column::text("action", "What")
                .mobile()
                .searchable()
                .badges(&[
                    ("auth.login", "success"),
                    ("auth.login_failed", "danger"),
                    ("auth.locked_out", "danger"),
                    ("invoice.voided", "warning"),
                    ("staff.roles_changed", "warning"),
                ]),
        )
        .column(Column::text("subject_type", "On").hidden())
        .column(Column::number("subject_id", "Id").hidden())
        .column(Column::text("data", "Details").limit(60).tooltip("data"))
        .column(Column::text("ip", "IP").hidden())
        .sort_by("-created_at")
        .exports()
        .empty_state("Nothing recorded yet", None)
}

pub(super) async fn index(request: GridRequest) -> Result<Response> {
    if let Some(file) = grid().export(Activity::query(), &request).await? {
        return Ok(file);
    }
    let activity = grid().page(Activity::query(), &request).await?;
    Ok(view("activity/index.html", context! { activity }).into_response())
}
