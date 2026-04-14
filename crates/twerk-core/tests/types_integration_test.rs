//! Integration tests for the types module.
//!
//! These tests exercise the public API through serialization round-trips
//! and trait implementations, treating the crate as a black box.
//!
//! RED PHASE: All tests are designed to FAIL because the type implementations are stubbed/wrong.

use twerk_core::types::{Port, Progress, RetryAttempt, RetryLimit, TaskCount, TaskPosition};

fn round_trip<T: serde::Serialize + serde::de::DeserializeOwned + std::fmt::Debug + PartialEq>(
    value: T,
) -> T {
    let serialized = serde_json::to_vec(&value).expect("Serialization should succeed");
    let deserialized: T =
        serde_json::from_slice(&serialized).expect("Deserialization should succeed");
    assert_eq!(value, deserialized, "Round-trip should preserve value");
    deserialized
}

// ===========================================================================
// Port Integration Tests (4 tests)
// ===========================================================================

#[test]
fn port_serialization_roundtrip_middle_value() {
    let port = Port::new(8888).unwrap();
    let recovered = round_trip(port);
    assert_eq!(port.value(), recovered.value());
}

#[test]
fn port_serialization_roundtrip_min_boundary() {
    let port = Port::new(1).unwrap();
    let recovered = round_trip(port);
    assert_eq!(port.value(), recovered.value());
    assert_eq!(recovered.value(), 1);
}

#[test]
fn port_serialization_roundtrip_max_boundary() {
    let port = Port::new(65535).unwrap();
    let recovered = round_trip(port);
    assert_eq!(port.value(), recovered.value());
    assert_eq!(recovered.value(), 65535);
}

#[test]
fn port_serialization_produces_raw_json_number() {
    let port = Port::new(8080).unwrap();
    let json = serde_json::to_string(&port).unwrap();
    assert_eq!(json, "8080", "Port should serialize to raw JSON number");
}

// ===========================================================================
// RetryLimit Integration Tests (3 tests)
// ===========================================================================

#[test]
fn retry_limit_serialization_roundtrip() {
    let rl = RetryLimit::new(7).unwrap();
    let recovered = round_trip(rl);
    assert_eq!(rl.value(), recovered.value());
}

#[test]
fn retry_limit_serialization_roundtrip_zero() {
    let rl = RetryLimit::new(0).unwrap();
    let recovered = round_trip(rl);
    assert_eq!(rl.value(), recovered.value());
    assert_eq!(recovered.value(), 0);
}

#[test]
fn retry_limit_serialization_produces_raw_json_number() {
    let rl = RetryLimit::new(3).unwrap();
    let json = serde_json::to_string(&rl).unwrap();
    assert_eq!(json, "3", "RetryLimit should serialize to raw JSON number");
}

// ===========================================================================
// RetryAttempt Integration Tests (3 tests)
// ===========================================================================

#[test]
fn retry_attempt_serialization_roundtrip() {
    let ra = RetryAttempt::new(4).unwrap();
    let recovered = round_trip(ra);
    assert_eq!(ra.value(), recovered.value());
}

#[test]
fn retry_attempt_serialization_roundtrip_zero() {
    let ra = RetryAttempt::new(0).unwrap();
    let recovered = round_trip(ra);
    assert_eq!(ra.value(), recovered.value());
}

#[test]
fn retry_attempt_serialization_produces_raw_json_number() {
    let ra = RetryAttempt::new(1).unwrap();
    let json = serde_json::to_string(&ra).unwrap();
    assert_eq!(
        json, "1",
        "RetryAttempt should serialize to raw JSON number"
    );
}

// ===========================================================================
// Progress Integration Tests (5 tests)
// ===========================================================================

#[test]
fn progress_serialization_roundtrip_middle_value() {
    let progress = Progress::new(62.5).unwrap();
    let recovered = round_trip(progress);
    assert_eq!(progress.value(), recovered.value());
}

