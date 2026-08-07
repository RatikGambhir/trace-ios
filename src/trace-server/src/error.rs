use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

/// Every fallible handler returns this. Implementing `IntoResponse` keeps the
/// handlers themselves free of status-code plumbing.
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("validation failed")]
    Validation(Vec<String>),

    #[error("not found")]
    NotFound,

    #[error("conflict")]
    Conflict(String),

    #[error(transparent)]
    Database(#[from] sqlx::Error),

    #[error("internal error")]
    Internal(#[from] anyhow::Error),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, error, details) = match self {
            ApiError::Validation(errors) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "validation_failed".to_string(),
                Some(errors),
            ),
            ApiError::NotFound => (StatusCode::NOT_FOUND, "not_found".to_string(), None),
            ApiError::Conflict(message) => (
                StatusCode::CONFLICT,
                "conflict".to_string(),
                Some(vec![message]),
            ),
            // Database and internal failures are logged in full but reported to
            // the caller as an opaque 500 — no driver messages over the wire.
            ApiError::Database(err) => {
                tracing::error!(error = ?err, "database error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal_error".to_string(),
                    None,
                )
            }
            ApiError::Internal(err) => {
                tracing::error!(error = ?err, "internal error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal_error".to_string(),
                    None,
                )
            }
        };

        let body = match details {
            Some(details) => json!({ "error": error, "details": details }),
            None => json!({ "error": error }),
        };

        (status, Json(body)).into_response()
    }
}

impl ApiError {
    /// Turn a Postgres unique-violation into a 409 and leave everything else alone.
    pub fn from_insert(err: sqlx::Error) -> Self {
        if let sqlx::Error::Database(ref db_err) = err {
            if db_err.code().as_deref() == Some("23505") {
                return ApiError::Conflict("a user with that api key already exists".to_string());
            }
        }
        ApiError::Database(err)
    }
}
