//! Cron-based trigger configuration.

use serde::de;
use serde::{Deserialize, Deserializer, Serialize};

use crate::domain::CronExpression;
use crate::id::TriggerId;

use super::error::TriggerDataError;
use super::validation::{normalize_timezone, validate_timezone};

// =============================================================================
// CronTrigger
// =============================================================================

/// Cron-based trigger configuration.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CronTrigger {
    pub id: TriggerId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub cron: CronExpression,
    pub timezone: String,
    #[serde(default)]
    pub disabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<serde_json::Value>,
}

/// Intermediate struct for CronTrigger deserialization with validation.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CronTriggerRaw {
    id: TriggerId,
    name: Option<String>,
    description: Option<String>,
    cron: CronExpression,
    timezone: String,
    #[serde(default)]
    disabled: bool,
    payload: Option<serde_json::Value>,
}

impl From<CronTriggerRaw> for CronTrigger {
    fn from(raw: CronTriggerRaw) -> Self {
        CronTrigger {
            id: raw.id,
            name: raw.name,
            description: raw.description,
            cron: raw.cron,
            timezone: normalize_timezone(&raw.timezone),
            disabled: raw.disabled,
            payload: raw.payload,
        }
    }
}

impl<'de> Deserialize<'de> for CronTrigger {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = CronTriggerRaw::deserialize(deserializer)?;
        validate_timezone(&raw.timezone).map_err(de::Error::custom)?;
        Ok(CronTrigger::from(raw))
    }
}

impl CronTrigger {
    /// Constructs a new `CronTrigger`.
    ///
    /// # Parameters
    /// - `id: impl Into<String>` - Trigger ID (3-64 chars, alphanumeric/-/_)
    /// - `name: Option<String>` - Optional human-readable name
    /// - `description: Option<String>` - Optional description
    /// - `cron: impl Into<String>` - Cron expression (e.g., "0 0 * * * *")
    /// - `timezone: impl Into<String>` - IANA timezone string (e.g., "UTC", "Z", "America/New_York")
    /// - `disabled: bool` - Whether trigger is disabled (default: false)
    /// - `payload: Option<serde_json::Value>` - Optional JSON payload
    ///
    /// # Returns
    /// - `Ok(CronTrigger { ... })` when all validations pass
    /// - `Err(TriggerDataError::InvalidTriggerId)` when `id` is invalid
    /// - `Err(TriggerDataError::InvalidCronExpression)` when `cron` is invalid
    /// - `Err(TriggerDataError::InvalidTimezone)` when `timezone` is not a valid IANA timezone
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: impl Into<String>,
        name: Option<String>,
        description: Option<String>,
        cron: impl Into<String>,
        timezone: impl Into<String>,
        disabled: bool,
        payload: Option<serde_json::Value>,
    ) -> Result<CronTrigger, TriggerDataError> {
        let id = TriggerId::new(id)?;
        let cron = CronExpression::new(cron)?;
        let timezone = normalize_timezone(&timezone.into());

        validate_timezone(&timezone)?;

        Ok(CronTrigger {
            id,
            name,
            description,
            cron,
            timezone,
            disabled,
            payload,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crons_trigger_new_returns_invalid_trigger_id_when_id_is_too_short() {
        let result = CronTrigger::new("ab", None, None, "0 0 * * * *", "UTC", false, None);
        assert!(matches!(result, Err(TriggerDataError::InvalidTriggerId(_))));
    }

    #[test]
    fn crons_trigger_new_returns_invalid_trigger_id_when_id_is_too_long() {
        let long_id = "a".repeat(65);
        let result = CronTrigger::new(long_id, None, None, "0 0 * * * *", "UTC", false, None);
        assert!(matches!(result, Err(TriggerDataError::InvalidTriggerId(_))));
    }

    #[test]
    fn crons_trigger_new_returns_invalid_cron_expression_when_cron_is_invalid() {
        let result = CronTrigger::new("trigger-001", None, None, "not-a-cron", "UTC", false, None);
        assert!(matches!(
            result,
            Err(TriggerDataError::InvalidCronExpression(_))
        ));
    }
}
