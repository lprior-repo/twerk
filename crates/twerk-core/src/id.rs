use serde::{Deserialize, Serialize};
use std::borrow::Borrow;
use std::fmt;
use std::ops::Deref;
use std::str::FromStr;
use thiserror::Error;

const MAX_ID_LENGTH: usize = 1000;

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum IdError {
    #[error("ID cannot be empty")]
    Empty,
    #[error("ID exceeds maximum length of {MAX_ID_LENGTH} characters: {0} characters")]
    TooLong(usize),
    #[error("ID contains invalid characters: only alphanumeric, dash, and underscore allowed")]
    InvalidCharacters,
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
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
        #[serde(transparent)]
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

define_id!(JobId);
define_id!(TaskId);
define_id!(NodeId);
define_id!(ScheduledJobId);
define_id!(UserId);
define_id!(RoleId);

#[cfg(test)]
mod tests {
    use super::*;

    // =======================================================================
    // JobId validation tests
    // =======================================================================

    #[allow(clippy::redundant_pattern_matching)]
    #[test]
    fn job_id_returns_ok_when_input_is_valid_alphanumeric() {
        let result = JobId::new("job-123");
        assert!(matches!(result, Ok(_)));
        assert_eq!(result.unwrap().as_str(), "job-123");
    }

    #[allow(clippy::redundant_pattern_matching)]
    #[test]
    fn job_id_returns_ok_with_underscore() {
        let result = JobId::new("job_456");
        assert!(matches!(result, Ok(_)));
        assert_eq!(result.unwrap().as_str(), "job_456");
    }

    #[test]
    fn job_id_returns_err_empty_string() {
        let result = JobId::new("");
        assert!(matches!(result, Err(IdError::Empty)));
    }

    #[test]
    fn job_id_returns_err_with_newline() {
        let result = JobId::new("job\n123");
        assert!(matches!(result, Err(IdError::InvalidCharacters)));
    }

    #[test]
    fn job_id_returns_err_with_null_byte() {
        let result = JobId::new("job\x00123");
        assert!(matches!(result, Err(IdError::InvalidCharacters)));
    }

    #[test]
    fn job_id_returns_err_with_slash() {
        let result = JobId::new("job/123");
        assert!(matches!(result, Err(IdError::InvalidCharacters)));
    }

    #[test]
    fn job_id_returns_err_with_space() {
        let result = JobId::new("job 123");
        assert!(matches!(result, Err(IdError::InvalidCharacters)));
    }

    #[test]
    fn job_id_returns_err_with_tab() {
        let result = JobId::new("job\t123");
        assert!(matches!(result, Err(IdError::InvalidCharacters)));
    }

    #[test]
    fn job_id_returns_ok_at_max_length() {
        let max_id = "a".repeat(1000);
        let result = JobId::new(&max_id);
        assert!(result.is_ok(), "should accept 1000 character ID");
        assert_eq!(result.unwrap().as_str().len(), 1000);
    }

    #[test]
    fn job_id_returns_err_when_exceeds_max_length() {
        let over_max = "a".repeat(1001);
        let result = JobId::new(&over_max);
        assert!(matches!(result, Err(IdError::TooLong(1001))));
    }

    #[test]
    fn job_id_with_cjk_characters_is_accepted() {
        // Rust's is_alphanumeric() returns true for CJK characters (Unicode Lo category)
        // So "job-日本語-123" is actually valid according to the current implementation
        let result = JobId::new("job-日本語-123");
        assert!(
            result.is_ok(),
            "CJK characters are alphanumeric in Rust: {:?}",
            result
        );
    }

    #[test]
    fn job_id_with_only_cjk_characters_is_accepted() {
        // Pure CJK ID is valid in Rust's classification
        let result = JobId::new("日本語");
        assert!(result.is_ok(), "pure CJK should be accepted: {:?}", result);
    }

    #[test]
    fn job_id_returns_err_with_emoji() {
        let result = JobId::new("job-🔥-123");
        assert!(matches!(result, Err(IdError::InvalidCharacters)));
    }

    #[test]
    fn job_id_returns_err_with_special_chars() {
        let special = [
            "!", "@", "#", "$", "%", "^", "&", "*", "(", ")", "=", "+", "[", "]", "{", "}", "|",
            "\\", ";", ":", "'", "\"", ",", ".", "<", ">", "/", "?", "`", "~",
        ];
        for c in special {
            let id = format!("job{c}123");
            let result = JobId::new(&id);
            assert!(
                matches!(result, Err(IdError::InvalidCharacters)),
                "expected InvalidCharacters for '{}'",
                id
            );
        }
    }

    #[test]
    fn job_id_from_string_ownership() {
        let owned = String::from("test-job-456");
        let id = JobId::from(owned);
        assert_eq!(id.as_str(), "test-job-456");
    }

    #[test]
    fn job_id_from_str_borrowing() {
        let id = JobId::from("borrowed-job-789");
        assert_eq!(id.as_str(), "borrowed-job-789");
    }

    #[test]
    fn job_id_display_trait() {
        let id = JobId::new("display-test").unwrap();
        let formatted = format!("{}", id);
        assert_eq!(formatted, "display-test");
    }

    #[test]
    fn job_id_as_ref_trait() {
        let id = JobId::new("ref-test").unwrap();
        let s: &str = id.as_ref();
        assert_eq!(s, "ref-test");
    }

    #[test]
    fn job_id_deref_trait() {
        let id = JobId::new("deref-test").unwrap();
        let s: &str = &id;
        assert_eq!(s, "deref-test");
    }

    #[test]
    fn job_id_from_str_trait() {
        let parsed: JobId = "parsed-job".parse().unwrap();
        assert_eq!(parsed.as_str(), "parsed-job");
    }

    #[test]
    fn job_id_from_str_trait_error() {
        let result: Result<JobId, _> = "".parse();
        assert!(matches!(result, Err(IdError::Empty)));
    }

    #[test]
    fn job_id_eq_and_hash() {
        let id1 = JobId::new("equal-job").unwrap();
        let id2 = JobId::new("equal-job").unwrap();
        let id3 = JobId::new("different-job").unwrap();

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
        let id1 = JobId::new("cloned-job").unwrap();
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
}
