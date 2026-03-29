//! Encryption module using AES-GCM.
//!
//! Provides password-based key derivation and symmetric encryption/decryption.

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Errors that can occur during encryption operations.
#[derive(Debug, Error)]
pub enum EncryptError {
    #[error("cipher creation failed: {0}")]
    CipherCreation(String),
    #[error("GCM creation failed: {0}")]
    GcmCreation(String),
    #[error("encryption failed: {0}")]
    EncryptionFailed(String),
    #[error("decryption failed: {0}")]
    DecryptionFailed(String),
    #[error("invalid ciphertext: ciphertext too short")]
    InvalidCiphertext,
    #[error("base64 decoding failed: {0}")]
    Base64Decode(#[from] base64::DecodeError),
}

/// Derives a 256-bit key from a password using SHA-256.
///
/// # Arguments
/// * `password` - The password to derive the key from
///
/// # Returns
/// A 32-byte key suitable for AES-256
pub fn derive_key(password: &str) -> [u8; 32] {
    Sha256::digest(password).into()
}

/// Encrypts plaintext using AES-256-GCM.
///
/// The ciphertext is base64-encoded and includes the nonce prepended.
///
/// # Arguments
/// * `plaintext` - The data to encrypt
/// * `key` - A 32-byte key derived from a password
///
/// # Returns
/// Base64-encoded ciphertext with prepended nonce, or an error
pub fn encrypt(plaintext: &[u8], key: &[u8]) -> Result<Vec<u8>, EncryptError> {
    let _key_array: [u8; 32] = key
        .try_into()
        .map_err(|e| EncryptError::CipherCreation(format!("invalid key length: {}", e)))?;

    let cipher =
        Aes256Gcm::new_from_slice(key).map_err(|e| EncryptError::CipherCreation(e.to_string()))?;

    let nonce_bytes: [u8; 12] = rand::random();
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| EncryptError::EncryptionFailed(e.to_string()))?;

    let mut result = nonce_bytes.to_vec();
    result.extend(ciphertext);
    Ok(result)
}

/// Decrypts ciphertext using AES-256-GCM.
///
/// Expects base64-decoded ciphertext with the nonce prepended.
///
/// # Arguments
/// * `ciphertext` - The encrypted data (nonce + ciphertext)
/// * `key` - A 32-byte key derived from a password
///
/// # Returns
/// The decrypted plaintext, or an error
pub fn decrypt(ciphertext: &[u8], key: &[u8]) -> Result<Vec<u8>, EncryptError> {
    let _key_array: [u8; 32] = key
        .try_into()
        .map_err(|e| EncryptError::CipherCreation(format!("invalid key length: {}", e)))?;

    let cipher =
        Aes256Gcm::new_from_slice(key).map_err(|e| EncryptError::CipherCreation(e.to_string()))?;

    const NONCE_SIZE: usize = 12;

    if ciphertext.len() < NONCE_SIZE {
        return Err(EncryptError::InvalidCiphertext);
    }

    let (nonce_bytes, actual_ciphertext) = ciphertext.split_at(NONCE_SIZE);
    let nonce = Nonce::from_slice(nonce_bytes);

    cipher
        .decrypt(nonce, actual_ciphertext)
        .map_err(|e| EncryptError::DecryptionFailed(e.to_string()))
}

/// Encrypts a string and returns base64-encoded ciphertext.
///
/// # Arguments
/// * `plaintext` - The string to encrypt
/// * `password` - The password to derive the key from
///
/// # Returns
/// Base64-encoded ciphertext, or an error
pub fn encrypt_string(plaintext: &str, password: &str) -> Result<String, EncryptError> {
    let key = derive_key(password);
    encrypt(plaintext.as_bytes(), &key).map(|ct| BASE64.encode(ct))
}

/// Decrypts a base64-encoded ciphertext string.
///
/// # Arguments
/// * `ciphertext` - Base64-encoded ciphertext
/// * `password` - The password used during encryption
///
/// # Returns
/// The decrypted string, or an error
pub fn decrypt_string(ciphertext: &str, password: &str) -> Result<String, EncryptError> {
    let key = derive_key(password);
    let decoded = BASE64.decode(ciphertext)?;
    decrypt(&decoded, &key)
        .map_err(|e| EncryptError::DecryptionFailed(e.to_string()))?
        .try_into()
        .map_err(|_| EncryptError::DecryptionFailed("invalid UTF-8 in decrypted data".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let password = "test_password_123";
        let plaintext = "Hello, World!";

        let ciphertext = encrypt_string(plaintext, password).expect("encrypt should succeed");
        let decrypted = decrypt_string(&ciphertext, password).expect("decrypt should succeed");

        assert_eq!(plaintext, decrypted);
    }

    #[test]
    fn test_different_passwords_produce_different_ciphertext() {
        let password1 = "password1";
        let password2 = "password2";
        let plaintext = "Secret message";

        let ct1 = encrypt_string(plaintext, password1).expect("encrypt should succeed");
        let ct2 = encrypt_string(plaintext, password2).expect("encrypt should succeed");

        assert_ne!(ct1, ct2);
    }

    #[test]
    fn test_decrypt_with_wrong_password_fails() {
        let password = "correct_password";
        let wrong_password = "wrong_password";
        let plaintext = "Secret data";

        let ciphertext = encrypt_string(plaintext, password).expect("encrypt should succeed");
        let result = decrypt_string(&ciphertext, wrong_password);

        assert!(result.is_err());
    }

    #[test]
    fn test_derive_key_produces_32_bytes() {
        let key = derive_key("any_password");
        assert_eq!(key.len(), 32);
    }

    #[test]
    fn test_same_password_produces_same_key() {
        let password = "consistent_password";
        let key1 = derive_key(password);
        let key2 = derive_key(password);
        assert_eq!(key1, key2);
    }
}
