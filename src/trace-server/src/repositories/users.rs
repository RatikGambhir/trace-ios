//! `users`.

use sqlx::PgConnection;
use uuid::Uuid;

use crate::{
    core::error::ApiError,
    core::sql_builder::{Insert, Select},
    models::entities::user::User,
    models::requests::user::ValidatedUser,
};

/// A user without any secret columns — never `api_key`, never `password_hash`.
const PUBLIC_COLUMNS: &[&str] = &[
    "id",
    "first_name",
    "last_name",
    "role",
    "created_at",
    "updated_at",
];

/// Insert a user. The API key is supplied by the caller rather than generated
/// here — this layer runs SQL, it does not mint secrets.
pub async fn insert(
    conn: &mut PgConnection,
    user: &ValidatedUser,
    api_key: &str,
    password_hash: &str,
) -> Result<User, ApiError> {
    Insert::into("users")
        .set("first_name", &user.first_name)
        .set("last_name", &user.last_name)
        .set("role", &user.role)
        .set("api_key", api_key)
        .set("password_hash", password_hash)
        .returning(PUBLIC_COLUMNS)
        .fetch_one(conn)
        .await
        .map_err(ApiError::from_insert)
}

/// Read a user back without any secret columns.
pub async fn find(conn: &mut PgConnection, id: Uuid) -> Result<Option<User>, ApiError> {
    Select::from("users")
        .columns(PUBLIC_COLUMNS)
        .where_eq("id", id)
        .fetch_optional(conn)
        .await
        .map_err(ApiError::Database)
}

/// Whether a user exists, for the foreign keys that would otherwise fail as an
/// opaque 500 halfway through a transaction.
pub async fn exists(conn: &mut PgConnection, id: Uuid) -> Result<bool, ApiError> {
    let found: Option<Uuid> = Select::from("users")
        .columns(&["id"])
        .where_eq("id", id)
        .fetch_optional_scalar(conn)
        .await?;

    Ok(found.is_some())
}
