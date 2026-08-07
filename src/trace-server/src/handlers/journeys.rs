//! HTTP for `/api/v1/journeys`.

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use uuid::Uuid;

use crate::{
    core::{error::ApiError, state::AppState},
    models::journey::{CreateJourneyRequest, JourneyResponse},
    services::journeys,
};

/// `POST /api/v1/journeys` — record a trip and everything it is made of.
///
/// `201` means the journey was created. `200` means an earlier request with the
/// same `idempotency_key` already created it and the stored trip is returned
/// unchanged.
pub async fn create(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateJourneyRequest>,
) -> Result<(StatusCode, Json<JourneyResponse>), ApiError> {
    let (created, journey) = journeys::create(&state.db, payload).await?;

    let status = if created {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };

    Ok((status, Json(journey)))
}

/// `GET /api/v1/journeys/{id}` — the trip, its segments, and their details.
pub async fn get(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<JourneyResponse>, ApiError> {
    Ok(Json(journeys::get(&state.db, id).await?))
}
