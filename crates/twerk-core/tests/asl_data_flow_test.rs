//! Tests for ASL data flow processing (input_path, result_path, output_path).

use serde_json::json;
use twerk_core::asl::types::JsonPath;
use twerk_core::eval::data_flow::{
    apply_data_flow, apply_input_path, apply_output_path, apply_result_path, DataFlowError,
};

// ---------------------------------------------------------------------------
// apply_input_path
// ---------------------------------------------------------------------------

#[test]
fn input_path_none_passes_through() {
    let input = json!({"x": 1, "y": 2});
    let result = apply_input_path(&input, None).unwrap();
    assert_eq!(result, json!({"x": 1, "y": 2}));
}

#[test]
fn input_path_dollar_returns_entire_input() {
    let input = json!({"a": 10});
    let path = JsonPath::new("$").unwrap();
    let result = apply_input_path(&input, Some(&path)).unwrap();
    assert_eq!(result, json!({"a": 10}));
}

#[test]
fn input_path_extracts_top_level_field() {
    let input = json!({"name": "alice", "age": 30});
    let path = JsonPath::new("$.name").unwrap();
    let result = apply_input_path(&input, Some(&path)).unwrap();
    assert_eq!(result, json!("alice"));
}

#[test]
fn input_path_extracts_nested_field() {
    let input = json!({"config": {"db": {"host": "localhost"}}});
    let path = JsonPath::new("$.config.db.host").unwrap();
    let result = apply_input_path(&input, Some(&path)).unwrap();
    assert_eq!(result, json!("localhost"));
}

#[test]
fn input_path_extracts_nested_object() {
    let input = json!({"config": {"db": {"host": "localhost", "port": 5432}}});
    let path = JsonPath::new("$.config.db").unwrap();
    let result = apply_input_path(&input, Some(&path)).unwrap();
    assert_eq!(result, json!({"host": "localhost", "port": 5432}));
}

#[test]
fn input_path_array_index() {
    let input = json!({"items": [10, 20, 30]});
    let path = JsonPath::new("$.items[1]").unwrap();
    let result = apply_input_path(&input, Some(&path)).unwrap();
    assert_eq!(result, json!(20));
}

#[test]
fn input_path_not_found_returns_error() {
    let input = json!({"a": 1});
    let path = JsonPath::new("$.missing").unwrap();
    let result = apply_input_path(&input, Some(&path));
    match result.unwrap_err() {
        DataFlowError::PathNotFound { path, .. } => assert_eq!(path, "$.missing"),
        other => panic!("expected PathNotFound, got {other:?}"),
    }
}

#[test]
fn input_path_field_on_non_object_returns_error() {
    let input = json!(42);
    let path = JsonPath::new("$.field").unwrap();
    let result = apply_input_path(&input, Some(&path));
    match result.unwrap_err() {
        DataFlowError::NotAnObject { .. } => {}
        other => panic!("expected NotAnObject, got {other:?}"),
    }
}

