//! `users`.

use sqlx::PgConnection;
use uuid::Uuid;

use crate::{
    core::error::ApiError,
    models::user::{User, ValidatedUser},
};

/// Insert a user. The API key is supplied by the caller rather than generated
/// here — this layer runs SQL, it does not mint secrets.
pub async fn insert(
    conn: &mut PgConnection,
    user: &ValidatedUser,
    api_key: &str,
    password_hash: &str,
) -> Result<User, ApiError> {
    sqlx::query_as::<_, User>(
        r#"
        INSERT INTO users (first_name, last_name, role, api_key, password_hash)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING id, first_name, last_name, role, created_at, updated_at
        "#,
    )
    .bind(&user.first_name)
    .bind(&user.last_name)
    .bind(&user.role)
    .bind(api_key)
    .bind(password_hash)
    .fetch_one(conn)
    .await
    .map_err(ApiError::from_insert)
}

/// Read a user back without any secret columns.
pub async fn find(conn: &mut PgConnection, id: Uuid) -> Result<Option<User>, ApiError> {
    sqlx::query_as::<_, User>(
        r#"
        SELECT id, first_name, last_name, role, created_at, updated_at
        FROM users
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(conn)
    .await
    .map_err(ApiError::Database)
}

/// Whether a user exists, for the foreign keys that would otherwise fail as an
/// opaque 500 halfway through a transaction.
pub async fn exists(conn: &mut PgConnection, id: Uuid) -> Result<bool, ApiError> {
    let row: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM users WHERE id = $1")
        .bind(id)
        .fetch_optional(conn)
        .await?;

    Ok(row.is_some())
}
