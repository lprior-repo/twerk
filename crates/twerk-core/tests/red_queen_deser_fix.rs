//! Red Queen adversarial test for deserialization validation fix.
//!
//! Tests that the ACTUAL Port and Progress types reject invalid values.

use serde::{Deserialize, Serialize};
use twerk_core::types::{Port, Progress};

#[derive(Debug, Serialize, Deserialize)]
#[serde(transparent)]
struct PortWrapper(Port);

#[derive(Debug, Serialize, Deserialize)]
#[serde(transparent)]
struct ProgressWrapper(Progress);

#[test]
fn port_zero_should_fail_deserialization() {
    // Port 0 is invalid - should FAIL to deserialize
    let result: Result<PortWrapper, _> = serde_json::from_str("0");
    assert!(
        result.is_err(),
        "Port(0) should be rejected by deserialization"
    );
}

#[test]
fn progress_negative_should_fail_deserialization() {
    // Progress -0.001 is out of range - should FAIL to deserialize
    let result: Result<ProgressWrapper, _> = serde_json::from_str("-0.001");
    assert!(
        result.is_err(),
        "Progress(-0.001) should be rejected by deserialization"
    );
}

#[test]
fn progress_over_100_should_fail_deserialization() {
    // Progress 100.001 is out of range - should FAIL to deserialize
    let result: Result<ProgressWrapper, _> = serde_json::from_str("100.001");
    assert!(
        result.is_err(),
        "Progress(100.001) should be rejected by deserialization"
    );
}

#[test]
fn valid_port_deserializes_correctly() {
    // Valid port should work
    let result: Result<PortWrapper, _> = serde_json::from_str("8080");
    assert!(result.is_ok());
    assert_eq!(result.unwrap().0.value(), 8080);
}

#[test]
fn valid_progress_deserializes_correctly() {
    // Valid progress should work
    let result: Result<ProgressWrapper, _> = serde_json::from_str("50.0");
    assert!(result.is_ok());
    assert!((result.unwrap().0.value() - 50.0).abs() < f64::EPSILON);
}

#[test]
fn port_roundtrip_works() {
    // Round-trip should preserve value
    let port = PortWrapper(Port::new(8080).unwrap());
    let json = serde_json::to_string(&port).unwrap();
    let deserialized: PortWrapper = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.0.value(), 8080);
}

#[test]
fn progress_roundtrip_works() {
    // Round-trip should preserve value
    let progress = ProgressWrapper(Progress::new(75.5).unwrap());
    let json = serde_json::to_string(&progress).unwrap();
    let deserialized: ProgressWrapper = serde_json::from_str(&json).unwrap();
    assert!((deserialized.0.value() - 75.5).abs() < f64::EPSILON);
}
