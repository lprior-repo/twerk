use super::super::common::validate_id;
use super::super::{IdError, NodeId, RoleId, ScheduledJobId, TaskId, UserId};

#[allow(clippy::redundant_pattern_matching)]
#[test]
fn task_id_new_valid() {
    let result = TaskId::new("task-abc-123");
    assert!(matches!(result, Ok(_)));
}

#[test]
fn task_id_new_empty_rejected() {
    let result = TaskId::new("");
    assert!(matches!(result, Err(IdError::Empty)));
}

#[allow(clippy::redundant_pattern_matching)]
#[test]
fn node_id_new_valid() {
    let result = NodeId::new("node-xyz-789");
    assert!(matches!(result, Ok(_)));
}

#[test]
fn node_id_new_too_long_rejected() {
    let over_max = "x".repeat(1001);
    let result = NodeId::new(&over_max);
    assert!(matches!(result, Err(IdError::TooLong(1001))));
}

#[allow(clippy::redundant_pattern_matching)]
#[test]
fn user_id_new_valid() {
    let result = UserId::new("user-123");
    assert!(matches!(result, Ok(_)));
}

#[test]
fn user_id_new_with_invalid_chars_rejected() {
    let result = UserId::new("user@123");
    assert!(matches!(result, Err(IdError::InvalidCharacters)));
}

#[allow(clippy::redundant_pattern_matching)]
#[test]
fn role_id_new_valid() {
    let result = RoleId::new("role-admin");
    assert!(matches!(result, Ok(_)));
}

#[test]
fn role_id_new_empty_rejected() {
    let result = RoleId::new("");
    assert!(matches!(result, Err(IdError::Empty)));
}

#[allow(clippy::redundant_pattern_matching)]
#[test]
fn scheduled_job_id_new_valid() {
    let result = ScheduledJobId::new("scheduled-job-123");
    assert!(matches!(result, Ok(_)));
}

#[test]
fn validate_id_rejects_empty() {
    assert!(validate_id("").is_err());
}

#[test]
fn validate_id_rejects_only_whitespace() {
    assert!(validate_id(" ").is_err());
    assert!(validate_id("  ").is_err());
}

#[test]
fn validate_id_accepts_single_alphanumeric() {
    assert!(validate_id("a").is_ok());
    assert!(validate_id("1").is_ok());
    assert!(validate_id("A").is_ok());
}

#[test]
fn validate_id_accepts_dash_and_underscore() {
    assert!(validate_id("a-b").is_ok());
    assert!(validate_id("a_b").is_ok());
    assert!(validate_id("a-b_c-d").is_ok());
}

#[test]
fn validate_id_rejects_dots() {
    assert!(validate_id("file.txt").is_err());
}

#[test]
fn validate_id_max_length_boundary() {
    let max_valid = "a".repeat(1000);
    assert!(validate_id(&max_valid).is_ok());

    let over_one = "a".repeat(1001);
    assert!(validate_id(&over_one).is_err());
}

#[test]
fn validate_id_rejects_control_characters() {
    assert!(validate_id("test\x00value").is_err());
    assert!(validate_id("test\x01value").is_err());
    assert!(validate_id("test\x7fvalue").is_err());
}

#[test]
fn validate_id_error_messages_are_descriptive() {
    let empty_err = validate_id("").unwrap_err();
    assert!(empty_err.to_string().contains("empty"));

    let too_long_err = validate_id(&"x".repeat(1001)).unwrap_err();
    assert!(too_long_err.to_string().contains("1001"));

    let invalid_err = validate_id("bad@char").unwrap_err();
    assert!(invalid_err.to_string().contains("invalid"));
}
