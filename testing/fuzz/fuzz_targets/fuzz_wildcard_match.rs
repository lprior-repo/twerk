#![no_main]

use libfuzzer_sys::fuzz_target;
use twerk_common::wildcard_match;

fuzz_target!(|data: &[u8]| {
    let s = String::from_utf8_lossy(data);
    let parts: Vec<&str> = s.split('\0').collect();
    if parts.len() >= 2 {
        let _ = wildcard_match(parts[0], parts[1]);
    }
});
