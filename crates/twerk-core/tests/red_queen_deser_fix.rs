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
    let error = serde_json::from_str::<PortWrapper>("0")
        .expect_err("Port(0) should be rejected by deserialization");
    assert!(
        error.to_string().contains('0'),
        "expected rejected port error to mention 0, got: {error}"
    );
}

#[test]
fn progress_negative_should_fail_deserialization() {
    // Progress -0.001 is out of range - should FAIL to deserialize
    let error = serde_json::from_str::<ProgressWrapper>("-0.001")
        .expect_err("Progress(-0.001) should be rejected by deserialization");
    assert!(
        error.to_string().contains("-0.001"),
        "expected rejected progress error to mention -0.001, got: {error}"
    );
}

#[test]
fn progress_over_100_should_fail_deserialization() {
    // Progress 100.001 is out of range - should FAIL to deserialize
    let error = serde_json::from_str::<ProgressWrapper>("100.001")
        .expect_err("Progress(100.001) should be rejected by deserialization");
    assert!(
        error.to_string().contains("100.001"),
        "expected rejected progress error to mention 100.001, got: {error}"
    );
}

#[test]
fn valid_port_deserializes_correctly() {
    // Valid port should work
    let port = serde_json::from_str::<PortWrapper>("8080")
        .unwrap_or_else(|error| panic!("valid port should deserialize: {error}"));
    assert_eq!(port.0.value(), 8080);
}

#[test]
fn valid_progress_deserializes_correctly() {
    // Valid progress should work
    let progress = serde_json::from_str::<ProgressWrapper>("50.0")
        .unwrap_or_else(|error| panic!("valid progress should deserialize: {error}"));
    assert!((progress.0.value() - 50.0).abs() < f64::EPSILON);
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
