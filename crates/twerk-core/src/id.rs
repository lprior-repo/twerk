use serde::{Deserialize, Serialize};
use std::borrow::Borrow;
use std::fmt;
use std::ops::Deref;
use std::str::FromStr;
use thiserror::Error;
use utoipa::ToSchema;

const MAX_ID_LENGTH: usize = 1000;

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum IdError {
    #[error("ID cannot be empty")]
    Empty,
    #[error("ID is too short: {0} characters (minimum 3)")]
    TooShort(usize),
    #[error("ID is too long: {0} characters (maximum {MAX_ID_LENGTH})")]
    TooLong(usize),
    #[error("ID contains invalid characters: only alphanumeric, dash, and underscore allowed")]
    InvalidCharacters,
    #[error("invalid UUID format: expected RFC 4122 compliant UUID")]
    InvalidUuid,
}

fn validate_id(s: &str) -> Result<(), IdError> {
    if s.is_empty() {
        return Err(IdError::Empty);
    }
    if s.len() > MAX_ID_LENGTH {
        return Err(IdError::TooLong(s.len()));
    }
    if !s
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        return Err(IdError::InvalidCharacters);
    }
    Ok(())
}

macro_rules! define_id {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default, ToSchema)]
        #[serde(transparent)]
        #[schema(value_type = String)]
        pub struct $name(String);

        impl $name {
            /// Creates a new ID from a string.
            ///
            /// # Errors
            ///
            /// Returns `IdError` if the string is empty, too long, or contains invalid characters.
            pub fn new(id: impl Into<String>) -> Result<Self, IdError> {
                let s = id.into();
                validate_id(&s)?;
                Ok(Self(s))
            }

            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl From<String> for $name {
            fn from(s: String) -> Self {
                Self(s)
            }
        }

        impl From<&str> for $name {
            fn from(s: &str) -> Self {
                Self(s.to_string())
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }

        impl Deref for $name {
            type Target = str;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl Borrow<str> for $name {
            fn borrow(&self) -> &str {
                &self.0
            }
        }

        impl FromStr for $name {
            type Err = IdError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Self::new(s)
            }
        }
    };
}

// =========================================================================
// JobId — hand-written to enforce UUID format validation
// =========================================================================

/// Validated identifier for a job, must be a valid RFC 4122 UUID.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default, utoipa::ToSchema)]
#[serde(transparent)]
#[schema(value_type = String)]
pub struct JobId(pub String);

