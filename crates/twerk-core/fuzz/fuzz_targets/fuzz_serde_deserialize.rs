//! Fuzz target for serde deserialization of domain types.
//!
//! This target fuzzes JSON deserialization of WebhookUrl, Hostname, and CronExpression
//! to ensure deserialization never panics and validation is properly applied.
//!
//! Risk: Deserialization panic, validation bypass, memory issues
//! Corpus seeds: ["\"https://example.com\"", "\"localhost\"", "\"0 0 * * *\""]

#![no_main]

use libfuzzer_sys::fuzz_target;
use twerk_core::domain::{CronExpression, Hostname, WebhookUrl};

fuzz_target!(|data: &str| {
    // Must not panic — any JSON string is valid to attempt deserialization
    // WebhookUrl
    let _: Result<WebhookUrl, _> = serde_json::from_str(data);
    // Hostname
    let _: Result<Hostname, _> = serde_json::from_str(data);
    // CronExpression
    let _: Result<CronExpression, _> = serde_json::from_str(data);
});
