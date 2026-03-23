//! AES-GCM encryption module with SHA256 key derivation.
//!
//! # Example
//!
//! ```
//! use encrypt::{encrypt, decrypt};
//!
//! let ciphertext = encrypt("hello", "secret").unwrap();
//! let plaintext = decrypt(&ciphertext, "secret").unwrap();
//! assert_eq!(plaintext, "hello");
//! ```

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// AES-GCM nonce size in bytes (96 bits).
const NONCE_LEN: usize = 12;

/// Errors that can occur during encryption or decryption operations.
#[derive(Debug, Error)]
pub enum EncryptError {
    /// Failed to create the AES-GCM cipher from the derived key.
    #[error("cipher creation failed")]
    CipherError,

    /// Encryption operation failed.
    #[error("encryption failed")]
    EncryptionError,

    /// Decryption operation failed (wrong key or corrupted data).
    #[error("decryption failed")]
    DecryptionError,

    /// Ciphertext is too short to contain a valid nonce.
    #[error("invalid ciphertext")]
    InvalidCiphertext,

    /// Base64 decoding failed.
    #[error("base64 decode error")]
    Base64DecodeError,
}

/// Derives a 256-bit key from a passphrase using SHA256.
fn derive_key(passphrase: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(passphrase.as_bytes());
    hasher.finalize().into()
}

/// Encrypts plaintext using AES-256-GCM with a derived key.
///
/// The nonce is randomly generated and prepended to the ciphertext.
/// The result is base64-encoded for safe transport/storage.
///
/// # Arguments
///
/// * `plaintext` - The text to encrypt
/// * `key` - The passphrase used to derive the encryption key
///
/// # Returns
///
/// Base64-encoded ciphertext on success, or an error if encryption fails.
pub fn encrypt(plaintext: &str, key: &str) -> Result<String, EncryptError> {
    let key_bytes = derive_key(key);
    let cipher = Aes256Gcm::new_from_slice(&key_bytes).map_err(|_| EncryptError::CipherError)?;

    let mut nonce_bytes = [0u8; NONCE_LEN];
    OsRng.fill(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|_| EncryptError::EncryptionError)?;

    Ok(STANDARD.encode(&ciphertext))
}

