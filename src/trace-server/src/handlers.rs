use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{
    error::ApiError,
    journeys::{
        ensure_user_exists, insert_journey, insert_segment, load_journey, CreateJourneyRequest,
        JourneyResponse, JourneyUpsert, PlaceCache,
    },
    models::{generate_api_key, hash_password, CreateUserRequest, CreateUserResponse, User},
    state::AppState,
};

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

/// `POST /api/v1/users` — insert a user and return it along with a freshly
/// minted API key. The key is shown here and never again.
pub async fn create_user(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateUserRequest>,
) -> Result<(StatusCode, Json<CreateUserResponse>), ApiError> {
    let validated = payload.validate()?;

    // Argon2 is intentionally expensive, so keep it off the async executor.
    let password = validated.password;
    let password_hash = tokio::task::spawn_blocking(move || hash_password(&password))
        .await
        .map_err(|err| ApiError::Internal(anyhow::anyhow!("hashing task panicked: {err}")))??;

    let api_key = generate_api_key();

    let user = sqlx::query_as::<_, User>(
        r#"
        INSERT INTO users (first_name, last_name, role, api_key, password_hash)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING id, first_name, last_name, role, created_at, updated_at
        "#,
    )
    .bind(&validated.first_name)
    .bind(&validated.last_name)
    .bind(&validated.role)
    .bind(&api_key)
    .bind(&password_hash)
    .fetch_one(&state.db)
    .await
    .map_err(ApiError::from_insert)?;

    tracing::info!(user_id = %user.id, "created user");

    Ok((
        StatusCode::CREATED,
        Json(CreateUserResponse { user, api_key }),
    ))
}

/// `POST /api/v1/journeys` — record a trip and everything it is made of.
///
/// One request writes the journey, its segments, the places at either end of
/// each one, and — per mode — the airport, airline, and flight behind a flight
/// segment, or the vehicle behind a drive. All of it in a single transaction,
/// so either the whole trip lands or none of it does.
///
/// `201` means the journey was created. `200` means an earlier request with the
/// same `idempotency_key` already created it and the stored trip is returned
/// unchanged.
pub async fn create_journey(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateJourneyRequest>,
) -> Result<(StatusCode, Json<JourneyResponse>), ApiError> {
    let journey = payload.validate()?;

    // Dropping `tx` without committing rolls back, so every `?` below leaves
    // the database untouched.
    let mut tx = state.db.begin().await?;

    ensure_user_exists(&mut tx, journey.user_id).await?;

    // The journey row goes in first. A replay conflicts here, before any
    // segment or reference row is touched, so it costs one statement and
    // creates nothing.
    let journey_id = match insert_journey(&mut tx, &journey).await? {
        JourneyUpsert::AlreadyExists(id) => {
            let response = load_journey(&mut tx, id).await?;
            tx.commit().await?;

            tracing::info!(journey_id = %id, "journey already recorded");
            return Ok((StatusCode::OK, Json(response)));
        }
        JourneyUpsert::Created(id) => id,
    };

    // Shared across the segments so a waypoint two legs both name — the airport
    // one leg ends at and the next begins from — resolves to a single place.
    let mut places = PlaceCache::default();

    for segment in &journey.segments {
        insert_segment(&mut tx, &mut places, journey_id, journey.user_id, segment).await?;
    }

    let response = load_journey(&mut tx, journey_id).await?;

    tx.commit().await?;

    tracing::info!(
        journey_id = %journey_id,
        segments = response.segments.len(),
        distance_miles = response.totals.total_distance_miles,
        "created journey"
    );

    Ok((StatusCode::CREATED, Json(response)))
}

/// `GET /api/v1/journeys/{id}` — the trip, its segments, and their details.
pub async fn get_journey(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<JourneyResponse>, ApiError> {
    let mut conn = state.db.acquire().await?;

    Ok(Json(load_journey(&mut conn, id).await?))
}

/// `GET /api/v1/users/{id}` — read back a user without any secret columns.
pub async fn get_user(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<User>, ApiError> {
    let user = sqlx::query_as::<_, User>(
        r#"
        SELECT id, first_name, last_name, role, created_at, updated_at
        FROM users
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(ApiError::NotFound)?;

    Ok(Json(user))
}
