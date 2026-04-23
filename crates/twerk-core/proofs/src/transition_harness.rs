use twerk_core::asl::transition::Transition;

#[kani::proof]
fn transition_deserialize_rejects_both_next_and_end() {
    let json = r#"{"next":"some_state","end":true}"#;
    let result: Result<Transition, _> = serde_json::from_str(json);
    assert!(result.is_err(), "JSON with both 'next' and 'end' should fail");
}

#[kani::proof]
fn transition_deserialize_rejects_neither() {
    let json = r#"{}"#;
    let result: Result<Transition, _> = serde_json::from_str(json);
    assert!(
        result.is_err(),
        "JSON with neither 'next' nor 'end' should fail"
    );
}

#[kani::proof]
fn transition_deserialize_accepts_next_only() {
    let json = r#"{"next":"some_state"}"#;
    let result: Result<Transition, _> = serde_json::from_str(json);
    assert!(
        result.is_ok(),
        "JSON with only 'next' should succeed"
    );
    let t = result.unwrap();
    assert!(t.is_next());
}
