//! Password hashing utilities using bcrypt.

use bcrypt::{hash, verify, DEFAULT_COST};

/// Hashes a password using bcrypt with default cost.
/// Returns the hashed password string or an error.
pub fn password(password: &str) -> Result<String, bcrypt::BcryptError> {
    hash(password, DEFAULT_COST).map(String::from)
}

/// Verifies a password against a bcrypt hash.
/// Returns `true` if the password matches the hash, `false` otherwise.
pub fn check_password_hash(password: &str, hash: &str) -> bool {
    verify(password, hash).is_ok_and(|r| r)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_password_and_verify() {
        let password = "super_secret_password";
        let hashed = password(password).expect("hashing should succeed");
        assert_ne!(hashed, password, "hash should differ from original");
        assert!(check_password_hash(password, &hashed));
    }

    #[test]
    fn test_wrong_password_fails() {
        let password = "correct_password";
        let wrong = "wrong_password";
        let hashed = password(password).expect("hashing should succeed");
        assert!(!check_password_hash(wrong, &hashed));
    }
}
