//! Fuzz target for Hostname::new with arbitrary string input.
//!
//! This target fuzzes the Hostname constructor to ensure it never panics
//! on any arbitrary string input.
//!
//! Risk: Panic from regex/regex-lite failure, logic error in label validation
//! Corpus seeds: ["localhost", "example.com", "api.example.com", "host:8080", "", "a".repeat(300)]

#![no_main]

use libfuzzer_sys::fuzz_target;
use twerk_core::domain::Hostname;

fuzz_target!(|data: &str| {
    // Must not panic — any input is valid to attempt parsing
    let _ = Hostname::new(data);
});
