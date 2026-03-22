//! # Hash Module
//!
//! Provides password hashing and verification functionality using bcrypt.

use thiserror::Error;

/// Errors that can occur during hash operations.
#[derive(Debug, Error)]
pub enum HashError {
    #[error("failed to generate hash: {0}")]
    GenerationError(String),

    #[error("failed to verify password: {0}")]
    VerificationError(String),
}

/// Generates a bcrypt hash of the given password.
///
/// # Arguments
///
/// * `password` - The plaintext password to hash
///
/// # Returns
///
/// A `Result` containing the hashed password string or a `HashError`
pub fn password(password: &str) -> Result<String, HashError> {
    bcrypt::hash(password, bcrypt::DEFAULT_COST)
        .map_err(|e| HashError::GenerationError(e.to_string()))
}

/// Verifies a password against a bcrypt hash.
///
/// # Arguments
///
/// * `password` - The plaintext password to verify
/// * `hash` - The bcrypt hash to verify against
///
/// # Returns
///
/// `true` if the password matches the hash, `false` otherwise
pub fn check_password_hash(password: &str, hash: &str) -> bool {
    bcrypt::verify(password, hash).is_ok_and(|r| r)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_password() {
        let hashed = password("1234");
        assert!(hashed.is_ok(), "bcrypt should generate hash with default cost");
        let hashed = hashed.unwrap();
        assert!(check_password_hash("1234", &hashed));
    }

    #[test]
    fn test_check_password_hash_wrong_password() {
        let hashed = password("1234");
        assert!(hashed.is_ok(), "bcrypt should generate hash with default cost");
        let hashed = hashed.unwrap();
        assert!(!check_password_hash("wrong", &hashed));
    }

    #[test]
    fn test_check_password_hash_empty_password() {
        // Empty password should still produce a valid hash
        let hashed = password("");
        assert!(hashed.is_ok(), "bcrypt should handle empty password");
    }

    #[test]
    fn test_check_password_hash_invalid_hash() {
        // Invalid hash format should return false, not panic
        assert!(!check_password_hash("password", "invalid_hash"));
    }
}
