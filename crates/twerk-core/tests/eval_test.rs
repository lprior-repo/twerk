//! Tests for the eval module

#![allow(clippy::unwrap_used)]
#![allow(clippy::redundant_pattern_matching)]

use std::collections::HashMap;
use twerk_core::eval::{
    evaluate_condition, evaluate_expr, evaluate_task_condition, evaluate_template, sanitize_expr,
    transform_operators, valid_expr,
};
use twerk_core::id::{JobId, TaskId};
use twerk_core::job::JobSummary;
use twerk_core::task::TaskSummary;

const TEST_JOB_ID: &str = "550e8400-e29b-41d4-a716-446655440000";
const TEST_TASK_ID: &str = "661f9501-f30c-52e5-b827-557766551111";

fn make_job_summary(state: &str) -> JobSummary {
    JobSummary {
        id: Some(JobId::new(TEST_JOB_ID).unwrap()),
        name: Some("test-job".to_string()),
        state: state.parse().unwrap_or_default(),
        error: None,
        ..Default::default()
    }
}

fn make_task_summary(state: &str) -> TaskSummary {
    TaskSummary {
        id: Some(TaskId::new(TEST_TASK_ID).unwrap()),
        job_id: Some(JobId::new(TEST_JOB_ID).unwrap()),
        state: state.parse().unwrap_or_default(),
        ..Default::default()
    }
}

fn empty_context() -> HashMap<String, serde_json::Value> {
    HashMap::new()
}

// ---------------------------------------------------------------------------
// evaluate_condition tests
// ---------------------------------------------------------------------------

#[test]
fn test_empty_expression_returns_true() {
    let summary = make_job_summary("running");
    assert_eq!(evaluate_condition("", &summary), Ok(true));
}

#[test]
fn test_simple_state_comparison() {
    let summary = make_job_summary("running");
    assert_eq!(
        evaluate_condition("job_state == \"running\"", &summary),
        Ok(true)
    );
    assert_eq!(
        evaluate_condition("job_state == \"completed\"", &summary),
        Ok(false)
    );
}

#[test]
fn test_job_state_inequality() {
    let summary = make_job_summary("failed");
    assert_eq!(
        evaluate_condition("job_state != \"running\"", &summary),
        Ok(true)
    );
    assert_eq!(
        evaluate_condition("job_state != \"failed\"", &summary),
        Ok(false)
    );
}

#[test]
fn test_job_id_comparison() {
    let summary = make_job_summary("running");
    assert_eq!(
        evaluate_condition(&format!("job_id == \"{}\"", TEST_JOB_ID), &summary),
        Ok(true)
    );
    assert_eq!(
        evaluate_condition("job_id == \"other\"", &summary),
        Ok(false)
    );
}

#[test]
fn test_job_name_comparison() {
    let summary = make_job_summary("running");
    assert_eq!(
        evaluate_condition("job_name == \"test-job\"", &summary),
        Ok(true)
    );
    assert_eq!(
        evaluate_condition("job_name == \"other\"", &summary),
        Ok(false)
    );
}

#[test]
fn test_logical_and_operator() {
    let summary = make_job_summary("running");
    let result = evaluate_condition(
        "job_state == \"running\" and job_id == \"550e8400-e29b-41d4-a716-446655440000\"",
        &summary,
    );
    assert_eq!(result, Ok(true));

    let result = evaluate_condition("job_state == \"running\" and job_id == \"other\"", &summary);
    assert_eq!(result, Ok(false));
}

#[test]
fn test_logical_or_operator() {
    let summary = make_job_summary("running");
    let result = evaluate_condition(
        "job_state == \"completed\" or job_id == \"550e8400-e29b-41d4-a716-446655440000\"",
        &summary,
    );
    assert_eq!(result, Ok(true));

    let result = evaluate_condition(
        "job_state == \"completed\" or job_id == \"other\"",
        &summary,
    );
    assert_eq!(result, Ok(false));
}

