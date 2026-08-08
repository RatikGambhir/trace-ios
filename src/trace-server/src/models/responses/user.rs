//! What `POST` and `GET` on a user return.

use serde::Serialize;

use crate::models::entities::user::User;

/// `POST /api/v1/users` — the created row plus the key it was issued.
#[derive(Debug, Serialize)]
pub struct InsertUserResponse {
    #[serde(flatten)]
    pub user: User,
    /// Shown exactly once, at creation time. It is not retrievable afterwards
    /// through any endpoint.
    pub api_key: String,
}

/// `GET /api/v1/users/{id}` — the row, with no secret columns.
///
/// An alias rather than a struct: the response *is* the entity, and duplicating
/// its fields would only create somewhere for the two to drift apart.
pub type GetUserResponse = User;
