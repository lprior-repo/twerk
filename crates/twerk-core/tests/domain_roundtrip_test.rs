//! Integration tests for domain type serde roundtrips.

use twerk_core::domain::{CronExpression, Hostname, WebhookUrl};

// ---------------------------------------------------------------------------
// WebhookUrl Serde Roundtrips
// ---------------------------------------------------------------------------

#[test]
fn webhook_url_json_roundtrip_preserves_value() {
    let url = WebhookUrl::new("https://example.com/path").unwrap();
    let json = serde_json::to_string(&url).unwrap();
    assert_eq!(json, "\"https://example.com/path\"");
    let decoded: WebhookUrl = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.as_str(), url.as_str());
}

#[test]
fn webhook_url_yaml_roundtrip_preserves_value() {
    let url = WebhookUrl::new("https://example.com/path").unwrap();
    let yaml = serde_yaml::to_string(&url).unwrap();
    let decoded: WebhookUrl = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(decoded.as_str(), url.as_str());
}

// ---------------------------------------------------------------------------
// Hostname Serde Roundtrips
// ---------------------------------------------------------------------------

#[test]
fn hostname_json_roundtrip_preserves_value() {
    let host = Hostname::new("api.example.com").unwrap();
    let json = serde_json::to_string(&host).unwrap();
    assert_eq!(json, "\"api.example.com\"");
    let decoded: Hostname = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.as_str(), host.as_str());
}

#[test]
fn hostname_yaml_roundtrip_preserves_value() {
    let host = Hostname::new("api.example.com").unwrap();
    let yaml = serde_yaml::to_string(&host).unwrap();
    let decoded: Hostname = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(decoded.as_str(), host.as_str());
}

// ---------------------------------------------------------------------------
// CronExpression Serde Roundtrips
// ---------------------------------------------------------------------------

#[test]
fn cron_expression_json_roundtrip_preserves_value() {
    let expr = CronExpression::new("0 0 * * MON").unwrap();
    let json = serde_json::to_string(&expr).unwrap();
    assert_eq!(json, "\"0 0 * * MON\"");
    let decoded: CronExpression = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.as_str(), expr.as_str());
}

#[test]
fn cron_expression_yaml_roundtrip_preserves_value() {
    let expr = CronExpression::new("0 0 * * MON").unwrap();
    let yaml = serde_yaml::to_string(&expr).unwrap();
    let decoded: CronExpression = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(decoded.as_str(), expr.as_str());
}

// ---------------------------------------------------------------------------
// Additional Serde Invariants
// ---------------------------------------------------------------------------

#[test]
fn all_domain_types_serialize_transparently() {
    // WebhookUrl should serialize to just the URL string
    let url = WebhookUrl::new("https://example.com/hook").unwrap();
    let json = serde_json::to_string(&url).unwrap();
    assert!(json.starts_with('"'));
    assert!(json.ends_with('"'));
    assert!(!json.contains("WebhookUrl"));

    // Hostname should serialize to just the hostname string
    let host = Hostname::new("example.com").unwrap();
    let json = serde_json::to_string(&host).unwrap();
    assert!(json.starts_with('"'));
    assert!(json.ends_with('"'));
    assert!(!json.contains("Hostname"));

    // CronExpression should serialize to just the expression string
    let expr = CronExpression::new("0 0 * * *").unwrap();
    let json = serde_json::to_string(&expr).unwrap();
    assert!(json.starts_with('"'));
    assert!(json.ends_with('"'));
    assert!(!json.contains("CronExpression"));
}

#[test]
fn all_domain_types_implement_display() {
    use std::fmt::Write;

    let url = WebhookUrl::new("https://example.com/hook").unwrap();
    let mut url_str = String::new();
    write!(url_str, "{}", url).unwrap();
    assert_eq!(url_str, "https://example.com/hook");

    let host = Hostname::new("example.com").unwrap();
    let mut host_str = String::new();
    write!(host_str, "{}", host).unwrap();
    assert_eq!(host_str, "example.com");

    let expr = CronExpression::new("0 0 * * *").unwrap();
    let mut expr_str = String::new();
    write!(expr_str, "{}", expr).unwrap();
    assert_eq!(expr_str, "0 0 * * *");
}
