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

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    #![allow(clippy::expect_used)]
    use super::*;

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let plaintext = "secret password";
        let key = "my-secret-key";

        let encrypted = encrypt(plaintext, key).expect("encryption should succeed");
        let decrypted = decrypt(&encrypted, key).expect("decryption should succeed");

        assert_eq!(plaintext, decrypted);
    }

    #[test]
    fn test_encrypt_secrets_with_key() {
        let mut secrets = HashMap::new();
        secrets.insert("password".to_string(), "secret123".to_string());

        let encrypted = encrypt_secrets(&secrets, Some("key")).expect("encryption should succeed");

        assert!(encrypted.get("password").unwrap().starts_with("enc:"));
    }

    #[test]
    fn test_encrypt_secrets_without_key() {
        let mut secrets = HashMap::new();
        secrets.insert("password".to_string(), "secret123".to_string());

        let result = encrypt_secrets(&secrets, None).expect("should not fail");

        assert_eq!(result.get("password").unwrap(), "secret123");
    }

    #[test]
    fn test_decrypt_secrets_without_key() {
        let mut secrets = HashMap::new();
        secrets.insert("password".to_string(), "enc:xyz".to_string());

        let result = decrypt_secrets(&secrets, None).expect("should not fail");

        assert_eq!(result.get("password").unwrap(), "[encrypted]");
    }

    #[test]
    fn test_encrypt_different_keys_produce_different_ciphertext() {
        let plaintext = "secret password";
        let key_a = "key-alpha";
        let key_b = "key-beta";

        let enc_a = encrypt(plaintext, key_a).expect("encryption should succeed");
        let enc_b = encrypt(plaintext, key_b).expect("encryption should succeed");

        // Same plaintext, different keys → different ciphertext
        assert_ne!(enc_a, enc_b);

        // Each decrypts correctly with its own key
        let dec_a = decrypt(&enc_a, key_a).expect("decryption with key_a should succeed");
        let dec_b = decrypt(&enc_b, key_b).expect("decryption with key_b should succeed");
        assert_eq!(dec_a, plaintext);
        assert_eq!(dec_b, plaintext);
    }

    #[test]
    fn test_decrypt_with_wrong_key_fails() {
        let plaintext = "secret password";
        let enc = encrypt(plaintext, "correct-key").expect("encryption should succeed");

        let result = decrypt(&enc, "wrong-key");
        assert!(result.is_err());
    }

    #[test]
    fn test_decrypt_invalid_base64_fails() {
        let result = decrypt("not-valid-base64!!!", "key");
        assert!(result.is_err());
    }

    #[test]
    fn test_decrypt_short_ciphertext_fails() {
        // base64 of less than 12 bytes
        let short = base64::engine::general_purpose::STANDARD.encode([0u8; 8]);
        let result = decrypt(&short, "key");
        assert!(result.is_err());
    }

    #[test]
    fn test_encrypt_empty_string() {
        let enc = encrypt("", "key").expect("encryption should succeed");
        let dec = decrypt(&enc, "key").expect("decryption should succeed");
        assert_eq!(dec, "");
    }

    #[test]
    fn test_encrypt_secrets_empty_map() {
        let secrets = HashMap::new();
        let result = encrypt_secrets(&secrets, Some("key")).expect("should not fail");
        assert!(result.is_empty());
    }

    #[test]
    fn test_decrypt_secrets_empty_map() {
        let secrets = HashMap::new();
        let result = decrypt_secrets(&secrets, Some("key")).expect("should not fail");
        assert!(result.is_empty());
    }

    #[test]
    fn test_encrypt_decrypt_secrets_roundtrip() {
        let mut secrets = HashMap::new();
        secrets.insert("DB_PASSWORD".to_string(), "super_secret_123".to_string());
        secrets.insert("API_KEY".to_string(), "key-abc-xyz".to_string());
        secrets.insert("TOKEN".to_string(), "".to_string());

        let encrypted =
            encrypt_secrets(&secrets, Some("my-key")).expect("encryption should succeed");

        // All values should have enc: prefix
        for val in encrypted.values() {
            assert!(
                val.starts_with("enc:"),
                "encrypted value should start with enc:"
            );
        }

        let decrypted =
            decrypt_secrets(&encrypted, Some("my-key")).expect("decryption should succeed");

        assert_eq!(
            decrypted.get("DB_PASSWORD").map(String::as_str),
            Some("super_secret_123")
        );
        assert_eq!(
            decrypted.get("API_KEY").map(String::as_str),
            Some("key-abc-xyz")
        );
        assert_eq!(decrypted.get("TOKEN").map(String::as_str), Some(""));
    }

    #[test]
    fn test_decrypt_secrets_mixed_encrypted_and_plain() {
        let mut secrets = HashMap::new();
        secrets.insert("encrypted_val".to_string(), "enc:fake".to_string());
        secrets.insert("plain_val".to_string(), "plaintext".to_string());

        // Without key: encrypted gets [encrypted], plain passes through
        let result = decrypt_secrets(&secrets, None).expect("should not fail");
        assert_eq!(
            result.get("encrypted_val").map(String::as_str),
            Some("[encrypted]")
        );
        assert_eq!(
            result.get("plain_val").map(String::as_str),
            Some("plaintext")
        );
    }

    #[test]
    fn test_encrypt_secrets_preserves_keys() {
        let mut secrets = HashMap::new();
        secrets.insert("KEY_ONE".to_string(), "val1".to_string());
        secrets.insert("KEY_TWO".to_string(), "val2".to_string());
        secrets.insert("KEY_THREE".to_string(), "val3".to_string());

        let encrypted = encrypt_secrets(&secrets, Some("key")).expect("should succeed");

        assert!(encrypted.contains_key("KEY_ONE"));
        assert!(encrypted.contains_key("KEY_TWO"));
        assert!(encrypted.contains_key("KEY_THREE"));
        assert_eq!(encrypted.len(), 3);
    }

    #[test]
    fn test_encrypt_long_secret() {
        let long_secret = "x".repeat(10_000);
        let enc = encrypt(&long_secret, "key").expect("encryption should succeed");
        let dec = decrypt(&enc, "key").expect("decryption should succeed");
        assert_eq!(dec.len(), 10_000);
        assert_eq!(dec, long_secret);
    }

    #[test]
    fn test_encrypt_special_characters() {
        let special = "p@$$w0rd!#\t\n\r\"'\\";
        let enc = encrypt(special, "key").expect("encryption should succeed");
        let dec = decrypt(&enc, "key").expect("decryption should succeed");
        assert_eq!(dec, special);
    }
}
