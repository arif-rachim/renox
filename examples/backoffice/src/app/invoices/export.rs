//! Exports too big to wait for: the invoices grid's bulk action posts the
//! selection (or "all matching") with the grid's query string; a job makes
//! the CSV with the same filters (`Grid::export` on a `GridRequest` built
//! from them), stores it, and tells the user through the bell, with a link
//! that works for a day (`Storage::temporary_url`).

use renox::Toast;
use renox::auth::{Channel, DatabaseMessage, Notification, Recipient};
use renox::grid::{GridRequest, Selection};
use renox::prelude::*;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use super::Invoice;

pub(crate) async fn start(
    State(state): State<AppState>,
    user: AuthUser,
    Query(params): Query<Vec<(String, String)>>,
    Form(selection): Form<Selection>,
) -> Result<Toast> {
    let ids = selection
        .ids
        .iter()
        .map(|id| id.parse::<i64>())
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|_| Error::BadRequest("Those aren't invoice ids.".into()))?;
    state
        .queue
        .dispatch(ExportInvoices {
            user_id: user.id,
            params,
            ids,
            all: selection.all,
        })
        .await?;
    Ok(Toast::info("Exporting…").body("The bell will have the file in a moment."))
}

/// Writes the CSV and notifies the user who asked.
#[derive(Serialize, Deserialize)]
pub struct ExportInvoices {
    pub user_id: i64,
    /// The grid's query string when the export was asked for.
    pub params: Vec<(String, String)>,
    /// The selected invoices, unless `all`.
    pub ids: Vec<i64>,
    pub all: bool,
}

impl Job for ExportInvoices {
    const NAME: &'static str = "export-invoices";

    async fn handle(self, ctx: JobContext) -> Result {
        let state = &ctx.state;
        let user = User::find_or_404(&state.db, self.user_id).await?;
        let mut params: Vec<(&str, &str)> = self
            .params
            .iter()
            .filter(|(name, _)| name != "export")
            .map(|(name, value)| (name.as_str(), value.as_str()))
            .collect();
        params.push(("export", "csv"));
        let request = GridRequest::new(&state.db, "/invoices", &params);
        let query = if self.all {
            Invoice::query()
        } else {
            Invoice::query().where_in("id", self.ids.iter().copied())
        };
        let Some(file) = super::grid(false).export(query, &request).await? else {
            return Err(Error::permanent(std::io::Error::other(
                "the invoices grid made no file",
            )));
        };
        let bytes = renox::axum::body::to_bytes(file.into_body(), usize::MAX)
            .await
            .map_err(|err| Error::BadRequest(err.to_string()))?;
        let rows = bytes
            .iter()
            .filter(|b| **b == b'\n')
            .count()
            .saturating_sub(1);
        let key = format!(
            "exports/invoices-{}-{}.csv",
            user.id,
            renox::db::now().format("%Y%m%d-%H%M%S")
        );
        state.storage.put(&key, bytes).await?;
        let url = state
            .storage
            .temporary_url(state, &key, Duration::from_secs(24 * 60 * 60))
            .await?;
        state.notify(&user, &ExportReady { url, rows }).await
    }
}

/// The file is ready: a link in the bell.
pub struct ExportReady {
    pub url: String,
    pub rows: usize,
}

impl Notification for ExportReady {
    fn kind(&self) -> &'static str {
        "export-ready"
    }

    fn channels(&self) -> Vec<Channel> {
        vec![Channel::Database]
    }

    fn to_database(&self, _: &Recipient) -> renox::serde_json::Value {
        DatabaseMessage::success("Your invoice export is ready")
            .body(format!("{} invoices; the link works for a day.", self.rows))
            .url(self.url.clone())
            .into()
    }
}
