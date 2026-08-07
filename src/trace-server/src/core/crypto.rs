//! Password hashing and API-key generation.
//!
//! Primitives, not user logic: nothing here knows what a user is. They live
//! in `core` because any future credential-bearing thing needs the same two.

use argon2::{
    password_hash::{
        rand_core::{OsRng, RngCore},
        PasswordHasher, SaltString,
    },
    Argon2,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};

pub const API_KEY_PREFIX: &str = "trace_sk_";

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
#[path = "crypto_tests.rs"]
mod tests;
