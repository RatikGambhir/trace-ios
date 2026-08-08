//! Validation helpers shared by every request type.
//!
//! They all collect into a caller-owned error list rather than returning on
//! the first problem, which is what lets one response name every fault in a
//! request at once — across every segment of a journey, not just the first.

use serde_json::{json, Value};

/// Join a nesting prefix to a field name. A flight now arrives inside a journey
/// segment, so its errors have to say *which* segment: `segments[2].flight.origin`
/// rather than a bare `origin`.
pub fn field(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_string()
    } else {
        format!("{prefix}.{name}")
    }
}

/// Normalise a fixed-width alphanumeric code (IATA, ICAO, ISO country) and
/// record a message if it is the wrong shape. Returns the normalised value
/// either way so validation can carry on and report every problem at once.
pub fn fixed_code(value: &str, field: &str, len: usize, errors: &mut Vec<String>) -> String {
    let code = value.trim().to_ascii_uppercase();

    if code.chars().count() != len || !code.chars().all(|c| c.is_ascii_alphanumeric()) {
        errors.push(format!("{field} must be {len} alphanumeric characters"));
    }

    code
}

/// Trim an optional string, treat blank as absent, and length-check the rest.
pub fn optional_text(
    value: Option<String>,
    field: &str,
    max: usize,
    errors: &mut Vec<String>,
) -> Option<String> {
    let value = value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())?;

    if value.chars().count() > max {
        errors.push(format!("{field} must be at most {max} characters"));
    }

    Some(value)
}

/// Normalise an optional enum-ish string against its allowed set, falling back
/// to `default` when absent.
pub fn one_of(
    value: Option<String>,
    field: &str,
    allowed: &[&str],
    default: &str,
    errors: &mut Vec<String>,
) -> String {
    let value = value
        .map(|v| v.trim().to_ascii_lowercase())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| default.to_string());

    if !allowed.contains(&value.as_str()) {
        errors.push(format!("{field} must be one of: {}", allowed.join(", ")));
    }

    value
}

/// Metadata has to be a JSON object — an array or a bare scalar in a column
/// meant for key/value extras is almost always a client bug.
pub fn object_or_default(value: Option<Value>, field: &str, errors: &mut Vec<String>) -> Value {
    match value {
        None | Some(Value::Null) => json!({}),
        Some(value) if value.is_object() => value,
        Some(_) => {
            errors.push(format!("{field} must be a JSON object"));
            json!({})
        }
    }
}
