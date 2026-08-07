use super::*;

fn request(first: &str, last: &str, password: &str) -> InsertUserRequest {
    InsertUserRequest {
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
