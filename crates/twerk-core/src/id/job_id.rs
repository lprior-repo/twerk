use serde::{Deserialize, Serialize};
use std::borrow::Borrow;
use std::fmt;
use std::ops::Deref;
use std::str::FromStr;
use utoipa::openapi::schema::{ObjectBuilder, Schema, Type};

use super::common::{IdError, MAX_ID_LENGTH};

const RFC4122_UUID_LENGTH: usize = 36;
const SHORT_JOB_ID_LENGTH: usize = 22;
const SHORT_JOB_ID_ALPHABET: &str = "23456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
const JOB_ID_PATTERN: &str = "^(?:[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}|[23456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz]{22})$";

fn is_valid_rfc4122_uuid(id: &str) -> bool {
    id.len() == RFC4122_UUID_LENGTH && uuid::Uuid::parse_str(id).is_ok()
}

fn is_valid_short_job_id(id: &str) -> bool {
    id.len() == SHORT_JOB_ID_LENGTH
        && id
            .bytes()
            .all(|byte| SHORT_JOB_ID_ALPHABET.as_bytes().contains(&byte))
}

fn validate_job_id(id: &str) -> Result<(), IdError> {
    if id.is_empty() {
        return Err(IdError::Empty);
    }
    if id.len() > MAX_ID_LENGTH {
        return Err(IdError::TooLong(id.len()));
    }
    if is_valid_rfc4122_uuid(id) || is_valid_short_job_id(id) {
        Ok(())
    } else {
        Err(IdError::InvalidJobIdFormat)
    }
}

/// Validated identifier for a job.
///
/// Jobs may be identified by either RFC 4122 UUIDs or the shorter base57 IDs
/// emitted by the coordinator at runtime.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Default)]
#[serde(transparent)]
pub struct JobId(String);

impl JobId {
    pub fn new(id: impl Into<String>) -> Result<Self, IdError> {
        let value = id.into();
        validate_job_id(&value)?;
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for JobId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

impl utoipa::PartialSchema for JobId {
    fn schema() -> utoipa::openapi::RefOr<utoipa::openapi::schema::Schema> {
        utoipa::openapi::RefOr::T(Schema::Object(
            ObjectBuilder::new()
                .schema_type(Type::String)
                .description(Some(
                    "RFC 4122 UUID or 22-character base57 short job ID emitted by the coordinator",
                ))
                .pattern(Some(JOB_ID_PATTERN))
                .build(),
        ))
    }
}

impl utoipa::ToSchema for JobId {
    fn name() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed("JobId")
    }
}

impl fmt::Display for JobId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
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

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

impl From<JobId> for String {
    fn from(value: JobId) -> Self {
        value.0
    }
}
