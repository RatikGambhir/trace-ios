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
    flights::{
        distance_between, insert_flight, resolve_airline, resolve_airport, CreateFlightRequest,
        Flight, FlightUpsert,
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

/// `POST /api/v1/flights` — record a flight.
///
/// The airline and both airports are resolved by their IATA codes and created
/// if we have not seen them before, `distance_miles` is derived from the stored
/// airport coordinates, and the whole thing runs in one transaction: either the
/// flight and everything it references land together, or nothing does.
///
/// Replaying an identical request is safe. `201` means the flight was created,
/// `200` means an earlier request already created it and the stored row is
/// returned unchanged.
pub async fn create_flight(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateFlightRequest>,
) -> Result<(StatusCode, Json<Flight>), ApiError> {
    let validated = payload.validate()?;

    // Dropping `tx` without committing rolls back, so every `?` below leaves
    // the database untouched.
    let mut tx = state.db.begin().await?;

    let origin = resolve_airport(&mut tx, &validated.origin, "origin").await?;
    let destination = resolve_airport(&mut tx, &validated.destination, "destination").await?;
    let airline_id = resolve_airline(&mut tx, &validated.airline).await?;

    let distance_miles = distance_between(&origin, &destination);

    let upsert = insert_flight(
        &mut tx,
        &validated,
        airline_id,
        origin.id,
        destination.id,
        distance_miles,
    )
    .await?;

    tx.commit().await?;

    Ok(match upsert {
        FlightUpsert::Created(flight) => {
            tracing::info!(
                flight_id = %flight.id,
                distance_miles = ?flight.distance_miles,
                "created flight"
            );
            (StatusCode::CREATED, Json(flight))
        }
        FlightUpsert::AlreadyExists(flight) => {
            tracing::info!(flight_id = %flight.id, "flight already recorded");
            (StatusCode::OK, Json(flight))
        }
    })
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