impl JobId {
    /// Creates a new JobId from a UUID string.
    ///
    /// # Errors
    ///
    /// Returns `IdError::InvalidUuid` if the string is not a valid RFC 4122 UUID.
    pub fn new(uuid: impl Into<String>) -> Result<Self, IdError> {
        let s = uuid.into();
        if uuid::Uuid::parse_str(&s).is_ok() {
            Ok(JobId(s))
        } else {
            Err(IdError::InvalidUuid)
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl utoipa::PartialSchema for JobId {
    fn schema() -> utoipa::openapi::RefOr<utoipa::openapi::schema::Schema> {
        <String as utoipa::PartialSchema>::schema()
    }
}

impl utoipa::ToSchema for JobId {
    fn name() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed("string")
    }
}

impl From<String> for JobId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for JobId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl fmt::Display for JobId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for JobId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Deref for JobId {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Borrow<str> for JobId {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl FromStr for JobId {
    type Err = IdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

define_id!(TaskId);
define_id!(NodeId);
define_id!(ScheduledJobId);
define_id!(UserId);
define_id!(RoleId);

// Implement PartialSchema and ToSchema for all ID types
// This allows them to be used in structs with #[derive(ToSchema)]
macro_rules! impl_partial_schema_for_id {
    ($name:ident) => {
        impl utoipa::PartialSchema for $name {
            fn schema() -> utoipa::openapi::RefOr<utoipa::openapi::schema::Schema> {
                <String as utoipa::PartialSchema>::schema()
            }
        }

        impl utoipa::ToSchema for $name {
            fn name() -> std::borrow::Cow<'static, str> {
                std::borrow::Cow::Borrowed("string")
            }
        }
    };
}

impl_partial_schema_for_id!(TaskId);
impl_partial_schema_for_id!(NodeId);
impl_partial_schema_for_id!(ScheduledJobId);
impl_partial_schema_for_id!(UserId);
impl_partial_schema_for_id!(RoleId);

// =========================================================================
// TriggerId — hand-written (NOT using define_id!) to enforce 3-64 length
// =========================================================================

/// Validated identifier for a trigger instance.
///
/// Construction via [`TriggerId::new`] enforces:
/// - Length 3..=64 characters (inclusive)
/// - Characters: `[a-zA-Z0-9_-]` (plus Unicode alphanumeric per Rust's `is_alphanumeric()`)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Default, utoipa::ToSchema)]
#[serde(transparent)]
pub struct TriggerId(pub String);

impl<'de> Deserialize<'de> for TriggerId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s: String = String::deserialize(deserializer)?;
        Self::new(s).map_err(serde::de::Error::custom)
    }
}

impl utoipa::PartialSchema for TriggerId {
    fn schema() -> utoipa::openapi::RefOr<utoipa::openapi::schema::Schema> {
        <String as utoipa::PartialSchema>::schema()
    }
}

impl utoipa::ToSchema for TriggerId {
    fn name() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed("string")
    }
}

impl TriggerId {
    /// Minimum length for a valid `TriggerId`.
    const MIN_LENGTH: usize = 3;

    /// Maximum length for a valid `TriggerId`.
    const MAX_LENGTH: usize = 64;

