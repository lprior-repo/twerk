//! Tests for the eval module

use std::collections::HashMap;
use twerk_core::eval::{
    evaluate_condition, evaluate_expr, evaluate_task_condition, evaluate_template, sanitize_expr,
    transform_operators, valid_expr,
};
use twerk_core::id::{JobId, TaskId};
use twerk_core::job::JobSummary;
use twerk_core::task::TaskSummary;

fn make_job_summary(state: &str) -> JobSummary {
    JobSummary {
        id: Some(JobId::new("job-123")),
        name: Some("test-job".to_string()),
        state: state.to_string(),
        error: None,
        ..Default::default()
    }
}

fn make_task_summary(state: &str) -> TaskSummary {
    TaskSummary {
        id: Some(TaskId::new("task-456")),
        job_id: Some(JobId::new("job-123")),
        state: state.to_string(),
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
        evaluate_condition("job_id == \"job-123\"", &summary),
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
        "job_state == \"running\" and job_id == \"job-123\"",
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
        "job_state == \"completed\" or job_id == \"job-123\"",
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
        "(job_state == \"running\" or job_state == \"pending\") and job_id == \"job-123\"",
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
    assert!(result.is_err());
}

#[test]
fn test_evaluate_condition_non_boolean_result() {
    let summary = make_job_summary("running");
    let result = evaluate_condition("\"hello\"", &summary);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .contains("did not evaluate to a boolean"));
}

#[test]
fn test_evaluate_condition_invalid_expression() {
    let summary = make_job_summary("running");
    let result = evaluate_condition("job_state ===", &summary);
    assert!(result.is_err());
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
        evaluate_task_condition("task_id == \"task-456\"", &task_summary, &job_summary),
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
    assert!(result.is_err());
}

#[test]
fn test_task_condition_non_boolean() {
    let task_summary = make_task_summary("running");
    let job_summary = make_job_summary("running");
    let result = evaluate_task_condition("task_id", &task_summary, &job_summary);
    assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// evaluate_expr tests
// ---------------------------------------------------------------------------

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
    let result = evaluate_template(r#"Value: {{ toJSON(42) }}"#, &context);
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
    let result = evaluate_template(r#"Hello, {{ name }}!"#, &context);
    assert_eq!(result.unwrap(), "Hello, Alice!");
}

#[test]
fn test_template_with_number_variable() {
    let mut context = empty_context();
    context.insert("count".to_string(), serde_json::json!(42));
    let result = evaluate_template(r#"Count: {{ count }}"#, &context);
    assert_eq!(result.unwrap(), "Count: 42");
}

#[test]
fn test_template_trailing_text() {
    let context = empty_context();
    let result = evaluate_template(r#"Start {{ 1 + 1 }} end"#, &context);
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
