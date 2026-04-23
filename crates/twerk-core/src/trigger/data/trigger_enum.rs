//! Root enum for all trigger data types.

use serde::{Deserialize, Serialize};

use super::cron_trigger::CronTrigger;
use super::polling_trigger::PollingTrigger;
use super::webhook_trigger::WebhookTrigger;

// =============================================================================
// Trigger
// =============================================================================

/// Root enum for trigger types with polymorphic deserialization support.
///
/// The `#[serde(tag = "type")]` attribute ensures JSON has `"type": "Cron"|"Webhook"|"Polling"`
/// discriminant field.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Trigger {
    Cron(CronTrigger),
    Webhook(WebhookTrigger),
    Polling(PollingTrigger),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trigger_cron_roundtrip_serialization() {
        let cron = crate::trigger::data::CronTrigger::new(
            "trigger-001",
            Some("Daily Job".to_string()),
            None,
            "0 0 * * * *",
            "UTC",
            false,
            None,
        )
        .unwrap();
        let trigger = Trigger::Cron(cron);

        let json = serde_json::to_string(&trigger).unwrap();
        assert!(json.contains("\"type\":\"cron\""));

        let recovered: Trigger = serde_json::from_str(&json).unwrap();
        assert!(matches!(recovered, Trigger::Cron(_)));
    }

    #[test]
    fn trigger_webhook_roundtrip_serialization() {
        let webhook = crate::trigger::data::WebhookTrigger::new(
            "webhook-001",
            None,
            None,
            "https://example.com/hook",
            crate::trigger::data::HttpMethod::Post,
            crate::trigger::data::WebhookAuth::None,
            None,
            None,
            None,
            false,
        )
        .unwrap();
        let trigger = Trigger::Webhook(webhook);

        let json = serde_json::to_string(&trigger).unwrap();
        assert!(json.contains("\"type\":\"webhook\""));

        let recovered: Trigger = serde_json::from_str(&json).unwrap();
        assert!(matches!(recovered, Trigger::Webhook(_)));
    }

    #[test]
    fn trigger_polling_roundtrip_serialization() {
        let polling = crate::trigger::data::PollingTrigger::new(
            "polling-001",
            None,
            None,
            "https://api.example.com/data",
            crate::trigger::data::HttpMethod::Get,
            crate::trigger::data::WebhookAuth::None,
            None,
            "30s",
            None,
            None,
            None,
            false,
        )
        .unwrap();
        let trigger = Trigger::Polling(polling);

        let json = serde_json::to_string(&trigger).unwrap();
        assert!(json.contains("\"type\":\"polling\""));

        let recovered: Trigger = serde_json::from_str(&json).unwrap();
        assert!(matches!(recovered, Trigger::Polling(_)));
    }
}
