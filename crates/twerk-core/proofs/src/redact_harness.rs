use twerk_core::redact::is_secret_key;

#[kani::proof]
fn is_secret_key_detects_password() {
    assert!(
        is_secret_key("DB_PASSWORD"),
        "Keys containing 'PASSWORD' should be detected as secret"
    );
}

#[kani::proof]
fn is_secret_key_detects_secret() {
    assert!(
        is_secret_key("API_SECRET"),
        "Keys containing 'SECRET' should be detected as secret"
    );
}

#[kani::proof]
fn is_secret_key_allows_normal() {
    assert!(
        !is_secret_key("normal_field"),
        "Normal field names should not be detected as secret"
    );
}
