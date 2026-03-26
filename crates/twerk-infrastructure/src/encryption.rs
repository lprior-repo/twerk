//! AES-GCM encryption module with SHA256 key derivation.
//!
//! # Example
//!
//! ```
//! use twerk_infrastructure::encryption::{encrypt, decrypt};
//!
//! let ciphertext = encrypt("hello", "secret").unwrap();
//! let plaintext = decrypt(&ciphertext, "secret").unwrap();
//! assert_eq!(plaintext, "hello");
//! ```

use aes_gcm::{
    aead::{Aead, KeyInit, Payload},
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
    let result = hasher.finalize();
    let mut key = [0u8; 32];
    key.copy_from_slice(&result);
    key
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

    let nonce_bytes: [u8; NONCE_LEN] = rand::random();
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|_| EncryptError::EncryptionError)?;

    // Prepend nonce to ciphertext for storage/transmission
    let mut result = Vec::with_capacity(NONCE_LEN + ciphertext.len());
    result.extend_from_slice(&nonce_bytes);
    result.extend_from_slice(&ciphertext);

    Ok(STANDARD.encode(&result))
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

    let (nonce_bytes, ciphertext_bytes) = data.split_at(nonce_size);

    let nonce = Nonce::from_slice(nonce_bytes);
    let payload = Payload {
        msg: ciphertext_bytes,
        aad: &[],
    };
    let plaintext = gcm
        .decrypt(nonce, payload)
        .map_err(|_| EncryptError::DecryptionError)?;

    String::from_utf8(plaintext).map_err(|_| EncryptError::DecryptionError)
}
