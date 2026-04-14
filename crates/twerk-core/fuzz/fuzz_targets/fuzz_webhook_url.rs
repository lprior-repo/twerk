//! Fuzz target for WebhookUrl::new with arbitrary string input.
//!
//! This target fuzzes the WebhookUrl constructor to ensure it never panics
//! on any arbitrary string input.
//!
//! Risk: Panic from malformed URL parsing, logic error in scheme/host validation
//! Corpus seeds: ["https://example.com", "http://localhost:8080", "https://api.test.co:443/v1", "ftp://bad.com", ""]

#![no_main]

use libfuzzer_sys::fuzz_target;
use twerk_core::domain::WebhookUrl;

fuzz_target!(|data: &str| {
    // Must not panic — any input is valid to attempt parsing
    let _ = WebhookUrl::new(data);
});
