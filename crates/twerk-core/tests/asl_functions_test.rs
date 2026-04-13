//! Tests for ASL intrinsic functions.

#![allow(clippy::unwrap_used)]

use std::collections::HashMap;
use twerk_core::eval::evaluate_template;

fn empty_context() -> HashMap<String, serde_json::Value> {
    HashMap::new()
}

fn ctx_with(key: &str, val: serde_json::Value) -> HashMap<String, serde_json::Value> {
    let mut m = HashMap::new();
    m.insert(key.to_string(), val);
    m
}

// ───────────────────── format ─────────────────────

#[test]
fn format_basic_positional() {
    let ctx = empty_context();
    let r = evaluate_template(r#"{{ format("Hello, {}!", "world") }}"#, &ctx).unwrap();
    assert_eq!(r, "Hello, world!");
}

#[test]
fn format_multiple_args() {
    let ctx = empty_context();
    let r = evaluate_template(r#"{{ format("{} + {} = {}", 1, 2, 3) }}"#, &ctx).unwrap();
    assert_eq!(r, "1 + 2 = 3");
}

#[test]
fn format_no_placeholders() {
    let ctx = empty_context();
    let r = evaluate_template(r#"{{ format("plain text") }}"#, &ctx).unwrap();
    assert_eq!(r, "plain text");
}

#[test]
fn format_too_few_args_leaves_placeholder() {
    let ctx = empty_context();
    let r = evaluate_template(r#"{{ format("{} plus {}", "one") }}"#, &ctx).unwrap();
    assert_eq!(r, "one plus {}");
}

// ───────────────────── stringToJson ─────────────────────

#[test]
fn string_to_json_object() {
    let ctx = empty_context();
    // stringToJson returns an evalexpr value, which becomes JSON in template output
    let r = evaluate_template(r#"{{ stringToJson("{\"a\":1}") }}"#, &ctx).unwrap();
    // evalexpr tuple of tuple pairs → JSON array
    assert!(r.contains("a"));
}

#[test]
fn string_to_json_invalid() {
    let ctx = empty_context();
    let r = evaluate_template(r#"{{ stringToJson("not json!") }}"#, &ctx);
    let err = r.unwrap_err();
    assert!(err.to_string().contains("stringToJson"), "{err}");
}

// ───────────────────── jsonToString ─────────────────────

#[test]
fn json_to_string_int() {
    let ctx = empty_context();
    let r = evaluate_template(r#"{{ jsonToString(42) }}"#, &ctx).unwrap();
    assert_eq!(r, "42");
}

#[test]
fn json_to_string_string() {
    let ctx = empty_context();
    let r = evaluate_template(r#"{{ jsonToString("hello") }}"#, &ctx).unwrap();
    assert_eq!(r, r#""hello""#);
}

// ───────────────────── array ─────────────────────

#[test]
fn array_creates_tuple() {
    let ctx = empty_context();
    let r = evaluate_template(r#"{{ array(1, 2, 3) }}"#, &ctx).unwrap();
    assert_eq!(r, "[1,2,3]");
}

#[test]
fn array_empty() {
    let ctx = empty_context();
    let r = evaluate_template(r#"{{ array() }}"#, &ctx).unwrap();
    assert_eq!(r, "[]");
}

// ───────────────────── mathRandom ─────────────────────

#[test]
fn math_random_in_range() {
    let ctx = empty_context();
    for _ in 0..20 {
        let r = evaluate_template(r#"{{ mathRandom(1, 10) }}"#, &ctx).unwrap();
        let n: i64 = r.parse().unwrap();
        assert!((1..10).contains(&n), "expected 1..10 got {n}");
    }
}

#[test]
fn math_random_single_value_range() {
    let ctx = empty_context();
    let r = evaluate_template(r#"{{ mathRandom(5, 6) }}"#, &ctx).unwrap();
    assert_eq!(r, "5");
}

#[test]
fn math_random_invalid_range() {
    let ctx = empty_context();
    let r = evaluate_template(r#"{{ mathRandom(10, 5) }}"#, &ctx);
    let err = r.unwrap_err();
    assert!(err.to_string().contains("mathRandom"), "{err}");
}

// ───────────────────── mathAdd / mathSub ─────────────────────

#[test]
fn math_add_ints() {
    let ctx = empty_context();
    let r = evaluate_template(r#"{{ mathAdd(3, 4) }}"#, &ctx).unwrap();
    assert_eq!(r, "7");
}

#[test]
fn math_add_floats() {
    let ctx = empty_context();
    let r = evaluate_template(r#"{{ mathAdd(1.5, 2.5) }}"#, &ctx).unwrap();
    assert_eq!(r, "4.0");
}

#[test]
fn math_sub_ints() {
    let ctx = empty_context();
    let r = evaluate_template(r#"{{ mathSub(10, 3) }}"#, &ctx).unwrap();
    assert_eq!(r, "7");
}

#[test]
fn math_sub_negative_result() {
    let ctx = empty_context();
    let r = evaluate_template(r#"{{ mathSub(3, 10) }}"#, &ctx).unwrap();
    assert_eq!(r, "-7");
}

// ───────────────────── uuid ─────────────────────

#[test]
fn uuid_returns_valid_v4() {
    let ctx = empty_context();
    let r = evaluate_template(r#"{{ uuid() }}"#, &ctx).unwrap();
    assert_eq!(r.len(), 36, "UUID should be 36 chars: {r}");
    assert!(r.contains('-'));
    // Parse to validate
    uuid::Uuid::parse_str(&r).expect("should be valid UUID");
}

#[test]
fn uuid_unique() {
    let ctx = empty_context();
    let a = evaluate_template(r#"{{ uuid() }}"#, &ctx).unwrap();
    let b = evaluate_template(r#"{{ uuid() }}"#, &ctx).unwrap();
    assert_ne!(a, b);
}

// ───────────────────── hash ─────────────────────

#[test]
fn hash_sha256() {
    let ctx = empty_context();
    let r = evaluate_template(r#"{{ hash("hello", "sha256") }}"#, &ctx).unwrap();
    assert_eq!(
        r,
        "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
    );
}

#[test]
fn hash_md5() {
    let ctx = empty_context();
    let r = evaluate_template(r#"{{ hash("hello", "md5") }}"#, &ctx).unwrap();
    assert_eq!(r, "5d41402abc4b2a76b9719d911017c592");
}

#[test]
fn hash_unsupported_algo() {
    let ctx = empty_context();
    let r = evaluate_template(r#"{{ hash("hello", "sha512") }}"#, &ctx);
    let err = r.unwrap_err();
    assert!(err.to_string().contains("hash"), "{err}");
}

// ───────────────────── base64 ─────────────────────

#[test]
fn base64_encode_decode_roundtrip() {
    let ctx = empty_context();
    let encoded = evaluate_template(r#"{{ base64Encode("Hello World") }}"#, &ctx).unwrap();
    assert_eq!(encoded, "SGVsbG8gV29ybGQ=");
    let decoded =
        evaluate_template(&format!(r#"{{{{ base64Decode("{}") }}}}"#, encoded), &ctx).unwrap();
    assert_eq!(decoded, "Hello World");
}

#[test]
fn base64_decode_invalid() {
    let ctx = empty_context();
    let r = evaluate_template(r#"{{ base64Decode("!!!not-base64") }}"#, &ctx);
    let err = r.unwrap_err();
    assert!(err.to_string().contains("base64Decode"), "{err}");
}

// ───────────────────── arrayPartition ─────────────────────

#[test]
fn array_partition_even() {
    let ctx = ctx_with("arr", serde_json::json!([1, 2, 3, 4]));
    let r = evaluate_template(r#"{{ arrayPartition(arr, 2) }}"#, &ctx).unwrap();
    assert_eq!(r, "[[1,2],[3,4]]");
}

#[test]
fn array_partition_uneven() {
    let ctx = ctx_with("arr", serde_json::json!([1, 2, 3, 4, 5]));
    let r = evaluate_template(r#"{{ arrayPartition(arr, 2) }}"#, &ctx).unwrap();
    assert_eq!(r, "[[1,2],[3,4],[5]]");
}

#[test]
fn array_partition_zero_chunk() {
    let ctx = ctx_with("arr", serde_json::json!([1, 2]));
    let r = evaluate_template(r#"{{ arrayPartition(arr, 0) }}"#, &ctx);
    let err = r.unwrap_err();
    assert!(err.to_string().contains("arrayPartition"), "{err}");
}

// ───────────────────── arrayContains ─────────────────────

#[test]
fn array_contains_found() {
    let ctx = ctx_with("arr", serde_json::json!([1, 2, 3]));
    let r = evaluate_template(r#"{{ arrayContains(arr, 2) }}"#, &ctx).unwrap();
    assert_eq!(r, "true");
}

#[test]
fn array_contains_not_found() {
    let ctx = ctx_with("arr", serde_json::json!([1, 2, 3]));
    let r = evaluate_template(r#"{{ arrayContains(arr, 9) }}"#, &ctx).unwrap();
    assert_eq!(r, "false");
}

#[test]
fn array_contains_string() {
    let ctx = ctx_with("arr", serde_json::json!(["a", "b", "c"]));
    let r = evaluate_template(r#"{{ arrayContains(arr, "b") }}"#, &ctx).unwrap();
    assert_eq!(r, "true");
}

// ───────────────────── arrayRange ─────────────────────

#[test]
fn array_range_basic() {
    let ctx = empty_context();
    let r = evaluate_template(r#"{{ arrayRange(0, 5, 1) }}"#, &ctx).unwrap();
    assert_eq!(r, "[0,1,2,3,4]");
}

#[test]
fn array_range_step_2() {
    let ctx = empty_context();
    let r = evaluate_template(r#"{{ arrayRange(0, 10, 3) }}"#, &ctx).unwrap();
    assert_eq!(r, "[0,3,6,9]");
}

#[test]
fn array_range_zero_step() {
    let ctx = empty_context();
    let r = evaluate_template(r#"{{ arrayRange(0, 5, 0) }}"#, &ctx);
    let err = r.unwrap_err();
    assert!(err.to_string().contains("arrayRange"), "{err}");
}

// ───────────────────── arrayLength ─────────────────────

#[test]
fn array_length_basic() {
    let ctx = ctx_with("arr", serde_json::json!([10, 20, 30]));
    let r = evaluate_template(r#"{{ arrayLength(arr) }}"#, &ctx).unwrap();
    assert_eq!(r, "3");
}

#[test]
fn array_length_empty() {
    let ctx = ctx_with("arr", serde_json::json!([]));
    let r = evaluate_template(r#"{{ arrayLength(arr) }}"#, &ctx).unwrap();
    assert_eq!(r, "0");
}

// ───────────────────── arrayUnique ─────────────────────

#[test]
fn array_unique_dedup() {
    let ctx = ctx_with("arr", serde_json::json!([1, 2, 2, 3, 3, 3]));
    let r = evaluate_template(r#"{{ arrayUnique(arr) }}"#, &ctx).unwrap();
    assert_eq!(r, "[1,2,3]");
}

#[test]
fn array_unique_strings() {
    let ctx = ctx_with("arr", serde_json::json!(["a", "b", "a", "c"]));
    let r = evaluate_template(r#"{{ arrayUnique(arr) }}"#, &ctx).unwrap();
    assert_eq!(r, r#"["a","b","c"]"#);
}

#[test]
fn array_unique_already_unique() {
    let ctx = ctx_with("arr", serde_json::json!([1, 2, 3]));
    let r = evaluate_template(r#"{{ arrayUnique(arr) }}"#, &ctx).unwrap();
    assert_eq!(r, "[1,2,3]");
}
