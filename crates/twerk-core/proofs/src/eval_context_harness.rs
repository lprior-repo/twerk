use twerk_core::eval::context::{eval_value_to_json, json_to_eval_value};

// ---------------------------------------------------------------------------
// Null -> Empty -> Null roundtrip
// ---------------------------------------------------------------------------

#[kani::proof]
fn json_null_to_eval_empty_roundtrip() {
    let json_null = serde_json::Value::Null;
    let eval_val = json_to_eval_value(&json_null).unwrap();
    assert!(matches!(eval_val, evalexpr::Value::Empty));

    let back = eval_value_to_json(&eval_val);
    assert_eq!(back, serde_json::Value::Null);
}

// ---------------------------------------------------------------------------
// Bool -> Boolean -> Bool roundtrip
// ---------------------------------------------------------------------------

#[kani::proof]
fn json_bool_true_to_eval_boolean_roundtrip() {
    let json_true = serde_json::Value::Bool(true);
    let eval_val = json_to_eval_value(&json_true).unwrap();
    assert!(matches!(eval_val, evalexpr::Value::Boolean(true)));

    let back = eval_value_to_json(&eval_val);
    assert_eq!(back, json_true);
}

#[kani::proof]
fn json_bool_false_to_eval_boolean_roundtrip() {
    let json_false = serde_json::Value::Bool(false);
    let eval_val = json_to_eval_value(&json_false).unwrap();
    assert!(matches!(eval_val, evalexpr::Value::Boolean(false)));

    let back = eval_value_to_json(&eval_val);
    assert_eq!(back, json_false);
}

// ---------------------------------------------------------------------------
// Integer number -> Int -> Number roundtrip
// ---------------------------------------------------------------------------

#[kani::proof]
fn json_number_to_eval_int_roundtrip() {
    let json_num = serde_json::json!(42);
    let eval_val = json_to_eval_value(&json_num).unwrap();
    assert!(matches!(eval_val, evalexpr::Value::Int(42)));

    let back = eval_value_to_json(&eval_val);
    assert_eq!(back, json_num);
}

// ---------------------------------------------------------------------------
// String -> String -> String roundtrip
// ---------------------------------------------------------------------------

#[kani::proof]
fn json_string_to_eval_string_roundtrip() {
    let json_str = serde_json::json!("hello world");
    let eval_val = json_to_eval_value(&json_str).unwrap();
    assert!(matches!(eval_val, evalexpr::Value::String(ref s) if s == "hello world"));

    let back = eval_value_to_json(&eval_val);
    assert_eq!(back, json_str);
}

// ---------------------------------------------------------------------------
// Array -> Tuple -> Array roundtrip
// ---------------------------------------------------------------------------

#[kani::proof]
fn json_array_to_eval_tuple_roundtrip() {
    let json_arr = serde_json::json!([1, "two", true]);
    let eval_val = json_to_eval_value(&json_arr).unwrap();
    // Should be a Tuple with 3 elements
    if let evalexpr::Value::Tuple(ref items) = eval_val {
        assert_eq!(items.len(), 3);
        assert!(matches!(items[0], evalexpr::Value::Int(1)));
        assert!(matches!(items[1], evalexpr::Value::String(ref s) if s == "two"));
        assert!(matches!(items[2], evalexpr::Value::Boolean(true)));
    } else {
        panic!("Expected Tuple variant");
    }

    let back = eval_value_to_json(&eval_val);
    assert_eq!(back, json_arr);
}

// ---------------------------------------------------------------------------
// Float number -> Float -> Number roundtrip
// ---------------------------------------------------------------------------

#[kani::proof]
fn json_number_to_eval_float_roundtrip() {
    // Use a number that cannot be represented as i64 to get Float variant
    let json_flt = serde_json::json!(1.5);
    let eval_val = json_to_eval_value(&json_flt).unwrap();
    assert!(matches!(eval_val, evalexpr::Value::Float(_)));

    let back = eval_value_to_json(&eval_val);
    assert_eq!(back, json_flt);
}

// ---------------------------------------------------------------------------
// Empty array -> empty Tuple roundtrip
// ---------------------------------------------------------------------------

#[kani::proof]
fn json_empty_array_to_eval_tuple_roundtrip() {
    let json_arr = serde_json::json!([]);
    let eval_val = json_to_eval_value(&json_arr).unwrap();
    if let evalexpr::Value::Tuple(ref items) = eval_val {
        assert!(items.is_empty());
    } else {
        panic!("Expected Tuple variant");
    }

    let back = eval_value_to_json(&eval_val);
    assert_eq!(back, json_arr);
}