/// Decrypts a base64-encoded ciphertext using AES-256-GCM.
///
/// # Arguments
///
/// * `ciphertext` - Base64-encoded ciphertext from [`encrypt`]
/// * `key` - The passphrase used to derive the encryption key
///
/// # Returns
///
/// The original plaintext on success, or an error if decryption fails
/// (wrong key, corrupted data, or invalid format).
pub fn decrypt(ciphertext: &str, key: &str) -> Result<String, EncryptError> {
    let data = STANDARD
        .decode(ciphertext)
        .map_err(|_| EncryptError::Base64DecodeError)?;

    let key_bytes = derive_key(key);
    let gcm = Aes256Gcm::new_from_slice(&key_bytes).map_err(|_| EncryptError::CipherError)?;

    let nonce_size = NONCE_LEN;
    if data.len() < nonce_size {
        return Err(EncryptError::InvalidCiphertext);
    }

    let (nonce, ciphertext_bytes) = data.split_at(nonce_size);

    let plaintext = gcm
        .open(ciphertext_bytes, nonce)
        .map_err(|_| EncryptError::DecryptionError)?;

    String::from_utf8(plaintext).map_err(|_| EncryptError::DecryptionError)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_produces_different_output() {
        let ciphertext = encrypt("hello", "secret").expect("encrypt should succeed");
        assert_ne!(
            ciphertext, "hello",
            "ciphertext should differ from plaintext"
        );
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let original = "hello";
        let key = "secret";

        let ciphertext = encrypt(original, key).expect("encrypt should succeed");
        let decrypted = decrypt(&ciphertext, key).expect("decrypt should succeed");

        assert_eq!(decrypted, original, "decrypted text should match original");
    }

    #[test]
    fn test_decrypt_with_wrong_key_fails() {
        let ciphertext = encrypt("hello", "secret").expect("encrypt should succeed");
        let result = decrypt(&ciphertext, "bad_secret");

        assert!(result.is_err(), "decrypt with wrong key should fail");
    }

    #[test]
    fn test_decrypt_with_wrong_key_returns_empty() {
        let ciphertext = encrypt("hello", "secret").expect("encrypt should succeed");
        let result = decrypt(&ciphertext, "bad_secret");

        assert!(result.is_err());
        if let Err(e) = result {
            assert_eq!(e.to_string(), "decryption failed");
        }
    }

    #[test]
    fn test_encrypt_different_messages_produce_different_ciphertexts() {
        let ciphertext1 = encrypt("message1", "secret").expect("encrypt should succeed");
        let ciphertext2 = encrypt("message2", "secret").expect("encrypt should succeed");

        assert_ne!(
            ciphertext1, ciphertext2,
            "different messages should produce different ciphertexts"
        );
    }

    #[test]
    fn test_encrypt_same_message_produces_different_ciphertexts() {
        // Due to random nonce generation, same message with same key
        // should produce different ciphertexts each time
        let ciphertext1 = encrypt("hello", "secret").expect("encrypt should succeed");
        let ciphertext2 = encrypt("hello", "secret").expect("encrypt should succeed");

        assert_ne!(
            ciphertext1, ciphertext2,
            "same message should produce different ciphertexts due to random nonce"
        );

        // But both should decrypt to the same plaintext
        let decrypted1 = decrypt(&ciphertext1, "secret").expect("decrypt should succeed");
        let decrypted2 = decrypt(&ciphertext2, "secret").expect("decrypt should succeed");

        assert_eq!(decrypted1, "hello");
        assert_eq!(decrypted2, "hello");
    }

    #[test]
    fn test_empty_plaintext() {
        let ciphertext = encrypt("", "secret").expect("encrypt should succeed");
        let decrypted = decrypt(&ciphertext, "secret").expect("decrypt should succeed");

        assert_eq!(
            decrypted, "",
            "empty plaintext should decrypt to empty string"
        );
    }

    #[test]
    fn test_unicode_plaintext() {
        let original = "Hello, 世界! 🦀";
        let ciphertext = encrypt(original, "secret").expect("encrypt should succeed");
        let decrypted = decrypt(&ciphertext, "secret").expect("decrypt should succeed");

        assert_eq!(
            decrypted, original,
            "unicode text should roundtrip correctly"
        );
    }

    #[test]
    fn test_long_plaintext() {
        let original = "A".repeat(10000);
        let ciphertext = encrypt(&original, "secret").expect("encrypt should succeed");
        let decrypted = decrypt(&ciphertext, "secret").expect("decrypt should succeed");

        assert_eq!(
            decrypted, original,
            "long plaintext should roundtrip correctly"
        );
    }

    #[test]
    fn test_invalid_base64_fails() {
        let result = decrypt("not-valid-base64!!!", "secret");

        assert!(result.is_err());
        if let Err(e) = result {
            assert_eq!(e.to_string(), "base64 decode error");
        }
    }

    #[test]
    fn test_short_ciphertext_fails() {
        // Less than NONCE_LEN bytes
        let result = decrypt("c2hvcnQ=", "secret"); // "short" base64 encoded

        assert!(result.is_err());
        if let Err(e) = result {
            assert_eq!(e.to_string(), "invalid ciphertext");
        }
    }

    #[test]
    fn test_derive_key_deterministic() {
        let key1 = derive_key("passphrase");
        let key2 = derive_key("passphrase");
        let key3 = derive_key("different");

        assert_eq!(key1, key2, "same passphrase should produce same key");
        assert_ne!(
            key1, key3,
            "different passphrase should produce different key"
        );
        assert_eq!(key1.len(), 32, "key should be 32 bytes (256 bits)");
    }
}
