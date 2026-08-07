//! `vehicles` — the car used, stored once per user and reused.

use sqlx::PgConnection;
use uuid::Uuid;

use crate::{core::error::ApiError, models::journey::ValidatedNewVehicle};

/// A vehicle by id, scoped to its owner so one user cannot name another's car.
pub async fn find_for_user(
    conn: &mut PgConnection,
    user_id: Uuid,
    id: i64,
) -> Result<Option<i64>, ApiError> {
    let row: Option<(i64,)> =
        sqlx::query_as("SELECT id FROM vehicles WHERE id = $1 AND user_id = $2")
            .bind(id)
            .bind(user_id)
            .fetch_optional(conn)
            .await?;

    Ok(row.map(|(id,)| id))
}

/// A vehicle by the nickname its owner gave it.
pub async fn find_by_nickname(
    conn: &mut PgConnection,
    user_id: Uuid,
    nickname: &str,
) -> Result<Option<i64>, ApiError> {
    let row: Option<(i64,)> =
        sqlx::query_as("SELECT id FROM vehicles WHERE user_id = $1 AND nickname = $2")
            .bind(user_id)
            .bind(nickname)
            .fetch_optional(conn)
            .await?;

    Ok(row.map(|(id,)| id))
}

/// Insert a vehicle, or return `None` when one with the same nickname already
/// exists for this user — the caller reads theirs instead.
pub async fn insert(
    conn: &mut PgConnection,
    user_id: Uuid,
    vehicle: &ValidatedNewVehicle,
) -> Result<Option<i64>, ApiError> {
    let row: Option<(i64,)> = sqlx::query_as(
        r#"
        INSERT INTO vehicles (user_id, nickname, make, model, year, license_plate)
        VALUES ($1, $2, $3, $4, $5, $6)
        ON CONFLICT (user_id, nickname) WHERE nickname IS NOT NULL
        DO NOTHING
        RETURNING id
        "#,
    )
    .bind(user_id)
    .bind(&vehicle.nickname)
    .bind(&vehicle.make)
    .bind(&vehicle.model)
    .bind(vehicle.year)
    .bind(&vehicle.license_plate)
    .fetch_optional(conn)
    .await?;

    Ok(row.map(|(id,)| id))
}
