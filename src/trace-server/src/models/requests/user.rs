//! The user a caller posts, and its validated form.

use serde::Deserialize;

use crate::core::error::ApiError;

const MAX_NAME_LEN: usize = 100;
const MAX_ROLE_LEN: usize = 50;
const MIN_PASSWORD_LEN: usize = 8;
/// Argon2 is deliberately slow; cap the input so a huge body can't be used to
/// tie up a blocking thread.
const MAX_PASSWORD_LEN: usize = 1024;

const DEFAULT_ROLE: &str = "user";

/// Body of `POST /api/v1/users`.
/// Body of `POST /api/v1/users`.
#[derive(Debug, Deserialize)]
pub struct InsertUserRequest {
    pub first_name: String,
    pub last_name: String,
    /// Optional; the column defaults to `user`.
    #[serde(default)]
    pub role: Option<String>,
    pub password: String,
}

/// A `InsertUserRequest` that has been trimmed and checked.
pub struct ValidatedUser {
    pub first_name: String,
    pub last_name: String,
    pub role: String,
    pub password: String,
}

impl InsertUserRequest {
    pub fn validate(self) -> Result<ValidatedUser, ApiError> {
        let mut errors = Vec::new();

        let first_name = self.first_name.trim().to_string();
        let last_name = self.last_name.trim().to_string();
        let role = self
            .role
            .map(|r| r.trim().to_string())
            .filter(|r| !r.is_empty())
            .unwrap_or_else(|| DEFAULT_ROLE.to_string());

        if first_name.is_empty() {
            errors.push("first_name must not be empty".to_string());
        } else if first_name.chars().count() > MAX_NAME_LEN {
            errors.push(format!(
                "first_name must be at most {MAX_NAME_LEN} characters"
            ));
        }

        if last_name.is_empty() {
            errors.push("last_name must not be empty".to_string());
        } else if last_name.chars().count() > MAX_NAME_LEN {
            errors.push(format!(
                "last_name must be at most {MAX_NAME_LEN} characters"
            ));
        }

        if role.chars().count() > MAX_ROLE_LEN {
            errors.push(format!("role must be at most {MAX_ROLE_LEN} characters"));
        }

        if self.password.chars().count() < MIN_PASSWORD_LEN {
            errors.push(format!(
                "password must be at least {MIN_PASSWORD_LEN} characters"
            ));
        } else if self.password.len() > MAX_PASSWORD_LEN {
            errors.push(format!("password must be at most {MAX_PASSWORD_LEN} bytes"));
        }

        if !errors.is_empty() {
            return Err(ApiError::Validation(errors));
        }

        Ok(ValidatedUser {
            first_name,
            last_name,
            role,
            password: self.password,
        })
    }
}

#[cfg(test)]
#[path = "user_tests.rs"]
mod tests;