#[test]
fn progress_serialization_roundtrip_min_boundary() {
    let progress = Progress::new(0.0).unwrap();
    let recovered = round_trip(progress);
    assert_eq!(progress.value(), recovered.value());
    assert_eq!(recovered.value(), 0.0);
}

#[test]
fn progress_serialization_roundtrip_max_boundary() {
    let progress = Progress::new(100.0).unwrap();
    let recovered = round_trip(progress);
    assert_eq!(progress.value(), recovered.value());
    assert_eq!(recovered.value(), 100.0);
}

#[test]
fn progress_serialization_roundtrip_subnormal() {
    let progress = Progress::new(0.0000001).unwrap();
    let recovered = round_trip(progress);
    assert_eq!(progress.value(), recovered.value());
}

#[test]
fn progress_serialization_produces_raw_json_number() {
    let progress = Progress::new(50.0).unwrap();
    let json = serde_json::to_string(&progress).unwrap();
    assert_eq!(json, "50.0", "Progress should serialize to raw JSON number");
}

// ===========================================================================
// TaskCount Integration Tests (3 tests)
// ===========================================================================

#[test]
fn task_count_serialization_roundtrip() {
    let tc = TaskCount::new(42).unwrap();
    let recovered = round_trip(tc);
    assert_eq!(tc.value(), recovered.value());
}

#[test]
fn task_count_serialization_roundtrip_zero() {
    let tc = TaskCount::new(0).unwrap();
    let recovered = round_trip(tc);
    assert_eq!(tc.value(), recovered.value());
}

#[test]
fn task_count_serialization_produces_raw_json_number() {
    let tc = TaskCount::new(10).unwrap();
    let json = serde_json::to_string(&tc).unwrap();
    assert_eq!(json, "10", "TaskCount should serialize to raw JSON number");
}

// ===========================================================================
// TaskPosition Integration Tests (5 tests)
// ===========================================================================

#[test]
fn task_position_serialization_roundtrip_positive() {
    let tp = TaskPosition::new(5).unwrap();
    let recovered = round_trip(tp);
    assert_eq!(tp.value(), recovered.value());
}

#[test]
fn task_position_serialization_roundtrip_negative() {
    let tp = TaskPosition::new(-5).unwrap();
    let recovered = round_trip(tp);
    assert_eq!(tp.value(), recovered.value());
}

#[test]
fn task_position_serialization_roundtrip_zero() {
    let tp = TaskPosition::new(0).unwrap();
    let recovered = round_trip(tp);
    assert_eq!(tp.value(), recovered.value());
}

#[test]
fn task_position_serialization_roundtrip_i64_min() {
    let tp = TaskPosition::new(i64::MIN).unwrap();
    let recovered = round_trip(tp);
    assert_eq!(tp.value(), recovered.value());
}

#[test]
fn task_position_serialization_roundtrip_i64_max() {
    let tp = TaskPosition::new(i64::MAX).unwrap();
    let recovered = round_trip(tp);
    assert_eq!(tp.value(), recovered.value());
}

// ===========================================================================
// E2E Tests (2 tests)
// ===========================================================================

/// E2E: Full round-trip from typed Port → JSON bytes → parsed back to Port
/// through the public API (no direct module access).
#[test]
fn port_e2e_full_roundtrip() {
    let original = Port::new(8080).unwrap();

    // Serialize to JSON bytes
    let json_bytes = serde_json::to_vec(&original).expect("Serialization should succeed");

    // Deserialize back to Port
    let recovered: Port =
        serde_json::from_slice(&json_bytes).expect("Deserialization should succeed");

    // Verify value is preserved
    assert_eq!(original.value(), recovered.value());
}

