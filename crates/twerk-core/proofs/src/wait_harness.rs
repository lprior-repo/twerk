use twerk_core::asl::wait::WaitDuration;

#[kani::proof]
fn wait_duration_rejects_multiple_fields() {
    let json = r#"{"seconds":5,"timestamp":"2025-01-01T00:00:00Z"}"#;
    let result: Result<WaitDuration, _> = serde_json::from_str(json);
    assert!(
        result.is_err(),
        "JSON with both seconds and timestamp should fail"
    );
}

#[kani::proof]
fn wait_duration_accepts_single_field() {
    let json = r#"{"seconds":5}"#;
    let result: Result<WaitDuration, _> = serde_json::from_str(json);
    assert!(
        result.is_ok(),
        "JSON with only 'seconds' should succeed"
    );
    let wd = result.unwrap();
    assert!(wd.is_seconds());
}
