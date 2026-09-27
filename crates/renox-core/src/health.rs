//! `GET /health` for load balancers and uptime checks: 200 when the database
//! answers, 503 otherwise, with details as JSON. Not affected by maintenance
//! mode or sessions.

use std::time::Duration;

use axum::Json;
use axum::Router;
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::get;
use serde_json::{Value, json};

use crate::AppState;

pub(crate) fn router() -> Router<AppState> {
    Router::new().route("/health", get(health))
}

async fn health(State(state): State<AppState>) -> (StatusCode, Json<Value>) {
    let ping = sqlx::query_scalar::<_, i64>("SELECT 1").fetch_one(&state.db);
    let database = match tokio::time::timeout(Duration::from_secs(2), ping).await {
        Ok(Ok(_)) => Ok(()),
        Ok(Err(err)) => Err(err.to_string()),
        Err(_) => Err("timed out".to_owned()),
    };
    let queue = match &database {
        Ok(()) => {
            let pending = state.queue.pending().await.ok();
            let failed: Option<i64> = sqlx::query_scalar("SELECT COUNT(*) FROM failed_jobs")
                .fetch_one(&state.db)
                .await
                .ok();
            json!({ "pending": pending, "failed": failed })
        }
        Err(_) => Value::Null,
    };
    let maintenance = crate::maintenance::status(&state.config.storage_path).is_some();
    let status = if database.is_ok() {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    let body = json!({
        "status": if database.is_ok() { "ok" } else { "error" },
        "database": database.err().unwrap_or_else(|| "ok".to_owned()),
        "queue": queue,
        "maintenance": maintenance,
    });
    (status, Json(body))
}
