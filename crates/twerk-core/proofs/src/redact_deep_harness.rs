use twerk_core::redact::{is_secret_key, Redacter};

// ---------------------------------------------------------------------------
// Redacter::wildcard is idempotent
// ---------------------------------------------------------------------------

#[kani::proof]
fn redacter_wildcard_idempotent() {
    let redacter = Redacter::new(vec!["SECRET".to_string()]);
    let input = "my_secret_value";
    let first = redacter.wildcard(input);
    let second = redacter.wildcard(&first);
    assert_eq!(
        first, second,
        "wildcard should be idempotent: applying twice produces same result"
    );
}

#[kani::proof]
fn redacter_wildcard_idempotent_multiple_keys() {
    let redacter = Redacter::new(vec![
        "SECRET".to_string(),
        "PASSWORD".to_string(),
        "TOKEN".to_string(),
    ]);
    let input = "secret=abc password=def token=ghi";
    let first = redacter.wildcard(input);
    let second = redacter.wildcard(&first);
    assert_eq!(
        first, second,
        "wildcard should be idempotent with multiple keys"
    );
}

// ---------------------------------------------------------------------------
// Redacter::wildcard is case-insensitive
// ---------------------------------------------------------------------------

#[kani::proof]
fn redacter_wildcard_case_insensitive_lowercase() {
    let redacter = Redacter::new(vec!["SECRET".to_string()]);
    let result = redacter.wildcard("my_secret_value");
    assert!(
        !result.contains("secret"),
        "Lowercase match should be redacted"
    );
    assert!(
        result.contains("[REDACTED]"),
        "Should contain REDACTED marker"
    );
}

#[kani::proof]
fn redacter_wildcard_case_insensitive_uppercase() {
    let redacter = Redacter::new(vec!["SECRET".to_string()]);
    let result = redacter.wildcard("my_SECRET_value");
    assert!(
        !result.contains("SECRET"),
        "Uppercase match should be redacted"
    );
}

#[kani::proof]
fn redacter_wildcard_case_insensitive_mixed() {
    let redacter = Redacter::new(vec!["SECRET".to_string()]);
    let result = redacter.wildcard("my_SeCrEt_value");
    assert!(
        !result.contains("eCrE"),
        "Mixed case match should be redacted"
    );
}

// ---------------------------------------------------------------------------
// Redacter::contains is case-insensitive
// ---------------------------------------------------------------------------

#[kani::proof]
fn redacter_contains_case_insensitive() {
    let redacter = Redacter::new(vec!["SECRET".to_string()]);
    assert!(redacter.contains("my_secret"), "lowercase should match");
    assert!(redacter.contains("my_SECRET"), "uppercase should match");
    assert!(redacter.contains("my_SeCrEt"), "mixed case should match");
    assert!(!redacter.contains("normal_field"), "no match should return false");
}

// ---------------------------------------------------------------------------
// Redacter::default_redacter uses default keys
// ---------------------------------------------------------------------------

#[kani::proof]
fn redacter_default_has_three_keys() {
    let redacter = Redacter::default_redacter();
    assert_eq!(redacter.keys().len(), 3, "Default redacter should have 3 keys");
}

// ---------------------------------------------------------------------------
// Redacter::wildcard with empty key is no-op
// ---------------------------------------------------------------------------

#[kani::proof]
fn redacter_wildcard_empty_key_noop() {
    let redacter = Redacter::new(vec![String::new()]);
    let input = "some text";
    let result = redacter.wildcard(input);
    assert_eq!(result, input, "Empty key should not change input");
}

// ---------------------------------------------------------------------------
// is_secret_key invariants
// ---------------------------------------------------------------------------

#[kani::proof]
fn is_secret_key_password_variants() {
    assert!(is_secret_key("password"), "lowercase password");
    assert!(is_secret_key("PASSWORD"), "uppercase PASSWORD");
    assert!(is_secret_key("db_password"), "prefixed password");
    assert!(is_secret_key("PASSWORD_HASH"), "containing PASSWORD");
}

#[kani::proof]
fn is_secret_key_secret_variants() {
    assert!(is_secret_key("secret"), "lowercase secret");
    assert!(is_secret_key("SECRET"), "uppercase SECRET");
    assert!(is_secret_key("my_secret_key"), "containing secret");
}

#[kani::proof]
fn is_secret_key_access_key_variants() {
    assert!(is_secret_key("ACCESS_KEY"), "uppercase ACCESS_KEY");
    assert!(is_secret_key("access_key"), "lowercase access_key");
    assert!(is_secret_key("my_access_key_here"), "containing ACCESS_KEY");
}

#[kani::proof]
fn is_secret_key_rejects_normal_keys() {
    assert!(!is_secret_key("username"), "username is not secret");
    assert!(!is_secret_key("host"), "host is not secret");
    assert!(!is_secret_key("port"), "port is not secret");
    assert!(!is_secret_key("database"), "database is not secret");
}
