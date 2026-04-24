//! Encryption utilities for secrets.

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use argon2::{Argon2, PasswordHasher, password_hash::SaltString};
use base64::Engine;
use std::collections::HashMap;

use super::super::Error;

const SALT_LEN: usize = 16;
const KEY_LEN: usize = 32;

fn derive_key(passphrase: &str, salt: &[u8]) -> Result<[u8; KEY_LEN], Error> {
    let salt_str = SaltString::encode_b64(salt)
        .map_err(|e| Error::Encryption(format!("failed to encode salt: {e}")))?;

    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(passphrase.as_bytes(), &salt_str)
        .map_err(|e| Error::Encryption(format!("argon2 hash failed: {e}")))?;

    let hash_str = hash.hash.ok_or_else(|| Error::Encryption("no hash output".to_string()))?;
    let hash_bytes = hash_str.as_bytes();

    if hash_bytes.len() < KEY_LEN {
        return Err(Error::Encryption("derived key too short".to_string()));
    }

    let mut key = [0u8; KEY_LEN];
    key.copy_from_slice(&hash_bytes[..KEY_LEN]);
    Ok(key)
}

/// Encrypts a plaintext string using AES-256-GCM with Argon2id key derivation.
pub fn encrypt(plaintext: &str, key: &str) -> Result<String, Error> {
    let salt_bytes: [u8; SALT_LEN] = rand::random();
    let derived_key = derive_key(key, &salt_bytes)?;

    let cipher = Aes256Gcm::new_from_slice(derived_key.as_slice())
        .map_err(|e| Error::Encryption(format!("failed to create cipher: {e}")))?;

    let nonce_bytes: [u8; 12] = rand::random();
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| Error::Encryption(format!("encryption failed: {e}")))?;

    let mut combined = salt_bytes.to_vec();
    combined.extend(nonce_bytes.to_vec());
    combined.extend(ciphertext);

    Ok(base64::engine::general_purpose::STANDARD.encode(combined))
}

/// Decrypts a ciphertext string using AES-256-GCM with Argon2id key derivation.
pub fn decrypt(ciphertext: &str, key: &str) -> Result<String, Error> {
    let data = base64::engine::general_purpose::STANDARD
        .decode(ciphertext)
        .map_err(|e| Error::Encryption(format!("failed to decode base64: {e}")))?;

    if data.len() < SALT_LEN + 12 {
        return Err(Error::Encryption("ciphertext too short".to_string()));
    }

    let (salt_bytes, rest) = data.split_at(SALT_LEN);
    let (nonce_bytes, ciphertext_bytes) = rest.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);

    let derived_key = derive_key(key, salt_bytes)?;
    let cipher = Aes256Gcm::new_from_slice(derived_key.as_slice())
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
