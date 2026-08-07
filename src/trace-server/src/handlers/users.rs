//! HTTP for `/api/v1/users`.

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use uuid::Uuid;

use crate::{
    core::{error::ApiError, state::AppState},
    models::user::{CreateUserRequest, CreateUserResponse, User},
    services::users,
};

/// `POST /api/v1/users`
pub async fn create(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateUserRequest>,
) -> Result<(StatusCode, Json<CreateUserResponse>), ApiError> {
    let user = users::create(&state.db, payload).await?;

    Ok((StatusCode::CREATED, Json(user)))
}

/// `GET /api/v1/users/{id}`
pub async fn get(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<User>, ApiError> {
    Ok(Json(users::get(&state.db, id).await?))
}