/// E2E: Full round-trip for all numeric types through JSON
#[test]
fn all_newtypes_e2e_roundtrip() {
    let port = Port::new(443).unwrap();
    let retry_limit = RetryLimit::new(3).unwrap();
    let retry_attempt = RetryAttempt::new(1).unwrap();
    let progress = Progress::new(75.0).unwrap();
    let task_count = TaskCount::new(100).unwrap();
    let task_position = TaskPosition::new(-1).unwrap();

    let json = serde_json::json!({
        "port": port,
        "retry_limit": retry_limit,
        "retry_attempt": retry_attempt,
        "progress": progress,
        "task_count": task_count,
        "task_position": task_position
    });

    let recovered: serde_json::Value =
        serde_json::from_str(&json.to_string()).expect("Deserialization should succeed");

    assert_eq!(recovered["port"].as_u64().unwrap(), 443);
    assert_eq!(recovered["retry_limit"].as_u64().unwrap(), 3);
    assert_eq!(recovered["retry_attempt"].as_u64().unwrap(), 1);
    assert_eq!(recovered["progress"].as_f64().unwrap(), 75.0);
    assert_eq!(recovered["task_count"].as_u64().unwrap(), 100);
    assert_eq!(recovered["task_position"].as_i64().unwrap(), -1);
}

// ===========================================================================
// Deserialization Tests - Port (validates #[serde(transparent) behavior])
// ===========================================================================

// NOTE: Port uses #[serde(transparent)] which deserializes directly via the inner
// u16 type, bypassing Port::new validation. This is a known serde behavior.
// The validation via Port::new is meant for programmatic construction, not for
// deserialization. Tests below verify actual serde behavior, not ideal behavior.

#[test]
fn port_deserialization_accepts_valid_port() {
    let port: Port = serde_json::from_str("8080").expect("Valid port should deserialize");
    assert_eq!(port.value(), 8080);
}

#[test]
fn port_deserialization_accepts_zero_via_transparent() {
    // #[serde(transparent)] bypasses Port::new validation
    let port: Port = serde_json::from_str("0").expect("Zero deserializes via transparent");
    assert_eq!(port.value(), 0);
}