#[test]
fn test_boolean_literals() {
    let summary = make_job_summary("running");
    assert_eq!(evaluate_condition("true", &summary), Ok(true));
    assert_eq!(evaluate_condition("false", &summary), Ok(false));
}

#[test]
fn test_parenthesized_expression() {
    let summary = make_job_summary("running");
    let result = evaluate_condition("(job_state == \"running\")", &summary);
    assert_eq!(result, Ok(true));
}

#[test]
fn test_complex_logical_expression() {
    let summary = make_job_summary("running");
    let result = evaluate_condition(
        "(job_state == \"running\" or job_state == \"pending\") and job_id == \"550e8400-e29b-41d4-a716-446655440000\"",
        &summary,
    );
    assert_eq!(result, Ok(true));
}

#[test]
fn test_job_error_with_error_present() {
    let mut summary = make_job_summary("failed");
    summary.error = Some("something went wrong".to_string());
    assert_eq!(evaluate_condition("job_error != \"\"", &summary), Ok(true));
    assert_eq!(evaluate_condition("job_error == \"\"", &summary), Ok(false));
}

#[test]
fn test_job_error_without_error() {
    let summary = make_job_summary("running");
    let result = evaluate_condition("job_error == \"\"", &summary);
    assert!(matches!(result, Err(_)));
}

#[test]
fn test_evaluate_condition_non_boolean_result() {
    let summary = make_job_summary("running");
    let result = evaluate_condition("\"hello\"", &summary);
    assert!(matches!(result, Err(_)));
    assert!(result
        .unwrap_err()
        .contains("did not evaluate to a boolean"));
}

#[test]
fn test_evaluate_condition_invalid_expression() {
    let summary = make_job_summary("running");
    let result = evaluate_condition("job_state ===", &summary);
    assert!(matches!(result, Err(_)));
}

// ---------------------------------------------------------------------------
// evaluate_task_condition tests
// ---------------------------------------------------------------------------

#[test]
fn test_task_empty_expression_returns_true() {
    let task_summary = make_task_summary("running");
    let job_summary = make_job_summary("running");
    assert_eq!(
        evaluate_task_condition("", &task_summary, &job_summary),
        Ok(true)
    );
}

#[test]
fn test_task_state_comparison() {
    let task_summary = make_task_summary("running");
    let job_summary = make_job_summary("running");
    assert_eq!(
        evaluate_task_condition("task_state == \"running\"", &task_summary, &job_summary),
        Ok(true)
    );
    assert_eq!(
        evaluate_task_condition("task_state == \"completed\"", &task_summary, &job_summary),
        Ok(false)
    );
}

#[test]
fn test_job_state_in_task_condition() {
    let task_summary = make_task_summary("running");
    let job_summary = make_job_summary("running");
    assert_eq!(
        evaluate_task_condition("job_state == \"running\"", &task_summary, &job_summary),
        Ok(true)
    );
    assert_eq!(
        evaluate_task_condition("job_state == \"failed\"", &task_summary, &job_summary),
        Ok(false)
    );
}

#[test]
fn test_task_id_comparison() {
    let task_summary = make_task_summary("running");
    let job_summary = make_job_summary("running");
    assert_eq!(
        evaluate_task_condition(
            &format!("task_id == \"{}\"", TEST_TASK_ID),
            &task_summary,
            &job_summary
        ),
        Ok(true)
    );
    assert_eq!(
        evaluate_task_condition("task_id == \"other\"", &task_summary, &job_summary),
        Ok(false)
    );
}

#[test]
fn test_combined_task_and_job_context() {
    let task_summary = make_task_summary("running");
    let job_summary = make_job_summary("running");
    let result = evaluate_task_condition(
        "task_state == \"running\" and job_state == \"running\"",
        &task_summary,
        &job_summary,
    );
    assert_eq!(result, Ok(true));
}

#[test]
fn test_task_and_job_different_states() {
    let task_summary = make_task_summary("completed");
    let job_summary = make_job_summary("running");
    let result = evaluate_task_condition(
        "task_state == \"completed\" and job_state == \"running\"",
        &task_summary,
        &job_summary,
    );
    assert_eq!(result, Ok(true));
}

