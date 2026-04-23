#![no_main]

use libfuzzer_sys::fuzz_target;
use twerk_infrastructure::datastore::postgres::encrypt::{decrypt, encrypt};

fuzz_target!(|data: &[u8]| {
    // Split input on first null byte: key || \0 || plaintext
    let s = String::from_utf8_lossy(data);
    let parts: Vec<&str> = s.splitn(2, '\0').collect();
    if parts.len() < 2 || parts[0].is_empty() || parts[1].is_empty() {
        return;
    }
    let key = parts[0];
    let plaintext = parts[1];

    if let Ok(ciphertext) = encrypt(plaintext, key) {
        if let Ok(decrypted) = decrypt(&ciphertext, key) {
            assert_eq!(decrypted, plaintext);
        }
    }
});
