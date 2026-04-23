//! Encryption utilities for secrets.

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use base64::Engine;
use std::collections::HashMap;

use super::super::Error;

/// Derives a 256-bit key from a passphrase using SHA-256.
fn derive_key(passphrase: &str) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(passphrase.as_bytes());
    let result = hasher.finalize();
    let mut key = [0u8; 32];
    key.copy_from_slice(&result);
    key
}

/// Encrypts a plaintext string using AES-256-GCM.
pub fn encrypt(plaintext: &str, key: &str) -> Result<String, Error> {
    let cipher = Aes256Gcm::new_from_slice(derive_key(key).as_slice())
        .map_err(|e| Error::Encryption(format!("failed to create cipher: {e}")))?;

    let nonce_bytes: [u8; 12] = rand::random();
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| Error::Encryption(format!("encryption failed: {e}")))?;

    let mut combined = nonce_bytes.to_vec();
    combined.extend(ciphertext);

    Ok(base64::engine::general_purpose::STANDARD.encode(combined))
}

/// Decrypts a ciphertext string using AES-256-GCM.
pub fn decrypt(ciphertext: &str, key: &str) -> Result<String, Error> {
    let data = base64::engine::general_purpose::STANDARD
        .decode(ciphertext)
        .map_err(|e| Error::Encryption(format!("failed to decode base64: {e}")))?;

    if data.len() < 12 {
        return Err(Error::Encryption("ciphertext too short".to_string()));
    }

    let (nonce_bytes, ciphertext_bytes) = data.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);

    let cipher = Aes256Gcm::new_from_slice(derive_key(key).as_slice())
        .map_err(|e| Error::Encryption(format!("failed to create cipher: {e}")))?;

    let plaintext = cipher
        .decrypt(nonce, ciphertext_bytes)
        .map_err(|e| Error::Encryption(format!("decryption failed: {e}")))?;

    String::from_utf8(plaintext).map_err(|e| Error::Encryption(format!("invalid utf8: {e}")))
}

/// Encrypts secrets map for storage.
pub fn encrypt_secrets(
    secrets: &HashMap<String, String>,
    encryption_key: Option<&str>,
) -> Result<HashMap<String, String>, Error> {
    match encryption_key {
        None => Ok(secrets.clone()),
        Some(key) => secrets
            .iter()
            .map(|(k, v)| {
                let encrypted = encrypt(v, key)?;
                Ok((k.clone(), format!("enc:{encrypted}")))
            })
            .collect(),
    }
}

/// Decrypts secrets map from storage.
pub fn decrypt_secrets(
    secrets: &HashMap<String, String>,
    encryption_key: Option<&str>,
) -> Result<HashMap<String, String>, Error> {
    let mut result = HashMap::new();
    for (k, v) in secrets {
        if !v.starts_with("enc:") {
            result.insert(k.clone(), v.clone());
            continue;
        }
        match encryption_key {
            None => {
                result.insert(k.clone(), "[encrypted]".to_string());
            }
            Some(key) => {
                let ciphertext = v.trim_start_matches("enc:");
                let decrypted = decrypt(ciphertext, key)?;
                result.insert(k.clone(), decrypted);
            }
        }
    }
    Ok(result)
}