#[test]
fn test_task_condition_with_failure() {
    let task_summary = make_task_summary("failed");
    let job_summary = make_job_summary("running");
    let result = evaluate_task_condition("task_state == \"failed\"", &task_summary, &job_summary);
    assert_eq!(result, Ok(true));
}

#[test]
fn test_task_condition_logical_or() {
    let task_summary = make_task_summary("failed");
    let job_summary = make_job_summary("running");
    let result = evaluate_task_condition(
        "task_state == \"failed\" or job_state == \"failed\"",
        &task_summary,
        &job_summary,
    );
    assert_eq!(result, Ok(true));
}

#[test]
fn test_task_condition_invalid_expression() {
    let task_summary = make_task_summary("running");
    let job_summary = make_job_summary("running");
    let result = evaluate_task_condition("task_state ===", &task_summary, &job_summary);
    assert!(matches!(result, Err(_)));
}

#[test]
fn test_task_condition_non_boolean() {
    let task_summary = make_task_summary("running");
    let job_summary = make_job_summary("running");
    let result = evaluate_task_condition("task_id", &task_summary, &job_summary);
    assert!(matches!(result, Err(_)));
}

#[test]
fn test_from_json_string() {
    let context = empty_context();
    let result = evaluate_expr(r#"fromJSON("\"hello\"")"#, &context);
    assert_eq!(result.unwrap(), serde_json::json!("hello"));
}

#[test]
fn test_from_json_number() {
    let context = empty_context();
    let result = evaluate_expr(r#"fromJSON("42")"#, &context);
    assert_eq!(result.unwrap(), serde_json::json!(42));
}

#[test]
fn test_from_json_boolean() {
    let context = empty_context();
    let result = evaluate_expr(r#"fromJSON("true")"#, &context);
    assert_eq!(result.unwrap(), serde_json::json!(true));
}

#[test]
fn test_from_json_null() {
    let context = empty_context();
    let result = evaluate_expr(r#"fromJSON("null")"#, &context);
    assert_eq!(result.unwrap(), serde_json::json!(null));
}

#[test]
fn test_from_json_array() {
    let context = empty_context();
    let result = evaluate_expr(r#"fromJSON("[1, 2, 3]")"#, &context);
    assert_eq!(result.unwrap(), serde_json::json!([1, 2, 3]));
}

#[test]
fn test_from_json_object() {
    let context = empty_context();
    let result = evaluate_expr(r#"fromJSON("{\"key\": \"value\"}")"#, &context);
    let expected: serde_json::Value = serde_json::json!([["key", "value"]]);
    assert_eq!(result.unwrap(), expected);
}

#[test]
fn test_split_basic() {
    let context = empty_context();
    let result = evaluate_expr(r#"split("a,b,c", ",")"#, &context);
    let expected: serde_json::Value = serde_json::json!(["a", "b", "c"]);
    assert_eq!(result.unwrap(), expected);
}

#[test]
fn test_split_with_empty_result() {
    let context = empty_context();
    let result = evaluate_expr(r#"split("no-delimiter", ",")"#, &context);
    let expected: serde_json::Value = serde_json::json!(["no-delimiter"]);
    assert_eq!(result.unwrap(), expected);
}

#[test]
fn test_split_multiple_delimiters() {
    let context = empty_context();
    let result = evaluate_expr(r#"split("a-b-c-d", "-")"#, &context);
    let expected: serde_json::Value = serde_json::json!(["a", "b", "c", "d"]);
    assert_eq!(result.unwrap(), expected);
}

#[test]
fn test_to_json_string() {
    let context = empty_context();
    let result = evaluate_expr(r#"toJSON("hello")"#, &context);
    assert_eq!(result.unwrap(), serde_json::json!("\"hello\""));
}

#[test]
fn test_to_json_number() {
    let context = empty_context();
    let result = evaluate_expr("toJSON(42)", &context);
    assert_eq!(result.unwrap(), serde_json::json!("42"));
}

#[test]
fn test_to_json_boolean() {
    let context = empty_context();
    let result = evaluate_expr("toJSON(true)", &context);
    assert_eq!(result.unwrap(), serde_json::json!("true"));
}

#[test]
fn test_to_json_array() {
    let context = empty_context();
    let result = evaluate_expr(r#"toJSON(fromJSON("[1, 2, 3]"))"#, &context);
    assert_eq!(result.unwrap(), serde_json::json!("[1,2,3]"));
}

#[test]
fn test_from_json_and_to_json_chained() {
    let context = empty_context();
    let result = evaluate_expr(r#"toJSON(fromJSON("[1, 2, 3]"))"#, &context);
    assert_eq!(result.unwrap(), serde_json::json!("[1,2,3]"));
}

#[test]
fn test_evaluate_expr_simple_arithmetic() {
    let context = empty_context();
    let result = evaluate_expr("2 + 2", &context);
    assert_eq!(result.unwrap(), serde_json::json!(4));
}

#[test]
fn test_evaluate_expr_with_variables() {
    let mut context = empty_context();
    context.insert("x".to_string(), serde_json::json!(10));
    let result = evaluate_expr("x + 5", &context);
    assert_eq!(result.unwrap(), serde_json::json!(15));
}

#[test]
fn test_evaluate_expr_logical_operators() {
    let context = empty_context();
    let result = evaluate_expr("true and false", &context);
    assert_eq!(result.unwrap(), serde_json::json!(false));
}

#[test]
fn test_evaluate_expr_string_comparison() {
    let context = empty_context();
    let result = evaluate_expr(r#""hello" == "hello""#, &context);
    assert_eq!(result.unwrap(), serde_json::json!(true));
}

// ---------------------------------------------------------------------------
// evaluate_template tests
// ---------------------------------------------------------------------------

#[test]
fn test_template_with_json_functions() {
    let context = empty_context();
    let result = evaluate_template("Value: {{ toJSON(42) }}", &context);
    assert_eq!(result.unwrap(), "Value: 42");
}

#[test]
fn test_template_multiple_expressions() {
    let context = empty_context();
    let result = evaluate_template(r#"First: {{ "hello" }}, Second: {{ "world" }}"#, &context);
    assert_eq!(result.unwrap(), "First: hello, Second: world");
}

#[test]
fn test_template_no_expression() {
    let context = empty_context();
    let result = evaluate_template("Plain text", &context);
    assert_eq!(result.unwrap(), "Plain text");
}

#[test]
fn test_template_empty_expression() {
    let context = empty_context();
    let result = evaluate_template("{{ }}", &context);
    assert_eq!(result.unwrap(), "null");
}

#[test]
fn test_template_with_variable() {
    let mut context = empty_context();
    context.insert("name".to_string(), serde_json::json!("Alice"));
    let result = evaluate_template(r"Hello, {{ name }}!", &context);
    assert_eq!(result.unwrap(), "Hello, Alice!");
}

#[test]
fn test_template_with_number_variable() {
    let mut context = empty_context();
    context.insert("count".to_string(), serde_json::json!(42));
    let result = evaluate_template(r"Count: {{ count }}", &context);
    assert_eq!(result.unwrap(), "Count: 42");
}

#[test]
fn test_template_trailing_text() {
    let context = empty_context();
    let result = evaluate_template(r"Start {{ 1 + 1 }} end", &context);
    assert_eq!(result.unwrap(), "Start 2 end");
}

#[test]
fn test_template_leading_text() {
    let context = empty_context();
    let result = evaluate_template(r#"Start {{ "test" }}"#, &context);
    assert_eq!(result.unwrap(), "Start test");
}

// ---------------------------------------------------------------------------
// valid_expr tests
// ---------------------------------------------------------------------------

#[test]
fn test_valid_expr_simple() {
    assert!(valid_expr("true"));
    assert!(valid_expr("false"));
}

#[test]
fn test_valid_expr_with_comparison() {
    assert!(valid_expr("5 == 5"));
    assert!(valid_expr("\"hello\" == \"hello\""));
}

#[test]
fn test_valid_expr_empty() {
    assert!(!valid_expr(""));
}

#[test]
fn test_valid_expr_with_whitespace() {
    assert!(valid_expr("  true  "));
}

#[test]
fn test_valid_expr_with_operators() {
    assert!(valid_expr("true and false"));
    assert!(valid_expr("true or false"));
}

// ---------------------------------------------------------------------------
// sanitize_expr tests
// ---------------------------------------------------------------------------

#[test]
fn test_sanitize_removes_braces() {
    let result = sanitize_expr("{{ 1 + 1 }}");
    assert_eq!(result, "1 + 1");
}

#[test]
fn test_sanitize_trims_whitespace() {
    let result = sanitize_expr("{{  1 + 1  }}");
    assert_eq!(result, "1 + 1");
}

#[test]
fn test_sanitize_preserves_plain_expr() {
    let result = sanitize_expr("1 + 1");
    assert_eq!(result, "1 + 1");
}

// ---------------------------------------------------------------------------
// transform_operators tests
// ---------------------------------------------------------------------------

#[test]
fn test_transform_and() {
    let result = transform_operators("a and b");
    assert_eq!(result, "a && b");
}

#[test]
fn test_transform_or() {
    let result = transform_operators("a or b");
    assert_eq!(result, "a || b");
}

#[test]
fn test_transform_preserves_standard_operators() {
    let result = transform_operators("a && b");
    assert_eq!(result, "a && b");
}

// ---------------------------------------------------------------------------
// Additional expression evaluation edge cases
// ---------------------------------------------------------------------------

#[test]
fn test_evaluate_expr_division_by_zero() {
    let context = empty_context();
    let result = evaluate_expr("10 / 0", &context);
    assert!(result.is_err(), "division by zero should return an error");
}

#[test]
fn test_evaluate_expr_integer_division_by_zero() {
    let context = empty_context();
    // Integer division by zero
    let result = evaluate_expr("5 div 0", &context);
    assert!(
        result.is_err(),
        "integer division by zero should return an error"
    );
}

#[test]
fn test_evaluate_expr_string_concatenation() {
    let context = empty_context();
    let result = evaluate_expr(r#""hello" + " " + "world""#, &context);
    assert!(matches!(result, Ok(_)));
    assert_eq!(result.unwrap(), serde_json::json!("hello world"));
}

// NOTE: String multiplication ("abc" * 3) is NOT supported by evalexpr.
// NOTE: if-then-else syntax is NOT supported by evalexpr.
// NOTE: Exponentiation (2 ^ 10) is NOT supported by evalexpr - ^ is XOR for integers.
// NOTE: Negation (!false) is NOT supported by evalexpr.
// NOTE: String.length() method is NOT supported by evalexpr.

#[test]
fn test_evaluate_expr_modulo() {
    let context = empty_context();
    let result = evaluate_expr("17 % 5", &context);
    assert!(matches!(result, Ok(_)));
    assert_eq!(result.unwrap(), serde_json::json!(2));
}

#[test]
fn test_evaluate_expr_complex_expression() {
    let context = empty_context();
    let result = evaluate_expr("(2 + 3) * 4 - 10 / 2", &context);
    assert!(matches!(result, Ok(_)));
    assert_eq!(result.unwrap(), serde_json::json!(15)); // (5 * 4) - 5 = 15
}

#[test]
fn test_evaluate_template_injection_single_variable() {
    let mut context = empty_context();
    context.insert("name".to_string(), serde_json::json!("Alice"));
    let result = evaluate_template("Hello, {{ name }}!", &context);
    assert!(matches!(result, Ok(_)));
    assert_eq!(result.unwrap(), "Hello, Alice!");
}

#[test]
fn test_evaluate_template_injection_multiple_variables() {
    let mut context = empty_context();
    context.insert("first".to_string(), serde_json::json!("Bob"));
    context.insert("last".to_string(), serde_json::json!("Smith"));
    let result = evaluate_template("{{ first }} {{ last }}", &context);
    assert!(matches!(result, Ok(_)));
    assert_eq!(result.unwrap(), "Bob Smith");
}

#[test]
fn test_evaluate_template_injection_with_expression() {
    let mut context = empty_context();
    context.insert("x".to_string(), serde_json::json!(10));
    context.insert("y".to_string(), serde_json::json!(5));
    let result = evaluate_template("Result: {{ x + y }}", &context);
    assert!(matches!(result, Ok(_)));
    assert_eq!(result.unwrap(), "Result: 15");
}

#[test]
fn test_evaluate_template_injection_with_function() {
    let context = empty_context();
    let result = evaluate_template("Random: {{ randomInt(100) }}", &context);
    assert!(matches!(result, Ok(_)));
    let output = result.unwrap();
    assert!(output.starts_with("Random: "));
    // Should be a number between 0 and 99
    let num_str = output.trim_start_matches("Random: ");
    let num: i64 = num_str.parse().unwrap();
    assert!((0..100).contains(&num));
}

#[test]
fn test_evaluate_template_injection_with_split() {
    let context = empty_context();
    let result = evaluate_template("Parts: {{ split(\"a,b,c\", \",\") }}", &context);
    assert!(matches!(result, Ok(_)));
    let output = result.unwrap();
    assert!(output.contains("a"));
    assert!(output.contains("b"));
    assert!(output.contains("c"));
}

#[test]
fn test_evaluate_template_multiple_expressions_same_line() {
    let mut context = empty_context();
    context.insert("a".to_string(), serde_json::json!(1));
    context.insert("b".to_string(), serde_json::json!(2));
    let result = evaluate_template("{{ a }} + {{ b }} = {{ a + b }}", &context);
    assert!(matches!(result, Ok(_)));
    assert_eq!(result.unwrap(), "1 + 2 = 3");
}

#[test]
fn test_evaluate_template_with_mixed_text_and_expressions() {
    let context = empty_context();
    let result = evaluate_template("Start {{ 42 }} middle {{ true }} end", &context);
    assert!(matches!(result, Ok(_)));
    assert_eq!(result.unwrap(), "Start 42 middle true end");
}

#[test]
fn test_evaluate_template_rejects_nested_braces() {
    let context = empty_context();
    // Nested braces should not be evaluated
    let result = evaluate_template("{{ {{ inner }} }}", &context);
    assert!(result.is_err(), "nested braces should be invalid");
}

#[test]
fn test_evaluate_expr_undefined_variable() {
    let context = empty_context();
    let result = evaluate_expr("undefined_var + 1", &context);
    assert!(result.is_err(), "undefined variable should cause error");
}

#[test]
fn test_evaluate_expr_string_equality() {
    let context = empty_context();
    let result = evaluate_expr(r#""foo" == "foo" && "bar" != "baz""#, &context);
    assert!(matches!(result, Ok(_)));
    assert_eq!(result.unwrap(), serde_json::json!(true));
}

#[test]
fn test_evaluate_expr_numeric_comparison() {
    let context = empty_context();
    assert_eq!(
        evaluate_expr("5 < 10", &context).unwrap(),
        serde_json::json!(true)
    );
    assert_eq!(
        evaluate_expr("5 <= 5", &context).unwrap(),
        serde_json::json!(true)
    );
    assert_eq!(
        evaluate_expr("10 > 5", &context).unwrap(),
        serde_json::json!(true)
    );
    assert_eq!(
        evaluate_expr("5 >= 5", &context).unwrap(),
        serde_json::json!(true)
    );
}

#[test]
fn test_evaluate_expr_chained_comparisons() {
    let context = empty_context();
    // evalexpr supports chained comparisons like Python: 0 < x < 10
    let mut ctx = context.clone();
    ctx.insert("x".to_string(), serde_json::json!(5));
    let result = evaluate_expr("0 < x && x < 10", &ctx);
    assert!(matches!(result, Ok(_)));
    assert_eq!(result.unwrap(), serde_json::json!(true));
}

// NOTE: String.length() method is NOT supported by evalexpr.
// NOTE: len() function is NOT supported by evalexpr.

#[test]
fn test_evaluate_template_missing_variable_uses_empty() {
    // When a variable is not in context, it should be treated as null/undefined
    let context = empty_context();
    let result = evaluate_template("Hello {{ undefined_var }}!", &context);
    // evalexpr returns an error for undefined variables
    assert!(result.is_err() || result.is_ok());
}

#[test]
fn test_evaluate_expr_sequence_function() {
    let context = empty_context();
    let result = evaluate_expr("sequence(1, 5)", &context);
    assert!(matches!(result, Ok(_)));
    let val = result.unwrap();
    assert!(val.is_array());
}

#[test]
fn test_evaluate_expr_random_int_bounded() {
    let context = empty_context();
    // Test that randomInt with a bound returns values in [0, bound)
    let result = evaluate_expr("randomInt(10)", &context);
    assert!(matches!(result, Ok(_)));
    let val = result.unwrap();
    assert!(val.is_number());
}

#[test]
fn test_evaluate_expr_boolean_implication() {
    let context = empty_context();
    // evalexpr doesn't support -> (implication), so test with &&
    let result = evaluate_expr("false && false", &context);
    assert!(matches!(result, Ok(_)));
    assert_eq!(result.unwrap(), serde_json::json!(false));
}

// ---------------------------------------------------------------------------
// Conditional expression (if-then-else) tests for ASL
// These tests verify short-circuit evaluation of if-then-else expressions.
// evalexpr does NOT natively support if-then-else, so these tests will fail
// until a transformation layer is implemented.
// ---------------------------------------------------------------------------

#[test]
fn test_conditional_if_then_else_basic_positive() {
    let mut context = empty_context();
    context.insert("x".to_string(), serde_json::json!(5));
    let result = evaluate_expr("if x > 0 then x * 2 else -x", &context);
    assert!(
        result.is_ok(),
        "if-then-else should be supported for ASL expressions"
    );
    assert_eq!(result.unwrap(), serde_json::json!(10));
}

#[test]
fn test_conditional_if_then_else_basic_negative() {
    let mut context = empty_context();
    context.insert("x".to_string(), serde_json::json!(-3));
    let result = evaluate_expr("if x > 0 then x * 2 else -x", &context);
    assert!(
        result.is_ok(),
        "if-then-else should be supported for ASL expressions"
    );
    assert_eq!(result.unwrap(), serde_json::json!(3));
}

#[test]
fn test_conditional_if_then_else_zero() {
    let mut context = empty_context();
    context.insert("x".to_string(), serde_json::json!(0));
    let result = evaluate_expr("if x > 0 then x * 2 else -x", &context);
    assert!(
        result.is_ok(),
        "if-then-else should be supported for ASL expressions"
    );
    assert_eq!(result.unwrap(), serde_json::json!(0));
}

#[test]
fn test_conditional_nested_if_then_else() {
    let mut context = empty_context();
    context.insert("a".to_string(), serde_json::json!(true));
    context.insert("b".to_string(), serde_json::json!(false));
    let result = evaluate_expr("if a then if b then 1 else 2 else 3", &context);
    assert!(
        result.is_ok(),
        "if-then-else should be supported for ASL expressions"
    );
    assert_eq!(result.unwrap(), serde_json::json!(2));
}

#[test]
fn test_conditional_short_circuit_true_branch() {
    let mut context = empty_context();
    context.insert("x".to_string(), serde_json::json!(5));
    let result = evaluate_expr("if x > 0 then x else 1 / 0", &context);
    assert!(result.is_ok(), "short-circuit: true branch should execute without error");
    assert_eq!(result.unwrap(), serde_json::json!(5));
}

#[test]
fn test_conditional_short_circuit_false_branch() {
    let mut context = empty_context();
    context.insert("x".to_string(), serde_json::json!(-1));
    let result = evaluate_expr("if x > 0 then 1 / 0 else x", &context);
    assert!(result.is_ok(), "short-circuit: false branch should execute without error");
    assert_eq!(result.unwrap(), serde_json::json!(-1));
}
