use twerk_core::asl::types::BackoffRate;
use twerk_core::types::{Port, Progress};

// ---------------------------------------------------------------------------
// Port
// ---------------------------------------------------------------------------

#[kani::proof]
fn port_new_rejects_zero() {
    let result = Port::new(0);
    assert!(result.is_err(), "Port 0 should be rejected");
}

#[kani::proof]
fn port_new_accepts_valid() {
    assert!(Port::new(80).is_ok(), "Port 80 should be accepted");
    assert!(Port::new(443).is_ok(), "Port 443 should be accepted");
}

#[kani::proof]
fn port_serde_roundtrip() {
    let port = Port::new(8080).unwrap();
    let serialized = serde_json::to_string(&port).unwrap();
    let deserialized: Port = serde_json::from_str(&serialized).unwrap();
    assert_eq!(port.value(), deserialized.value());
}

// ---------------------------------------------------------------------------
// Progress
// ---------------------------------------------------------------------------

#[kani::proof]
fn progress_new_rejects_nan() {
    let result = Progress::new(f64::NAN);
    assert!(result.is_err(), "NaN progress should be rejected");
}

#[kani::proof]
fn progress_new_rejects_negative() {
    let result = Progress::new(-1.0);
    assert!(result.is_err(), "Negative progress should be rejected");
}

#[kani::proof]
fn progress_new_rejects_over_100() {
    let result = Progress::new(100.1);
    assert!(result.is_err(), "Progress > 100 should be rejected");
}

#[kani::proof]
fn progress_new_accepts_boundaries() {
    assert!(
        Progress::new(0.0).is_ok(),
        "Progress 0.0 should be accepted"
    );
    assert!(
        Progress::new(100.0).is_ok(),
        "Progress 100.0 should be accepted"
    );
}

#[kani::proof]
fn progress_serde_roundtrip() {
    let progress = Progress::new(42.5).unwrap();
    let serialized = serde_json::to_string(&progress).unwrap();
    let deserialized: Progress = serde_json::from_str(&serialized).unwrap();
    assert_eq!(progress.value(), deserialized.value());
}

// ---------------------------------------------------------------------------
// BackoffRate
// ---------------------------------------------------------------------------

#[kani::proof]
fn backoff_rate_rejects_zero() {
    let result = BackoffRate::new(0.0);
    assert!(result.is_err(), "BackoffRate 0.0 should be rejected");
}

#[kani::proof]
fn backoff_rate_rejects_negative() {
    let result = BackoffRate::new(-1.0);
    assert!(result.is_err(), "Negative BackoffRate should be rejected");
}

#[kani::proof]
fn backoff_rate_rejects_infinity() {
    let result = BackoffRate::new(f64::INFINITY);
    assert!(result.is_err(), "Infinite BackoffRate should be rejected");
}

#[kani::proof]
fn backoff_rate_accepts_valid() {
    assert!(
        BackoffRate::new(1.0).is_ok(),
        "BackoffRate 1.0 should be accepted"
    );
    assert!(
        BackoffRate::new(2.5).is_ok(),
        "BackoffRate 2.5 should be accepted"
    );
}