#[test]
fn input_path_array_index_out_of_bounds() {
    let input = json!({"items": [1, 2]});
    let path = JsonPath::new("$.items[5]").unwrap();
    let result = apply_input_path(&input, Some(&path));
    match result.unwrap_err() {
        DataFlowError::PathNotFound { .. } => {}
        other => panic!("expected PathNotFound, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// apply_result_path
// ---------------------------------------------------------------------------

#[test]
fn result_path_none_replaces_input() {
    let input = json!({"old": true});
    let result = json!({"new": true});
    let out = apply_result_path(&input, &result, None).unwrap();
    assert_eq!(out, json!({"new": true}));
}

#[test]
fn result_path_dollar_replaces_input() {
    let input = json!({"old": true});
    let result = json!({"replaced": true});
    let path = JsonPath::new("$").unwrap();
    let out = apply_result_path(&input, &result, Some(&path)).unwrap();
    assert_eq!(out, json!({"replaced": true}));
}

#[test]
fn result_path_merges_into_field() {
    let input = json!({"x": 1});
    let result = json!(42);
    let path = JsonPath::new("$.output").unwrap();
    let out = apply_result_path(&input, &result, Some(&path)).unwrap();
    assert_eq!(out, json!({"x": 1, "output": 42}));
}

#[test]
fn result_path_merges_into_nested_field() {
    let input = json!({"a": {"b": 1}});
    let result = json!("hello");
    let path = JsonPath::new("$.a.c").unwrap();
    let out = apply_result_path(&input, &result, Some(&path)).unwrap();
    assert_eq!(out, json!({"a": {"b": 1, "c": "hello"}}));
}

#[test]
fn result_path_creates_intermediate_objects() {
    let input = json!({"x": 1});
    let result = json!(99);
    let path = JsonPath::new("$.new.nested").unwrap();
    let out = apply_result_path(&input, &result, Some(&path)).unwrap();
    assert_eq!(out, json!({"x": 1, "new": {"nested": 99}}));
}

#[test]
fn result_path_on_non_object_returns_error() {
    let input = json!(42);
    let result = json!("val");
    let path = JsonPath::new("$.field").unwrap();
    let out = apply_result_path(&input, &result, Some(&path));
    match out.unwrap_err() {
        DataFlowError::NotAnObject { .. } => {}
        other => panic!("expected NotAnObject, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// apply_output_path
// ---------------------------------------------------------------------------

#[test]
fn output_path_none_passes_through() {
    let output = json!({"a": 1, "b": 2});
    let result = apply_output_path(&output, None).unwrap();
    assert_eq!(result, json!({"a": 1, "b": 2}));
}

#[test]
fn output_path_extracts_field() {
    let output = json!({"result": 42, "debug": "info"});
    let path = JsonPath::new("$.result").unwrap();
    let result = apply_output_path(&output, Some(&path)).unwrap();
    assert_eq!(result, json!(42));
}

// ---------------------------------------------------------------------------
// apply_data_flow (full pipeline)
// ---------------------------------------------------------------------------

#[test]
fn full_pipeline_all_none() {
    let input = json!({"data": 1});
    let result = json!({"data": 1});
    let out = apply_data_flow(&input, &result, None, None, None).unwrap();
    // result replaces input (result_path=None), output_path=None passes through
    assert_eq!(out, json!({"data": 1}));
}

#[test]
fn full_pipeline_input_and_result_path() {
    let input = json!({"payload": {"val": 5}, "meta": "x"});
    let ip = JsonPath::new("$.payload").unwrap();
    let rp = JsonPath::new("$.computed").unwrap();

    // input_path extracts {"val": 5}
    // result is merged at $.computed → {"val": 5, "computed": <result>}
    // But result_path merges result into the *original* input after input_path filtering
    // Actually per ASL spec: input_path filters input, state processes filtered,
    // result_path merges result into *original* input, output_path filters that.
    // But in our simplified API, we take result as given.
    // apply_data_flow: filtered = apply_input_path(input, ip) → {"val": 5}
    //                  merged = apply_result_path(filtered, result, rp)
    //                         → {"val": 5, "computed": result}
    // Since result = the state output which we pass in.
    let state_result = json!(100);
    let out = apply_data_flow(&input, &state_result, Some(&ip), Some(&rp), None).unwrap();
    assert_eq!(out, json!({"val": 5, "computed": 100}));
}

#[test]
fn full_pipeline_all_paths() {
    let input = json!({"request": {"x": 10}, "extra": "ignored"});
    let ip = JsonPath::new("$.request").unwrap();
    let rp = JsonPath::new("$.answer").unwrap();
    let op = JsonPath::new("$.answer").unwrap();

    let state_result = json!(42);
    let out = apply_data_flow(&input, &state_result, Some(&ip), Some(&rp), Some(&op)).unwrap();
    // input_path: {"x": 10}
    // result_path: {"x": 10, "answer": 42}
    // output_path: 42
    assert_eq!(out, json!(42));
}

// ---------------------------------------------------------------------------
// Edge cases
// ---------------------------------------------------------------------------

#[test]
fn input_path_on_array_root() {
    let input = json!([1, 2, 3]);
    let path = JsonPath::new("$").unwrap();
    let result = apply_input_path(&input, Some(&path)).unwrap();
    assert_eq!(result, json!([1, 2, 3]));
}

#[test]
fn result_path_overwrites_existing_field() {
    let input = json!({"x": 1, "y": 2});
    let result = json!(99);
    let path = JsonPath::new("$.x").unwrap();
    let out = apply_result_path(&input, &result, Some(&path)).unwrap();
    assert_eq!(out, json!({"x": 99, "y": 2}));
}

#[test]
fn nested_array_index_access() {
    let input = json!({"matrix": [[1, 2], [3, 4]]});
    let path = JsonPath::new("$.matrix[1]").unwrap();
    let result = apply_input_path(&input, Some(&path)).unwrap();
    assert_eq!(result, json!([3, 4]));
}

#[test]
fn display_impl_for_data_flow_error() {
    let err = DataFlowError::PathNotFound {
        path: "$.x".to_string(),
        available: vec!["a".to_string(), "b".to_string()],
    };
    let msg = err.to_string();
    assert!(msg.contains("$.x"));
}

#[test]
fn input_path_with_unclosed_bracket_returns_invalid_path_error() {
    let input = json!({"items": [1, 2, 3]});
    let path = JsonPath::new("$.items[0").unwrap();
    let result = apply_input_path(&input, Some(&path));
    assert!(
        matches!(
            result,
            Err(DataFlowError::InvalidPath {
                ref path,
                ref reason
            }) if path == "$.items[0" && reason.contains("unclosed bracket")
        ),
        "expected InvalidPath, got {result:?}"
    );
}
