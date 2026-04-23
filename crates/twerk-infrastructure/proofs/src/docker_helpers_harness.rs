//! Kani proof harnesses for `runtime::docker::helpers`.
//!
//! Covers:
//! - `parse_go_duration`: unit parsing for s, h, d and empty-string rejection
//! - `parse_memory_bytes`: unit parsing for GB, MB, plain integers, and invalid input
//! - `slugify`: output lowercase, no special characters, empty-input identity

use std::time::Duration;

use twerk_infrastructure::runtime::docker::helpers::{
    parse_go_duration, parse_memory_bytes, slugify,
};

// ---------------------------------------------------------------------------
// parse_go_duration
// ---------------------------------------------------------------------------

#[kani::proof]
fn parse_go_duration_5s_is_5_seconds() {
    let result = parse_go_duration("5s").unwrap();
    assert_eq!(result, Duration::from_secs(5));
}

#[kani::proof]
fn parse_go_duration_1h_is_3600_seconds() {
    let result = parse_go_duration("1h").unwrap();
    assert_eq!(result, Duration::from_secs(3600));
}

#[kani::proof]
fn parse_go_duration_1d_is_err() {
    // 'd' is not a supported unit in parse_go_duration (only h, m, s are valid)
    let result = parse_go_duration("1d");
    assert!(result.is_err(), "1d should fail — 'd' is not a supported unit");
}

#[kani::proof]
fn parse_go_duration_10m_is_600_seconds() {
    let result = parse_go_duration("10m").unwrap();
    assert_eq!(result, Duration::from_secs(600));
}

#[kani::proof]
fn parse_go_duration_empty_is_err() {
    let result = parse_go_duration("");
    assert!(result.is_err(), "empty string should not parse as a valid duration");
}

// ---------------------------------------------------------------------------
// parse_memory_bytes
// ---------------------------------------------------------------------------

#[kani::proof]
fn parse_memory_bytes_1gb() {
    let result = parse_memory_bytes("1GB").unwrap();
    assert_eq!(result, 1_073_741_824_i64);
}

#[kani::proof]
fn parse_memory_bytes_512mb() {
    let result = parse_memory_bytes("512MB").unwrap();
    assert_eq!(result, 536_870_912_i64);
}

#[kani::proof]
fn parse_memory_bytes_plain_integer() {
    let result = parse_memory_bytes("1024").unwrap();
    assert_eq!(result, 1024_i64);
}

#[kani::proof]
fn parse_memory_bytes_invalid_returns_err() {
    let result = parse_memory_bytes("abc");
    assert!(result.is_err(), "non-numeric input without unit should fail");
}

// ---------------------------------------------------------------------------
// slugify
// ---------------------------------------------------------------------------

#[kani::proof]
fn slugify_lowercase() {
    let input = "Hello WORLD 123";
    let result = slugify(input);
    assert!(
        result.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
        "slugify output must be lowercase (or digits / dashes): got '{result}'"
    );
}

#[kani::proof]
fn slugify_no_special_chars() {
    let input = "foo@bar!baz#qux$";
    let result = slugify(input);
    assert!(
        result.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
        "slugify output must only contain [a-z0-9-]: got '{result}'"
    );
}

#[kani::proof]
fn slugify_empty_input() {
    let result = slugify("");
    assert!(result.is_empty(), "empty input should produce empty output");
}
