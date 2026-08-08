//! Creating a user: hash the password off the executor, mint a key, store both.

use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    core::{
        crypto::{generate_api_key, hash_password},
        error::ApiError,
    },
    models::entities::user::User,
    models::requests::user::InsertUserRequest,
    models::responses::user::InsertUserResponse,
    repositories::users,
};

/// Insert a user and return it with a freshly minted API key. The key is
/// returned here and never again — no endpoint reads it back.
pub async fn create(
    pool: &PgPool,
    request: InsertUserRequest,
) -> Result<InsertUserResponse, ApiError> {
    let validated = request.validate()?;

    // Argon2 is intentionally expensive, so keep it off the async executor.
    let password = validated.password.clone();
    let password_hash = tokio::task::spawn_blocking(move || hash_password(&password))
        .await
        .map_err(|err| ApiError::Internal(anyhow::anyhow!("hashing task panicked: {err}")))??;

    let api_key = generate_api_key();

    let mut conn = pool.acquire().await?;
    let user = users::insert(&mut conn, &validated, &api_key, &password_hash).await?;

    tracing::info!(user_id = %user.id, "created user");

    Ok(InsertUserResponse { user, api_key })
}

/// Read a user back without any secret columns.
pub async fn get(pool: &PgPool, id: Uuid) -> Result<User, ApiError> {
    let mut conn = pool.acquire().await?;

    users::find(&mut conn, id).await?.ok_or(ApiError::NotFound)
}
