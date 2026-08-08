//! Liveness and readiness.

use std::sync::Arc;

use axum::{extract::State, http::StatusCode, Json};
use serde_json::{json, Value};

use crate::core::state::AppState;

/// Liveness probe — answers as long as the process is up.
pub async fn health() -> Json<Value> {
    Json(json!({ "status": "healthy" }))
}

/// Readiness probe — answers only once the database is reachable.
pub async fn ready(State(state): State<Arc<AppState>>) -> Result<Json<Value>, StatusCode> {
    sqlx::query("SELECT 1")
        .execute(&state.db)
        .await
        .map_err(|err| {
            tracing::warn!(error = ?err, "readiness check failed");
            StatusCode::SERVICE_UNAVAILABLE
        })?;

    Ok(Json(json!({ "status": "ready" })))
}
