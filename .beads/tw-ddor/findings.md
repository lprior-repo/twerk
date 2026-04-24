# Findings: tw-ddor — CRITICAL Security: SHA-256 single-pass KDF with no salt

## Issue Summary

**Severity**: CRITICAL
**File**: `crates/twerk-infrastructure/src/datastore/postgres/encrypt.rs`
**Lines**: 13-21 (`derive_key` function)
**Status**: Vulnerable — needs immediate remediation

## Vulnerability Details

### Weak Key Derivation Function

```rust
fn derive_key(passphrase: &str) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(passphrase.as_bytes());
    let result = hasher.finalize();
    let mut key = [0u8; 32];
    key.copy_from_slice(&result);
    key
}
```

### Problems

1. **No Salt**: Rainbow table attacks are trivial
2. **Single SHA-256 Pass**: No computational cost to brute-force
3. **SHA-256 is fast by design**: Not suitable for password hashing (designed for speed, not memory hardness)
4. **No Iteration/Stretching**: One hash = trivial to compute millions of candidates per second

### Attack Scenario

Any encrypted secret using this KDF can be decrypted by:
1. Attacker obtains ciphertext (e.g., from database leak)
2. Attacker runs GPU-accelerated brute-force with SHA-256
3. Millions to billions of candidates per second tested
4. Typical user passwords cracked in minutes to hours

## Fix Recommendation

### Option 1: Argon2id (Recommended)

Argon2id is the winner of the Password Hashing Competition and provides:
- Memory-hardness (resistant to GPU/ASIC attacks)
- Time-cost scaling (iterations)
- Salt support (prevents rainbow tables)

### Option 2: PBKDF2-HMAC-SHA256 (Acceptable Alternative)

PBKDF2 with:
- Random 16-byte salt
- 100,000+ iterations minimum
- Stored alongside ciphertext

## Implementation Plan

1. Add `argon2` crate to workspace dependencies
2. Update `derive_key` to use Argon2id with random 16-byte salt
3. Change output format to include salt: `salt (16 bytes) + nonce (12 bytes) + ciphertext`
4. Update `decrypt` to detect legacy format (SHA-256 derived) vs new format (Argon2id)
5. Add migration path for existing encrypted data

## Backwards Compatibility

The `encrypt` function output format must change:
- **Before**: `nonce (12 bytes) + ciphertext`
- **After**: `salt (16 bytes) + nonce (12 bytes) + ciphertext`

Detection in `decrypt`:
- If data starts with 16-byte salt followed by valid Argon2id params → new format
- If data is 12+ bytes without salt prefix → legacy format (for migration warnings)

## Test Coverage

Existing tests in `tests/postgres_encrypt_test.rs` cover:
- Roundtrip encrypt/decrypt
- Wrong key rejection
- Invalid input handling
- Edge cases (empty strings, long secrets, special characters)

**Tests DO NOT verify security properties** — they only verify functionality.

## Files Affected

| File | Lines | Change |
|------|-------|--------|
| `encrypt.rs` | 13-21 | Replace `derive_key` with Argon2id |
| `encrypt.rs` | 24-38 | Update `encrypt` to include salt in output |
| `encrypt.rs` | 42-62 | Update `decrypt` to handle both formats |
| `Cargo.toml` | (twerk-infrastructure) | Add `argon2` dependency |

## Severity Justification

This is CRITICAL because:
1. All secrets encrypted with this function are trivially recoverable
2. Encryption is used for secrets storage (database passwords, API keys)
3. Exploit requires only ciphertext + brute-force
4. No special hardware or conditions needed

## Recommendation

**Fix immediately**. This vulnerability affects data-at-rest encryption for all secrets stored using this module. Even if the database itself is not compromised, any backup or snapshot containing encrypted values is vulnerable.
