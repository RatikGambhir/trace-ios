//! `places` — the endpoint of any segment, whatever the mode.

use sqlx::PgConnection;

use crate::{core::error::ApiError, models::journey::ValidatedCustomPlace};

/// Whether a place id exists, for the `{"type": "saved"}` form.
pub async fn find(conn: &mut PgConnection, id: i64) -> Result<Option<i64>, ApiError> {
    let row: Option<(i64,)> = sqlx::query_as("SELECT id FROM places WHERE id = $1")
        .bind(id)
        .fetch_optional(conn)
        .await?;

    Ok(row.map(|(id,)| id))
}

/// Insert a place described inline by the caller.
pub async fn insert(
    conn: &mut PgConnection,
    place: &ValidatedCustomPlace,
) -> Result<i64, ApiError> {
    // DECIMAL(9,6) has no native Rust mapping in this build of sqlx, and the
    // coordinates only ever feed a float computation — cast on the way in.
    let (id,): (i64,) = sqlx::query_as(
        r#"
        INSERT INTO places (
            name, kind, address, city, country_code, latitude, longitude, timezone
        )
        VALUES ($1, $2, $3, $4, $5, $6::float8::numeric, $7::float8::numeric, $8)
        RETURNING id
        "#,
    )
    .bind(&place.name)
    .bind(&place.kind)
    .bind(&place.address)
    .bind(&place.city)
    .bind(&place.country_code)
    .bind(place.latitude)
    .bind(place.longitude)
    .bind(&place.timezone)
    .fetch_one(conn)
    .await?;

    Ok(id)
}

/// The place mirroring an airport. Every airport has exactly one, guaranteed by
/// the `airports_sync_place` trigger from migration 0004.
pub async fn for_airport(conn: &mut PgConnection, airport_id: i64) -> Result<i64, ApiError> {
    let row: Option<(i64,)> = sqlx::query_as("SELECT id FROM places WHERE airport_id = $1")
        .bind(airport_id)
        .fetch_optional(conn)
        .await?;

    row.map(|(id,)| id).ok_or_else(|| {
        ApiError::Internal(anyhow::anyhow!(
            "airport {airport_id} has no mirrored place; is the airports_sync_place trigger installed?"
        ))
    })
}