    /// Creates a new `TriggerId` from a string.
    ///
    /// # Errors
    ///
    /// Returns `IdError::Empty` if the string is empty.
    /// Returns `IdError::TooShort(len)` if `len < 3`.
    /// Returns `IdError::TooLong(len)` if `len > 64`.
    /// Returns `IdError::InvalidCharacters` if any character is not
    /// alphanumeric, `-`, or `_`.
    pub fn new(id: impl Into<String>) -> Result<Self, IdError> {
        let s = id.into();
        if s.is_empty() {
            return Err(IdError::Empty);
        }
        if s.len() < Self::MIN_LENGTH {
            return Err(IdError::TooShort(s.len()));
        }
        if s.len() > Self::MAX_LENGTH {
            return Err(IdError::TooLong(s.len()));
        }
        if !s
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            return Err(IdError::InvalidCharacters);
        }
        Ok(Self(s))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for TriggerId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for TriggerId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl fmt::Display for TriggerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for TriggerId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Deref for TriggerId {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Borrow<str> for TriggerId {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl FromStr for TriggerId {
    type Err = IdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    // =======================================================================
    // JobId validation tests
    // =======================================================================

    /// Valid UUID for testing - RFC 4122 format
    const TEST_UUID: &'static str = "550e8400-e29b-41d4-a716-446655440000";
    /// Different valid UUID for testing equality
    const TEST_UUID_2: &'static str = "661f9501-f30c-52e5-b827-557766551111";

    #[allow(clippy::redundant_pattern_matching)]
    #[test]
    fn job_id_returns_ok_when_input_is_valid_uuid() {
        let result = JobId::new(TEST_UUID);
        assert!(matches!(result, Ok(_)));
        assert_eq!(result.unwrap().as_str(), TEST_UUID);
    }

    #[allow(clippy::redundant_pattern_matching)]
    #[test]
    fn job_id_returns_ok_with_uppercase_uuid() {
        // UUIDs can be uppercase too
        let result = JobId::new("550E8400-E29B-41D4-A716-446655440000");
        assert!(matches!(result, Ok(_)));
    }

    #[test]
    fn job_id_returns_err_empty_string() {
        let result = JobId::new("");
        assert!(matches!(result, Err(IdError::InvalidUuid)));
    }

    #[test]
    fn job_id_returns_err_with_newline() {
        // newline character makes it invalid UUID
        let result = JobId::new("550e8400-e29b-41d4-a716\n-446655440000");
        assert!(matches!(result, Err(IdError::InvalidUuid)));
    }

    #[test]
    fn job_id_returns_err_with_null_byte() {
        let result = JobId::new("550e8400-e29b-41d4-a716-44665\x005440000");
        assert!(matches!(result, Err(IdError::InvalidUuid)));
    }

    #[test]
    fn job_id_returns_err_with_slash() {
        // slash is not valid in UUID
        let result = JobId::new("550e8400-e29b-41d4-a716/446655440000");
        assert!(matches!(result, Err(IdError::InvalidUuid)));
    }

    #[test]
    fn job_id_returns_err_with_space() {
        let result = JobId::new("550e8400 e29b-41d4-a716-446655440000");
        assert!(matches!(result, Err(IdError::InvalidUuid)));
    }

    #[test]
    fn job_id_returns_err_with_tab() {
        let result = JobId::new("550e8400-\te29b-41d4-a716-446655440000");
        assert!(matches!(result, Err(IdError::InvalidUuid)));
    }

    #[test]
    fn job_id_returns_err_too_long() {
        // Very long string is not a valid UUID
        let over_max = "a".repeat(1001);
        let result = JobId::new(&over_max);
        assert!(matches!(result, Err(IdError::InvalidUuid)));
    }

    #[test]
    fn job_id_with_cjk_characters_is_rejected() {
        // CJK characters are not valid in UUID
        let result = JobId::new("550e8400-e29b-41d4-a716-日本語-440000");
        assert!(matches!(result, Err(IdError::InvalidUuid)));
    }

    #[test]
    fn job_id_with_only_cjk_characters_is_rejected() {
        let result = JobId::new("日本語");
        assert!(matches!(result, Err(IdError::InvalidUuid)));
    }

    #[test]
    fn job_id_returns_err_with_emoji() {
        let result = JobId::new("550e8400-e29b-41d4-🔥-446655440000");
        assert!(matches!(result, Err(IdError::InvalidUuid)));
    }

    #[test]
    fn job_id_returns_err_with_special_chars() {
        // Special characters that are not valid in UUID
        let invalid_uuids = [
            "550e8400!e29b-41d4-a716-446655440000",
            "550e8400@e29b-41d4-a716-446655440000",
            "550e8400#e29b-41d4-a716-446655440000",
        ];
        for id in invalid_uuids {
            let result = JobId::new(id);
            assert!(
                matches!(result, Err(IdError::InvalidUuid)),
                "expected InvalidUuid for '{}'",
                id
            );
        }
    }

    #[test]
    fn job_id_from_string_ownership() {
        let owned = String::from(TEST_UUID);
        let id = JobId::from(owned);
        assert_eq!(id.as_str(), TEST_UUID);
    }

    #[test]
    fn job_id_from_str_borrowing() {
        let id = JobId::from(TEST_UUID);
        assert_eq!(id.as_str(), TEST_UUID);
    }

    #[test]
    fn job_id_display_trait() {
        let id = JobId::new(TEST_UUID).unwrap();
        let formatted = format!("{}", id);
        assert_eq!(formatted, TEST_UUID);
    }

    #[test]
    fn job_id_as_ref_trait() {
        let id = JobId::new(TEST_UUID).unwrap();
        let s: &str = id.as_ref();
        assert_eq!(s, TEST_UUID);
    }

    #[test]
    fn job_id_deref_trait() {
        let id = JobId::new(TEST_UUID).unwrap();
        let s: &str = &id;
        assert_eq!(s, TEST_UUID);
    }

    #[test]
    fn job_id_from_str_trait() {
        let parsed: JobId = TEST_UUID.parse().unwrap();
        assert_eq!(parsed.as_str(), TEST_UUID);
    }

    #[test]
    fn job_id_from_str_trait_error() {
        let result: Result<JobId, _> = "not-a-uuid".parse();
        assert!(matches!(result, Err(IdError::InvalidUuid)));
    }

    #[test]
    fn job_id_eq_and_hash() {
        let id1 = JobId::new(TEST_UUID).unwrap();
        let id2 = JobId::new(TEST_UUID).unwrap();
        let id3 = JobId::new(TEST_UUID_2).unwrap();

        assert_eq!(id1, id2);
        assert_ne!(id1, id3);

        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(id1.clone());
        set.insert(id2.clone());
        set.insert(id3);
        assert_eq!(set.len(), 2); // id1 and id2 are equal, so only 2 unique
    }

    #[test]
    fn job_id_clone() {
        let id1 = JobId::new(TEST_UUID).unwrap();
        let id2 = id1.clone();
        assert_eq!(id1, id2);
    }

    #[test]
    fn job_id_default() {
        let default_id = JobId::default();
        assert_eq!(default_id.as_str(), "");
    }

    // =======================================================================
    // TaskId validation tests (verifies macro generates correct type)
    // =======================================================================

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

    // =======================================================================
    // NodeId validation tests
    // =======================================================================

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

    // =======================================================================
    // UserId validation tests
    // =======================================================================

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

    // =======================================================================
    // RoleId validation tests
    // =======================================================================

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

    // =======================================================================
    // ScheduledJobId validation tests
    // =======================================================================

    #[allow(clippy::redundant_pattern_matching)]
    #[test]
    fn scheduled_job_id_new_valid() {
        let result = ScheduledJobId::new("scheduled-job-123");
        assert!(matches!(result, Ok(_)));
    }

    // =======================================================================
    // validate_id function tests
    // =======================================================================

    #[test]
    fn validate_id_rejects_empty() {
        assert!(validate_id("").is_err());
    }

    #[test]
    fn validate_id_rejects_only_whitespace() {
        // Whitespace fails the character check since space is not alphanumeric/dash/underscore
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
        // Various control characters
        assert!(validate_id("test\x00value").is_err()); // null
        assert!(validate_id("test\x01value").is_err()); // SOH
        assert!(validate_id("test\x7fvalue").is_err()); // DEL
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

    // =======================================================================
    // TriggerId construction — happy paths (Behaviors 33-36)
    // =======================================================================

    #[test]
    fn trigger_id_new_returns_ok_when_input_is_3_chars() {
        let result = TriggerId::new("abc");
        assert_eq!(result.unwrap().as_str(), "abc");
    }

    #[test]
    fn trigger_id_new_accepts_exactly_64_chars() {
        let max_valid = "a".repeat(64);
        let id = TriggerId::new(&max_valid).unwrap();
        assert_eq!(id.as_str().len(), 64);
    }

    #[test]
    fn trigger_id_new_accepts_dash_and_underscore() {
        let result = TriggerId::new("a_b-c");
        assert_eq!(result.unwrap().as_str(), "a_b-c");
    }

    #[test]
    fn trigger_id_new_accepts_cjk_characters() {
        let result = TriggerId::new("日本語");
        assert_eq!(result.unwrap().as_str(), "日本語");
    }

    // =======================================================================
    // TriggerId validation — error paths (Behaviors 37-45)
    // =======================================================================

    #[test]
    fn trigger_id_new_returns_err_empty_when_input_is_empty() {
        let result = TriggerId::new("");
        assert_eq!(result, Err(IdError::Empty));
    }

    #[test]
    fn trigger_id_new_returns_err_too_short_when_input_is_2_chars() {
        let result = TriggerId::new("ab");
        assert_eq!(result, Err(IdError::TooShort(2)));
    }

    #[test]
    fn trigger_id_new_returns_err_too_short_when_input_is_1_char() {
        let result = TriggerId::new("a");
        assert_eq!(result, Err(IdError::TooShort(1)));
    }

    #[test]
    fn trigger_id_new_returns_err_too_long_when_input_is_65_chars() {
        let long = "a".repeat(65);
        let result = TriggerId::new(&long);
        assert_eq!(result, Err(IdError::TooLong(65)));
    }

    #[test]
    fn trigger_id_new_returns_err_too_long_when_input_is_100_chars() {
        let long = "a".repeat(100);
        let result = TriggerId::new(&long);
        assert_eq!(result, Err(IdError::TooLong(100)));
    }

    #[test]
    fn trigger_id_new_returns_err_invalid_characters_when_input_has_at_sign() {
        let result = TriggerId::new("abc@def");
        assert_eq!(result, Err(IdError::InvalidCharacters));
    }

    #[test]
    fn trigger_id_new_returns_err_invalid_characters_when_input_has_space() {
        let result = TriggerId::new("abc def");
        assert_eq!(result, Err(IdError::InvalidCharacters));
    }

    #[test]
    fn trigger_id_new_returns_err_invalid_characters_when_input_has_emoji() {
        let result = TriggerId::new("abc-\u{1F525}def");
        assert_eq!(result, Err(IdError::InvalidCharacters));
    }

    #[test]
    fn trigger_id_new_returns_err_invalid_characters_when_input_has_null_byte() {
        let result = TriggerId::new("abc\x00def");
        assert_eq!(result, Err(IdError::InvalidCharacters));
    }

    // =======================================================================
    // TriggerId — preservation and whitespace (Behaviors 46-49)
    // =======================================================================

    #[test]
    fn trigger_id_preserves_input_string_exactly() {
        let result = TriggerId::new("my-trigger_01");
        assert_eq!(result.unwrap().to_string(), "my-trigger_01");
    }

    #[test]
    fn trigger_id_new_rejects_leading_whitespace() {
        let result = TriggerId::new(" abc");
        assert_eq!(result, Err(IdError::InvalidCharacters));
    }

    #[test]
    fn trigger_id_new_rejects_trailing_whitespace() {
        let result = TriggerId::new("abc ");
        assert_eq!(result, Err(IdError::InvalidCharacters));
    }

    #[test]
    fn trigger_id_new_preserves_mixed_case() {
        let result = TriggerId::new("MyTrigger_01");
        let id = result.unwrap();
        assert_eq!(id.as_str(), "MyTrigger_01");
        assert!(id.as_str().contains('M'));
    }

    // =======================================================================
    // TriggerId — accessors (Behaviors 50-51)
    // =======================================================================

    #[test]
    fn trigger_id_as_str_returns_original() {
        let id = TriggerId::new("valid-id").unwrap();
        assert_eq!(id.as_str(), "valid-id");
    }

    #[test]
    fn trigger_id_display_returns_original_string() {
        let id = TriggerId::new("my-trigger").unwrap();
        assert_eq!(format!("{id}"), "my-trigger");
    }

    // =======================================================================
    // TriggerId — serde roundtrip valid (Behaviors 52-53)
    // =======================================================================

    #[test]
    fn trigger_id_serializes_as_plain_json_string() {
        let id = TriggerId::new("trigger-abc").unwrap();
        assert_eq!(serde_json::to_string(&id).unwrap(), "\"trigger-abc\"");
    }

    #[test]
    fn trigger_id_deserializes_from_valid_json_string() {
        let id: TriggerId = serde_json::from_str("\"my-trigger\"").unwrap();
        assert_eq!(id.as_str(), "my-trigger");
    }

    // =======================================================================
    // TriggerId — serde rejection (Behaviors 54-57)
    // =======================================================================

    #[test]
    fn trigger_id_deserialize_rejects_2_char_string() {
        let result: Result<TriggerId, _> = serde_json::from_str("\"ab\"");
        let err = result.unwrap_err();
        assert!(err.to_string().contains("too short"));
    }

    #[test]
    fn trigger_id_deserialize_rejects_1_char_string() {
        let result: Result<TriggerId, _> = serde_json::from_str("\"x\"");
        let err = result.unwrap_err();
        assert!(err.to_string().contains("too short"));
    }

    #[test]
    fn trigger_id_deserialize_rejects_empty_string() {
        let result: Result<TriggerId, _> = serde_json::from_str("\"\"");
        let err = result.unwrap_err();
        assert!(err.to_string().contains("empty"));
    }

    #[test]
    fn trigger_id_deserialize_rejects_65_char_string() {
        let json = format!("\"{}\"", "a".repeat(65));
        let result: Result<TriggerId, _> = serde_json::from_str(&json);
        let err = result.unwrap_err();
        assert!(err.to_string().contains("too long"));
    }

    // =======================================================================
    // TriggerId — Default (Behavior 58)
    // =======================================================================

    #[test]
    fn trigger_id_default_returns_empty_string() {
        let id = TriggerId::default();
        assert_eq!(id.as_str(), "");
    }

    // =======================================================================
    // TriggerId — FromStr (Behaviors 59-60)
    // =======================================================================

    #[test]
    fn trigger_id_from_str_parses_valid_string() {
        let id: TriggerId = "valid-id".parse().unwrap();
        assert_eq!(id.as_str(), "valid-id");
    }

    #[test]
    fn trigger_id_from_str_rejects_short_string() {
        let result: Result<TriggerId, _> = "x".parse();
        assert_eq!(result, Err(IdError::TooShort(1)));
    }

    // =======================================================================
    // TriggerId — From<String> and From<&str> bypass validation (Behaviors 61-62)
    // =======================================================================

    #[test]
    fn trigger_id_from_string_bypasses_validation() {
        let id = TriggerId::from(String::from("x"));
        assert_eq!(id.as_str(), "x");
        assert_eq!(id.as_str().len(), 1);
    }

    #[test]
    fn trigger_id_from_str_bypasses_validation() {
        let id = TriggerId::from("y");
        assert_eq!(id.as_str(), "y");
        assert_eq!(id.as_str().len(), 1);
    }

    // =======================================================================
    // TriggerId — trait impls (Behaviors 63-68)
    // =======================================================================

    #[test]
    fn trigger_id_as_ref_returns_inner_string() {
        let id = TriggerId::new("ref-test").unwrap();
        let s: &str = id.as_ref();
        assert_eq!(s, "ref-test");
    }

    #[test]
    fn trigger_id_deref_returns_inner_string() {
        let id = TriggerId::new("deref-test").unwrap();
        let s: &str = &id;
        assert_eq!(s, "deref-test");
    }

    #[test]
    fn trigger_id_borrow_returns_inner_string() {
        let id = TriggerId::new("borrow-test").unwrap();
        let s: &str = id.borrow();
        assert_eq!(s, "borrow-test");
    }

    #[test]
    fn trigger_id_clone_produces_equal_copy() {
        let id = TriggerId::new("clone-test").unwrap();
        let cloned = id.clone();
        assert_eq!(cloned, id);
        assert_eq!(cloned.as_str(), "clone-test");
    }

    #[test]
    fn trigger_id_partial_eq_reflexive() {
        let id = TriggerId::new("eq-test").unwrap();
        assert_eq!(id, id);
    }

    #[test]
    fn trigger_id_eq_and_hash_works_in_hashset() {
        let id1 = TriggerId::new("same").unwrap();
        let id2 = TriggerId::new("same").unwrap();
        let id3 = TriggerId::new("different").unwrap();

        let mut set = std::collections::HashSet::new();
        set.insert(id1);
        set.insert(id2);
        set.insert(id3);
        assert_eq!(set.len(), 2);
        assert!(set.contains(&TriggerId::new("same").unwrap()));
    }

    // =======================================================================
    // IdError Display through TriggerId::new() path (Behaviors 69-72)
    // =======================================================================

    #[test]
    fn trigger_id_new_returns_err_empty_displays_correct_message() {
        let err = TriggerId::new("").unwrap_err();
        assert!(matches!(err, IdError::Empty));
        assert!(format!("{err}").to_lowercase().contains("empty"));
    }

    #[test]
    fn trigger_id_new_returns_err_too_long_displays_correct_message() {
        let err = TriggerId::new("a".repeat(65)).unwrap_err();
        assert!(matches!(err, IdError::TooLong(65)));
        assert!(format!("{err}").contains("65"));
    }

    #[test]
    fn trigger_id_new_returns_err_too_short_displays_correct_message() {
        let err = TriggerId::new("ab").unwrap_err();
        assert!(matches!(err, IdError::TooShort(2)));
        let msg = format!("{err}");
        assert!(msg.to_lowercase().contains("too short"));
        assert!(msg.contains('2'));
    }

    #[test]
    fn trigger_id_new_returns_err_invalid_chars_displays_correct_message() {
        let err = TriggerId::new("bad@id").unwrap_err();
        assert!(matches!(err, IdError::InvalidCharacters));
        assert!(format!("{err}").to_lowercase().contains("invalid"));
    }

    // =======================================================================
    // TriggerId — Proptest invariants
    // =======================================================================

    proptest::proptest! {
        /// TriggerId::new() rejects lengths outside 3..=64 and accepts valid ones.
        #[test]
        fn proptest_trigger_id_rejects_lengths_outside_3_to_64(
            len in 0usize..=70
        ) {
            let s = "a".repeat(len);
            let result = TriggerId::new(&s);
            if len < 3 {
                prop_assert!(result.is_err());
            } else if len > 64 {
                prop_assert!(matches!(result, Err(IdError::TooLong(n)) if n == len));
            } else {
                prop_assert!(result.is_ok());
                let id = result.unwrap();
                prop_assert_eq!(id.as_str(), s);
            }
        }

        /// TriggerId::new() rejects invalid characters in strings of valid length.
        #[test]
        fn proptest_trigger_id_rejects_invalid_chars(
            base_len in 3usize..=64,
            special_char in proptest::sample::select(vec![
                '\t', '@', '#', '$', '%', '^', '&', '*', '(', ')', ' ', '=', '+',
                '[', ']', '{', '}', '|', '\\', ':', ';', '\'', '"', '<', '>', ',',
                '.', '/', '?', '`', '~',
            ])
        ) {
            let safe_part = "a".repeat(base_len.saturating_sub(1).max(1));
            let s = format!("{safe_part}{special_char}");
            // Ensure total length is in valid range so only char check fires
            if s.len() >= 3 && s.len() <= 64 {
                let result = TriggerId::new(&s);
                prop_assert!(matches!(result, Err(IdError::InvalidCharacters)));
            }
        }

        /// TriggerId serde roundtrip: valid IDs survive serialize then deserialize.
        #[test]
        fn proptest_trigger_id_serde_roundtrip_preserves_string(
            s in "[a-zA-Z0-9_-]{3,64}"
        ) {
            let id = TriggerId::new(&s).unwrap();
            let json = serde_json::to_string(&id).unwrap();
            let recovered: TriggerId = serde_json::from_str(&json).unwrap();
            prop_assert_eq!(recovered.as_str(), s);
        }

        /// TriggerId input preservation: as_str() returns byte-for-byte identical input.
        #[test]
        fn proptest_trigger_id_preserves_input_without_mutation(
            s in "[a-zA-Z0-9_-]{3,64}"
        ) {
            let id = TriggerId::new(&s).unwrap();
            prop_assert_eq!(id.as_str(), s);
        }
    }
}
