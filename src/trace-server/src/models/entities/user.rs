//! Rows of `users`.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::FromRow;
use uuid::Uuid;

/// A user row, minus the secret columns. `password_hash` is never serialised,
/// and `api_key` is only returned once, by `InsertUserResponse`.
#[derive(Debug, Serialize, FromRow)]
pub struct User {
    pub id: Uuid,
    pub first_name: String,
    pub last_name: String,
    pub role: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
