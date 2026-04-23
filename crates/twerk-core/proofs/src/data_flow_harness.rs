use twerk_core::asl::types::JsonPath;
use twerk_core::eval::data_flow::{apply_input_path, apply_output_path};

// ---------------------------------------------------------------------------
// apply_input_path: None returns clone of input
// ---------------------------------------------------------------------------

#[kani::proof]
fn apply_input_path_none_returns_clone() {
    let input = serde_json::json!({"key": "value", "num": 42});
    let result = apply_input_path(&input, None).unwrap();
    assert_eq!(result, input);
}

#[kani::proof]
fn apply_input_path_dollar_returns_clone() {
    let input = serde_json::json!({"key": "value", "num": 42});
    let path = JsonPath::new("$").unwrap();
    let result = apply_input_path(&input, Some(&path)).unwrap();
    assert_eq!(result, input);
}

// ---------------------------------------------------------------------------
// apply_input_path: $.field extracts a field
// ---------------------------------------------------------------------------

#[kani::proof]
fn apply_input_path_extracts_field() {
    let input = serde_json::json!({"key": "value", "num": 42});
    let path = JsonPath::new("$.key").unwrap();
    let result = apply_input_path(&input, Some(&path)).unwrap();
    assert_eq!(result, serde_json::json!("value"));
}

#[kani::proof]
fn apply_input_path_extracts_nested_field() {
    let input = serde_json::json!({"a": {"b": 99}});
    let path = JsonPath::new("$.a.b").unwrap();
    let result = apply_input_path(&input, Some(&path)).unwrap();
    assert_eq!(result, serde_json::json!(99));
}

#[kani::proof]
fn apply_input_path_extracts_array_index() {
    let input = serde_json::json!({"items": [10, 20, 30]});
    let path = JsonPath::new("$.items[1]").unwrap();
    let result = apply_input_path(&input, Some(&path)).unwrap();
    assert_eq!(result, serde_json::json!(20));
}

// ---------------------------------------------------------------------------
// apply_output_path: None returns clone
// ---------------------------------------------------------------------------

#[kani::proof]
fn apply_output_path_none_returns_clone() {
    let output = serde_json::json!({"result": true});
    let result = apply_output_path(&output, None).unwrap();
    assert_eq!(result, output);
}

#[kani::proof]
fn apply_output_path_dollar_returns_clone() {
    let output = serde_json::json!({"result": true});
    let path = JsonPath::new("$").unwrap();
    let result = apply_output_path(&output, Some(&path)).unwrap();
    assert_eq!(result, output);
}

// ---------------------------------------------------------------------------
// apply_output_path: $.field extracts a field
// ---------------------------------------------------------------------------

#[kani::proof]
fn apply_output_path_extracts_field() {
    let output = serde_json::json!({"result": "ok", "extra": "data"});
    let path = JsonPath::new("$.result").unwrap();
    let result = apply_output_path(&output, Some(&path)).unwrap();
    assert_eq!(result, serde_json::json!("ok"));
}

// ---------------------------------------------------------------------------
// apply_input_path: missing field returns error
// ---------------------------------------------------------------------------

#[kani::proof]
fn apply_input_path_missing_field_is_error() {
    let input = serde_json::json!({"key": "value"});
    let path = JsonPath::new("$.nonexistent").unwrap();
    let result = apply_input_path(&input, Some(&path));
    assert!(result.is_err(), "Missing field should return error");
}

// ---------------------------------------------------------------------------
// apply_input_path: non-object path returns error
// ---------------------------------------------------------------------------

#[kani::proof]
fn apply_input_path_non_object_is_error() {
    let input = serde_json::json!("just a string");
    let path = JsonPath::new("$.field").unwrap();
    let result = apply_input_path(&input, Some(&path));
    assert!(
        result.is_err(),
        "Navigating into a non-object should return error"
    );
}
