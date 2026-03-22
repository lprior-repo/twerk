//! UUID generation utilities
//!
//! Provides functions for creating standard and short UUIDs.

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![forbid(unsafe_code)]

use thiserror::Error;

/// Errors that can occur during UUID operations.
#[derive(Debug, Error)]
pub enum UuidError {
    #[error("failed to generate UUID")]
    GenerationError,
}

/// Alphabet for base57 encoding (URL-safe, unambiguous characters)
const BASE57_ALPHABET: &[u8] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

/// Encodes a UUID bytes into a base57 string.
///
/// # Arguments
/// * `bytes` - 16 bytes of UUID data
///
/// # Returns
/// A 22-character base57 encoded string.
#[must_use]
fn encode_base57(bytes: &[u8; 16]) -> String {
    let mut result = String::with_capacity(22);

    // Convert 16 bytes to u128 (UUID is 128 bits)
    let mut value = u128::from_be_bytes(*bytes);

    // Encode 128 bits using base57 - need 22 chars since 57^21 < 2^128 < 57^22
    for _ in 0..22 {
        let index = (value % 57) as usize;
        result.push(BASE57_ALPHABET[index] as char);
        value /= 57;
    }

    result
}

/// Generates a new random UUID without hyphens.
///
/// # Returns
/// A 32-character hex string representing a `UUIDv4`.
#[must_use]
pub fn new_uuid() -> String {
    // uuid::Uuid returns a standard format with hyphens
    // We remove them to get a 32-character hex string
    uuid::Uuid::new_v4().to_string().replace('-', "")
}

/// Generates a new short UUID encoded with base57.
///
/// # Returns
/// A 22-character string representing a compact `UUIDv4`.
#[must_use]
pub fn new_short_uuid() -> String {
    let uuid = uuid::Uuid::new_v4();
    let uuid_bytes = uuid.as_bytes();
    encode_base57(uuid_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_uuid_length() {
        let id = new_uuid();
        assert_eq!(32, id.len());
    }

    #[test]
    fn test_new_short_uuid_length() {
        let mut ids = std::collections::HashMap::new();
        for _ in 0..100 {
            let uid = new_short_uuid();
            assert_eq!(22, uid.len());
            ids.insert(uid.clone(), uid);
        }
        assert_eq!(100, ids.len());
    }
}
