use argon2::{
    password_hash::{
        rand_core::{OsRng, RngCore},
        PasswordHasher, SaltString,
    },
    Argon2,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::error::ApiError;

const MAX_NAME_LEN: usize = 100;
const MAX_ROLE_LEN: usize = 50;
const MIN_PASSWORD_LEN: usize = 8;
/// Argon2 is deliberately slow; cap the input so a huge body can't be used to
/// tie up a blocking thread.
const MAX_PASSWORD_LEN: usize = 1024;

const DEFAULT_ROLE: &str = "user";
const API_KEY_PREFIX: &str = "trace_sk_";

/// Body of `POST /api/v1/users`.
#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    pub first_name: String,
    pub last_name: String,
    /// Optional; the column defaults to `user`.
    #[serde(default)]
    pub role: Option<String>,
    pub password: String,
}

/// A user row, minus the secret columns. `password_hash` is never serialised,
/// and `api_key` is only returned once, by `CreateUserResponse`.
#[derive(Debug, Serialize, FromRow)]
pub struct User {
    pub id: Uuid,
    pub first_name: String,
    pub last_name: String,
    pub role: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct CreateUserResponse {
    #[serde(flatten)]
    pub user: User,
    /// Shown exactly once, at creation time. It is not retrievable afterwards
    /// through any endpoint.
    pub api_key: String,
}

/// A `CreateUserRequest` that has been trimmed and checked.
pub struct ValidatedUser {
    pub first_name: String,
    pub last_name: String,
    pub role: String,
    pub password: String,
}

impl CreateUserRequest {
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

/// Generate an opaque API key: a prefix for greppability plus 256 bits of entropy.
pub fn generate_api_key() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    format!("{API_KEY_PREFIX}{}", URL_SAFE_NO_PAD.encode(bytes))
}

/// Hash a password with Argon2id using a fresh random salt. The returned PHC
/// string carries the salt and parameters, so verification needs nothing else.
///
/// This is CPU-bound by design — call it from `spawn_blocking`.
pub fn hash_password(password: &str) -> anyhow::Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|err| anyhow::anyhow!("failed to hash password: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(first: &str, last: &str, password: &str) -> CreateUserRequest {
        CreateUserRequest {
            first_name: first.to_string(),
            last_name: last.to_string(),
            role: None,
            password: password.to_string(),
        }
    }

    #[test]
    fn trims_names_and_defaults_the_role() {
        let validated = request("  Ada  ", " Lovelace ", "correct horse")
            .validate()
            .expect("should be valid");

        assert_eq!(validated.first_name, "Ada");
        assert_eq!(validated.last_name, "Lovelace");
        assert_eq!(validated.role, "user");
    }

    #[test]
    fn rejects_blank_names_and_short_passwords_together() {
        // Deliberately no `Debug` on `ValidatedUser` — it carries a plaintext
        // password — so match the success arm explicitly rather than format it.
        let errors = match request("   ", "Lovelace", "short").validate() {
            Err(ApiError::Validation(errors)) => errors,
            Err(other) => panic!("expected validation errors, got {other:?}"),
            Ok(_) => panic!("expected validation to fail"),
        };

        assert_eq!(errors.len(), 2);
        assert!(errors.iter().any(|e| e.contains("first_name")));
        assert!(errors.iter().any(|e| e.contains("password")));
    }

    #[test]
    fn blank_role_falls_back_to_the_default() {
        let mut req = request("Ada", "Lovelace", "correct horse");
        req.role = Some("   ".to_string());

        assert_eq!(req.validate().expect("should be valid").role, "user");
    }

    #[test]
    fn api_keys_are_prefixed_and_unique() {
        let first = generate_api_key();
        let second = generate_api_key();

        assert!(first.starts_with(API_KEY_PREFIX));
        assert_ne!(first, second);
    }

    #[test]
    fn hashes_are_salted_per_call() {
        let first = hash_password("correct horse").expect("hashing works");
        let second = hash_password("correct horse").expect("hashing works");

        assert!(first.starts_with("$argon2id$"));
        assert_ne!(first, second);
    }
}
