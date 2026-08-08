use super::*;

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
