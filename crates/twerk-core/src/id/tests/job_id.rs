use std::collections::HashSet;

use super::super::{IdError, JobId};

const TEST_JOB_ID: &str = "550e8400-e29b-41d4-a716-446655440000";
const TEST_JOB_ID_2: &str = "agHsjbVsDHD2e3ZJ4VfwKw";

#[allow(clippy::redundant_pattern_matching)]
#[test]
fn job_id_returns_ok_when_input_is_valid_uuid() {
    let result = JobId::new(TEST_JOB_ID);
    assert!(matches!(result, Ok(_)));
    assert_eq!(result.unwrap().as_str(), TEST_JOB_ID);
}

#[allow(clippy::redundant_pattern_matching)]
#[test]
fn job_id_returns_ok_with_short_runtime_id() {
    let result = JobId::new(TEST_JOB_ID_2);
    assert!(matches!(result, Ok(_)));
}

#[test]
fn job_id_returns_err_for_short_ascii_identifier_not_in_supported_shape() {
    let result = JobId::new("job-123");
    assert!(matches!(result, Err(IdError::InvalidJobIdFormat)));
}

#[test]
fn job_id_returns_err_empty_string() {
    let result = JobId::new("");
    assert!(matches!(result, Err(IdError::Empty)));
}

#[test]
fn job_id_returns_err_with_newline() {
    let result = JobId::new("550e8400-e29b-41d4-a716\n-446655440000");
    assert!(matches!(result, Err(IdError::InvalidJobIdFormat)));
}

#[test]
fn job_id_returns_err_with_null_byte() {
    let result = JobId::new("550e8400-e29b-41d4-a716-44665\x005440000");
    assert!(matches!(result, Err(IdError::InvalidJobIdFormat)));
}

#[test]
fn job_id_returns_err_with_slash() {
    let result = JobId::new("550e8400-e29b-41d4-a716/446655440000");
    assert!(matches!(result, Err(IdError::InvalidJobIdFormat)));
}

#[test]
fn job_id_returns_err_with_space() {
    let result = JobId::new("550e8400 e29b-41d4-a716-446655440000");
    assert!(matches!(result, Err(IdError::InvalidJobIdFormat)));
}

#[test]
fn job_id_returns_err_with_tab() {
    let result = JobId::new("550e8400-\te29b-41d4-a716-446655440000");
    assert!(matches!(result, Err(IdError::InvalidJobIdFormat)));
}

#[test]
fn job_id_returns_err_too_long() {
    let over_max = "a".repeat(1001);
    let result = JobId::new(&over_max);
    assert!(matches!(result, Err(IdError::TooLong(1001))));
}

#[test]
fn job_id_with_unicode_shapes_is_rejected() {
    assert!(matches!(
        JobId::new("550e8400-e29b-41d4-a716-日本語-440000"),
        Err(IdError::InvalidJobIdFormat)
    ));
    assert!(matches!(
        JobId::new("日本語"),
        Err(IdError::InvalidJobIdFormat)
    ));
    assert!(matches!(
        JobId::new("550e8400-e29b-41d4-🔥-446655440000"),
        Err(IdError::InvalidJobIdFormat)
    ));
}

#[test]
fn job_id_returns_err_with_special_chars() {
    [
        "550e8400!e29b-41d4-a716-446655440000",
        "550e8400@e29b-41d4-a716-446655440000",
        "550e8400#e29b-41d4-a716-446655440000",
    ]
    .into_iter()
    .for_each(|id| {
        let result = JobId::new(id);
        assert!(matches!(result, Err(IdError::InvalidJobIdFormat)));
    });
}

#[test]
fn job_id_new_validates_owned_and_borrowed_inputs() {
    assert_eq!(
        JobId::new(TEST_JOB_ID.to_string()).unwrap().as_str(),
        TEST_JOB_ID
    );
    assert_eq!(JobId::new(TEST_JOB_ID).unwrap().as_str(), TEST_JOB_ID);
}

#[test]
fn job_id_display_trait() {
    let id = JobId::new(TEST_JOB_ID).unwrap();
    assert_eq!(format!("{id}"), TEST_JOB_ID);
}

#[test]
fn job_id_string_access_traits_return_original() {
    let id = JobId::new(TEST_JOB_ID).unwrap();
    let as_ref_value: &str = id.as_ref();
    let deref_value: &str = &id;
    let borrow_value: &str = std::borrow::Borrow::borrow(&id);
    assert_eq!(as_ref_value, TEST_JOB_ID);
    assert_eq!(deref_value, TEST_JOB_ID);
    assert_eq!(borrow_value, TEST_JOB_ID);
}

#[test]
fn job_id_from_str_trait() {
    let parsed: JobId = TEST_JOB_ID.parse().unwrap();
    assert_eq!(parsed.as_str(), TEST_JOB_ID);
}

#[test]
fn job_id_from_str_trait_error() {
    let result: Result<JobId, _> = "not a uuid".parse();
    assert!(matches!(result, Err(IdError::InvalidJobIdFormat)));
}

#[test]
fn job_id_eq_and_hash() {
    let id1 = JobId::new(TEST_JOB_ID).unwrap();
    let id2 = JobId::new(TEST_JOB_ID).unwrap();
    let id3 = JobId::new(TEST_JOB_ID_2).unwrap();

    assert_eq!(id1, id2);
    assert_ne!(id1, id3);

    let mut set = HashSet::new();
    set.insert(id1.clone());
    set.insert(id2.clone());
    set.insert(id3);
    assert_eq!(set.len(), 2);
}

#[test]
fn job_id_clone_and_default() {
    let id = JobId::new(TEST_JOB_ID).unwrap();
    assert_eq!(id.clone(), id);
    assert_eq!(JobId::default().as_str(), "");
}

#[test]
fn job_id_deserialize_rejects_invalid_shape() {
    let parsed = serde_json::from_str::<JobId>("\"job-123\"");
    assert!(parsed.is_err());
}