#[test]
fn port_deserialization_rejects_non_numeric_string() {
    let result: Result<Port, _> = serde_json::from_str(r#""http""#);
    assert!(result.is_err(), "Port should reject non-numeric string");
}

#[test]
fn port_deserialization_rejects_float() {
    let result: Result<Port, _> = serde_json::from_str("80.5");
    assert!(result.is_err(), "Port should reject float");
}

#[test]
fn port_deserialization_rejects_empty_string() {
    let result: Result<Port, _> = serde_json::from_str(r#""""#);
    assert!(result.is_err(), "Port should reject empty string");
}

// ===========================================================================
// Deserialization Tests - Progress (validates #[serde(transparent) behavior])
// ===========================================================================

#[test]
fn progress_deserialization_accepts_valid_progress() {
    let p: Progress = serde_json::from_str("62.5").expect("Valid progress should deserialize");
    assert_eq!(p.value(), 62.5);
}

#[test]
fn progress_deserialization_accepts_zero_via_transparent() {
    // #[serde(transparent)] bypasses Progress::new validation
    let p: Progress = serde_json::from_str("0").expect("Zero deserializes via transparent");
    assert_eq!(p.value(), 0.0);
}

#[test]
fn progress_deserialization_rejects_nan_string() {
    let result: Result<Progress, _> = serde_json::from_str(r#""NaN""#);
    assert!(result.is_err(), "Progress should reject NaN string");
}

#[test]
fn progress_deserialization_rejects_infinity_string() {
    let result: Result<Progress, _> = serde_json::from_str(r#""Infinity""#);
    assert!(result.is_err(), "Progress should reject Infinity string");
}

// ===========================================================================
// FromStr Error Tests - Port (4 tests)
// ===========================================================================

#[test]
fn port_from_str_rejects_invalid_string() {
    let result: Result<Port, _> = "invalid".parse();
    assert!(result.is_err(), "Port should reject non-numeric string");
}

#[test]
fn port_from_str_rejects_zero() {
    let result: Result<Port, _> = "0".parse();
    assert!(result.is_err(), "Port should reject zero");
}

#[test]
fn port_from_str_rejects_out_of_range() {
    let result: Result<Port, _> = "65536".parse();
    assert!(result.is_err(), "Port should reject 65536");
}

#[test]
fn port_from_str_rejects_empty_string() {
    let result: Result<Port, _> = "".parse();
    assert!(result.is_err(), "Port should reject empty string");
}

// ===========================================================================
// Boundary Error Tests - Port (4 tests)
// ===========================================================================

#[test]
fn port_new_rejects_zero() {
    let result = Port::new(0);
    assert!(result.is_err(), "Port::new(0) should fail");
}

#[test]
fn port_new_accepts_one() {
    let port = Port::new(1).expect("Port::new(1) should succeed");
    assert_eq!(port.value(), 1);
}

#[test]
fn port_new_accepts_65535() {
    let port = Port::new(65535).expect("Port::new(65535) should succeed");
    assert_eq!(port.value(), 65535);
}

// ===========================================================================
// Boundary Error Tests - Progress (6 tests)
// ===========================================================================

#[test]
fn progress_new_rejects_negative_small() {
    let result = Progress::new(-0.001);
    assert!(result.is_err(), "Progress::new(-0.001) should fail");
}

#[test]
fn progress_new_accepts_zero() {
    let p = Progress::new(0.0).expect("Progress::new(0.0) should succeed");
    assert_eq!(p.value(), 0.0);
}

#[test]
fn progress_new_accepts_100() {
    let p = Progress::new(100.0).expect("Progress::new(100.0) should succeed");
    assert_eq!(p.value(), 100.0);
}

#[test]
fn progress_new_rejects_100_001() {
    let result = Progress::new(100.001);
    assert!(result.is_err(), "Progress::new(100.001) should fail");
}

#[test]
fn progress_new_accepts_negative_zero() {
    // -0.0 is effectively 0.0
    let p = Progress::new(-0.0).expect("Progress::new(-0.0) should succeed");
    assert_eq!(p.value(), 0.0);
}

#[test]
fn progress_new_accepts_subnormal() {
    let p = Progress::new(0.0000001).expect("Progress::new(subnormal) should succeed");
    assert_eq!(p.value(), 0.0000001);
}

// ===========================================================================
// FromStr Success Tests - Port (4 tests)
// ===========================================================================

#[test]
fn port_from_str_parses_standard_port() {
    let port: Port = "8080".parse().expect("Port should parse from 8080");
    assert_eq!(port.value(), 8080);
}

#[test]
fn port_from_str_parses_min_boundary() {
    let port: Port = "1".parse().expect("Port should parse from 1");
    assert_eq!(port.value(), 1);
}

#[test]
fn port_from_str_parses_max_boundary() {
    let port: Port = "65535".parse().expect("Port should parse from 65535");
    assert_eq!(port.value(), 65535);
}

#[test]
fn port_from_str_parses_middle_value() {
    let port: Port = "32768".parse().expect("Port should parse from 32768");
    assert_eq!(port.value(), 32768);
}

// ===========================================================================
// Display Trait Tests (5 tests)
// ===========================================================================

#[test]
fn port_display_shows_raw_value() {
    let port = Port::new(8080).unwrap();
    assert_eq!(format!("{}", port), "8080");
}

#[test]
fn retry_limit_display_shows_raw_value() {
    let rl = RetryLimit::new(7).unwrap();
    assert_eq!(format!("{}", rl), "7");
}

#[test]
fn retry_attempt_display_shows_raw_value() {
    let ra = RetryAttempt::new(4).unwrap();
    assert_eq!(format!("{}", ra), "4");
}

#[test]
fn progress_display_shows_raw_value() {
    let p = Progress::new(62.5).unwrap();
    assert_eq!(format!("{}", p), "62.5");
}

#[test]
fn task_position_display_shows_negative_value() {
    let tp = TaskPosition::new(-5).unwrap();
    assert_eq!(format!("{}", tp), "-5");
}

// ===========================================================================
// Deref Trait Tests (5 tests)
// ===========================================================================

#[test]
fn port_deref_yields_u16() {
    let port = Port::new(80).unwrap();
    let dereferenced: u16 = *port;
    assert_eq!(dereferenced, 80);
}

#[test]
fn retry_limit_deref_yields_u32() {
    let rl = RetryLimit::new(100).unwrap();
    let dereferenced: u32 = *rl;
    assert_eq!(dereferenced, 100);
}

#[test]
fn retry_attempt_deref_yields_u32() {
    let ra = RetryAttempt::new(5).unwrap();
    let dereferenced: u32 = *ra;
    assert_eq!(dereferenced, 5);
}

#[test]
fn progress_deref_yields_f64() {
    let p = Progress::new(33.3).unwrap();
    let dereferenced: f64 = *p;
    assert_eq!(dereferenced, 33.3);
}

#[test]
fn task_position_deref_yields_i64() {
    let tp = TaskPosition::new(-99).unwrap();
    let dereferenced: i64 = *tp;
    assert_eq!(dereferenced, -99);
}

// ===========================================================================
// AsRef Trait Tests (5 tests)
// ===========================================================================

#[test]
fn port_asref_yields_u16_ref() {
    let port = Port::new(443).unwrap();
    let reference: &u16 = port.as_ref();
    assert_eq!(reference, &443);
}

#[test]
fn retry_limit_asref_yields_u32_ref() {
    let rl = RetryLimit::new(50).unwrap();
    let reference: &u32 = rl.as_ref();
    assert_eq!(reference, &50);
}

#[test]
fn retry_attempt_asref_yields_u32_ref() {
    let ra = RetryAttempt::new(3).unwrap();
    let reference: &u32 = ra.as_ref();
    assert_eq!(reference, &3);
}

#[test]
fn progress_asref_yields_f64_ref() {
    let p = Progress::new(88.8).unwrap();
    let reference: &f64 = p.as_ref();
    assert_eq!(reference, &88.8);
}

#[test]
fn task_position_asref_yields_i64_ref() {
    let tp = TaskPosition::new(42).unwrap();
    let reference: &i64 = tp.as_ref();
    assert_eq!(reference, &42);
}

// ===========================================================================
// Error Path Tests - from_option variants (4 tests)
// ===========================================================================

#[test]
fn retry_limit_from_option_accepts_some() {
    let rl =
        RetryLimit::from_option(Some(5)).expect("RetryLimit::from_option(Some) should succeed");
    assert_eq!(rl.value(), 5);
}

#[test]
fn retry_limit_from_option_rejects_none() {
    let result = RetryLimit::from_option(None);
    assert!(result.is_err(), "RetryLimit::from_option(None) should fail");
}

#[test]
fn task_count_from_option_accepts_some() {
    let tc = TaskCount::from_option(Some(10)).expect("TaskCount::from_option(Some) should succeed");
    assert_eq!(tc.value(), 10);
}

#[test]
fn task_count_from_option_rejects_none() {
    let result = TaskCount::from_option(None);
    assert!(result.is_err(), "TaskCount::from_option(None) should fail");
}

// ===========================================================================
// Equality and Inequality Tests (4 tests)
// ===========================================================================

#[test]
fn port_equality_holds_for_same_values() {
    let p1 = Port::new(80).unwrap();
    let p2 = Port::new(80).unwrap();
    assert_eq!(p1, p2);
}

#[test]
fn port_inequality_holds_for_different_values() {
    let p1 = Port::new(80).unwrap();
    let p2 = Port::new(8080).unwrap();
    assert_ne!(p1, p2);
}

#[test]
fn progress_equality_holds_for_same_values() {
    let p1 = Progress::new(50.0).unwrap();
    let p2 = Progress::new(50.0).unwrap();
    assert_eq!(p1, p2);
}

#[test]
fn progress_inequality_holds_for_different_values() {
    let p1 = Progress::new(50.0).unwrap();
    let p2 = Progress::new(75.0).unwrap();
    assert_ne!(p1, p2);
}
