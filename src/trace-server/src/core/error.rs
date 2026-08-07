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

/// Turn a unique violation into a caller-visible message when `describe`
/// recognises the constraint, and leave every other error alone.
pub fn unique_violation(err: sqlx::Error, describe: impl Fn(&str) -> Option<String>) -> ApiError {
    if let sqlx::Error::Database(ref db_err) = err {
        if db_err.code().as_deref() == Some("23505") {
            if let Some(message) = db_err.constraint().and_then(&describe) {
                return ApiError::Conflict(message);
            }
        }
    }

    ApiError::Database(err)
}

/// Report a check-constraint violation as a validation failure. Everything the
/// `flights` constraints cover is also checked in `validate`, so reaching this
/// means the row was rejected for a reason the request alone did not show —
/// name the constraint rather than swallowing it into a 500.
pub fn check_violation(err: sqlx::Error) -> ApiError {
    if let sqlx::Error::Database(ref db_err) = err {
        if db_err.code().as_deref() == Some("23514") {
            if let Some(constraint) = db_err.constraint() {
                return ApiError::Validation(vec![format!("the flight violates {constraint}")]);
            }
        }
    }

    ApiError::Database(err)
}

/// Name the segment when a `journey_segments` constraint rejects the row, so the
/// caller learns which leg was wrong rather than just which constraint fired.
pub fn segment_constraint_error(err: sqlx::Error, prefix: &str) -> ApiError {
    if let sqlx::Error::Database(ref db_err) = err {
        if matches!(db_err.code().as_deref(), Some("23514") | Some("23503")) {
            if let Some(constraint) = db_err.constraint() {
                return ApiError::Validation(vec![format!("{prefix} violates {constraint}")]);
            }
        }
    }

    ApiError::Database(err)
}
