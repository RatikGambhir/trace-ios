//! `vehicles` — the car used, stored once per user and reused.

use sqlx::PgConnection;
use uuid::Uuid;

use crate::{
    core::error::ApiError,
    core::sql_builder::{Insert, Select},
    models::requests::journey::ValidatedNewVehicle,
};

/// A vehicle by id, scoped to its owner so one user cannot name another's car.
pub async fn find_for_user(
    conn: &mut PgConnection,
    user_id: Uuid,
    id: i64,
) -> Result<Option<i64>, ApiError> {
    Select::from("vehicles")
        .columns(&["id"])
        .where_eq("id", id)
        .where_eq("user_id", user_id)
        .fetch_optional_scalar(conn)
        .await
        .map_err(ApiError::Database)
}

/// A vehicle by the nickname its owner gave it.
pub async fn find_by_nickname(
    conn: &mut PgConnection,
    user_id: Uuid,
    nickname: &str,
) -> Result<Option<i64>, ApiError> {
    Select::from("vehicles")
        .columns(&["id"])
        .where_eq("user_id", user_id)
        .where_eq("nickname", nickname)
        .fetch_optional_scalar(conn)
        .await
        .map_err(ApiError::Database)
}

/// Insert a vehicle, or return `None` when one with the same nickname already
/// exists for this user — the caller reads theirs instead.
pub async fn insert(
    conn: &mut PgConnection,
    user_id: Uuid,
    vehicle: &ValidatedNewVehicle,
) -> Result<Option<i64>, ApiError> {
    Insert::into("vehicles")
        .set("user_id", user_id)
        .set("nickname", &vehicle.nickname)
        .set("make", &vehicle.make)
        .set("model", &vehicle.model)
        .set("year", vehicle.year)
        .set("license_plate", &vehicle.license_plate)
        .on_conflict_do_nothing_where(&["user_id", "nickname"], "nickname IS NOT NULL")
        .returning(&["id"])
        .fetch_optional_scalar(conn)
        .await
        .map_err(ApiError::Database)
}
