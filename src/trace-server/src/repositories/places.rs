//! `places` — the endpoint of any segment, whatever the mode.

use sqlx::PgConnection;

use crate::{
    core::error::ApiError,
    core::sql_builder::{Insert, Select},
    models::requests::journey::ValidatedCustomPlace,
};

/// Whether a place id exists, for the `{"type": "saved"}` form.
pub async fn find(conn: &mut PgConnection, id: i64) -> Result<Option<i64>, ApiError> {
    Select::from("places")
        .columns(&["id"])
        .where_eq("id", id)
        .fetch_optional_scalar(conn)
        .await
        .map_err(ApiError::Database)
}

/// Insert a place described inline by the caller.
pub async fn insert(
    conn: &mut PgConnection,
    place: &ValidatedCustomPlace,
) -> Result<i64, ApiError> {
    // DECIMAL(9,6) has no native Rust mapping in this build of sqlx, and the
    // coordinates only ever feed a float computation — cast on the way in.
    Insert::into("places")
        .set("name", &place.name)
        .set("kind", &place.kind)
        .set("address", &place.address)
        .set("city", &place.city)
        .set("country_code", &place.country_code)
        .set_cast("latitude", place.latitude, "float8::numeric")
        .set_cast("longitude", place.longitude, "float8::numeric")
        .set("timezone", &place.timezone)
        .returning(&["id"])
        .fetch_one_scalar(conn)
        .await
        .map_err(ApiError::Database)
}

/// The place mirroring an airport. Every airport has exactly one, guaranteed by
/// the `airports_sync_place` trigger from migration 0004.
pub async fn for_airport(conn: &mut PgConnection, airport_id: i64) -> Result<i64, ApiError> {
    Select::from("places")
        .columns(&["id"])
        .where_eq("airport_id", airport_id)
        .fetch_optional_scalar(conn)
        .await?
        .ok_or_else(|| {
            ApiError::Internal(anyhow::anyhow!(
                "airport {airport_id} has no mirrored place; is the airports_sync_place trigger installed?"
            ))
        })
}
