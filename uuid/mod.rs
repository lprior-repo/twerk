//! UUID generation utilities
//!
//! Provides functions for creating standard and short UUIDs.
//! Parity with Go's `internal/uuid/uuid.go`:
//! - `new_uuid` → `NewUUID()` — stripped hyphens, 32-char hex
//! - `new_short_uuid` → `NewShortUUID()` — base57-encoded via `lithammer/shortuuid/v4`

/// Base57 alphabet — matches Go's `lithammer/shortuuid/v4` exactly.
///
/// Excludes ambiguous characters: 0, 1, I, O, l.
/// Total: 62 − 5 = 57 characters.
const BASE57_ALPHABET: &[u8] = b"23456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

/// Encodes 16 UUID bytes into a base57 string, matching Go's
/// `lithammer/shortuuid/v4` encoding exactly.
///
/// Algorithm: treat UUID as big-endian u128, repeatedly divide by 57
/// collecting remainders (LSB first), then reverse to get MSB-first order,
/// and pad with the first alphabet character to 22 characters.
///
/// Proof: 57²¹ < 2¹²⁸ < 57²², so any UUID fits in 22 base57 digits.
#[must_use]
fn encode_base57(bytes: &[u8; 16]) -> String {
    let mut value = u128::from_be_bytes(*bytes);
    let mut digits: Vec<u8> = Vec::new();

    // Collect base57 digits least-significant first (matches Go's big.Int loop)
    while value > 0 {
        let remainder = (value % 57) as usize;
        digits.push(BASE57_ALPHABET[remainder]);
        value /= 57;
    }

    // Pad to 22 characters with the first alphabet character (matches Go)
    let padding = 22usize.saturating_sub(digits.len());
    std::iter::repeat(BASE57_ALPHABET[0])
        .take(padding)
        .chain(digits.into_iter().rev())
        .map(|b| b as char)
        .collect()
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
    fn test_new_short_uuid_length_and_uniqueness() {
        let mut ids = std::collections::HashMap::new();
        for _ in 0..100 {
            let uid = new_short_uuid();
            assert_eq!(22, uid.len());
            // Verify all characters are from the base57 alphabet
            for ch in uid.bytes() {
                assert!(BASE57_ALPHABET.contains(&ch), "invalid char: {ch}");
            }
            ids.insert(uid.clone(), uid);
        }
        assert_eq!(100, ids.len());
    }

    #[test]
    fn test_base57_alphabet_is_57_chars() {
        assert_eq!(57, BASE57_ALPHABET.len());
    }

    #[test]
    fn test_encode_base57_deterministic() {
        // Same input bytes must produce same output
        let bytes = [0xFF; 16];
        let a = encode_base57(&bytes);
        let b = encode_base57(&bytes);
        assert_eq!(a, b);
        assert_eq!(22, a.len());
    }

    #[test]
    fn test_encode_base57_zero_bytes_padded() {
        // All-zero UUID should be all padding character
        let bytes = [0u8; 16];
        let encoded = encode_base57(&bytes);
        assert_eq!(22, encoded.len());
        // All-zero UUID encodes to all first-alphabet-char (with Go's algorithm
        // the while loop doesn't execute, so result is 22 padding chars)
        assert!(encoded.chars().all(|c| c == BASE57_ALPHABET[0] as char));
    }
}
