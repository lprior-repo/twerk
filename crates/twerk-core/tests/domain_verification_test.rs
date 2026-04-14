//! Verification tests for domain types validation rules
//!
//! These tests verify the contract implementation for:
//! - WebhookUrl: RFC 3986 compliant URL validation
//! - Hostname: RFC 1123 compliant hostname validation  
//! - CronExpression: 5-field and 6-field cron validation

use twerk_core::domain::{
    CronExpression, CronExpressionError, Hostname, HostnameError, WebhookUrl, WebhookUrlError,
};

// ============================================================================
// WebhookUrl Tests
// ============================================================================

#[test]
fn webhook_url_valid_https_urls_1() {
    let url = "https://example.com";
    let result = WebhookUrl::new(url);
    assert!(
        result.is_ok(),
        "Expected '{}' to be valid, got: {:?}",
        url,
        result.err()
    );
    assert_eq!(result.unwrap().as_str(), url);
}

#[test]
fn webhook_url_valid_https_urls_2() {
    let url = "https://example.com/path";
    let result = WebhookUrl::new(url);
    assert!(
        result.is_ok(),
        "Expected '{}' to be valid, got: {:?}",
        url,
        result.err()
    );
    assert_eq!(result.unwrap().as_str(), url);
}

#[test]
fn webhook_url_valid_https_urls_3() {
    let url = "https://example.com/path?query=value";
    let result = WebhookUrl::new(url);
    assert!(
        result.is_ok(),
        "Expected '{}' to be valid, got: {:?}",
        url,
        result.err()
    );
    assert_eq!(result.unwrap().as_str(), url);
}

#[test]
fn webhook_url_valid_https_urls_4() {
    let url = "https://localhost";
    let result = WebhookUrl::new(url);
    assert!(
        result.is_ok(),
        "Expected '{}' to be valid, got: {:?}",
        url,
        result.err()
    );
    assert_eq!(result.unwrap().as_str(), url);
}

#[test]
fn webhook_url_valid_https_urls_5() {
    let url = "https://localhost:8080";
    let result = WebhookUrl::new(url);
    assert!(
        result.is_ok(),
        "Expected '{}' to be valid, got: {:?}",
        url,
        result.err()
    );
    assert_eq!(result.unwrap().as_str(), url);
}

#[test]
fn webhook_url_valid_https_urls_6() {
    let url = "http://example.com";
    let result = WebhookUrl::new(url);
    assert!(
        result.is_ok(),
        "Expected '{}' to be valid, got: {:?}",
        url,
        result.err()
    );
    assert_eq!(result.unwrap().as_str(), url);
}

#[test]
fn webhook_url_invalid_scheme_rejected() {
    // file:// scheme
    let result = WebhookUrl::new("file:///path");
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, WebhookUrlError::InvalidScheme(s) if s == "file"));

    // ws:// scheme
    let result = WebhookUrl::new("ws://example.com");
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, WebhookUrlError::InvalidScheme(s) if s == "ws"));

    // ftp:// scheme
    let result = WebhookUrl::new("ftp://example.com");
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, WebhookUrlError::InvalidScheme(s) if s == "ftp"));
}

#[test]
fn webhook_url_empty_string_rejected() {
    let result = WebhookUrl::new("");
    assert!(result.is_err());
    // Empty string fails URL parsing, so we get UrlParseError
    assert!(matches!(
        result.unwrap_err(),
        WebhookUrlError::UrlParseError(_)
    ));
}

#[test]
fn webhook_url_empty_host_rejected() {
    // URL with empty host - the url crate rejects this with UrlParseError("empty host")
    let result = WebhookUrl::new("http://");
    assert!(result.is_err());
    // The url crate is strict and rejects empty hosts at parse time
    assert!(matches!(
        result.unwrap_err(),
        WebhookUrlError::UrlParseError(msg) if msg.contains("empty host")
    ));
}

#[test]
fn webhook_url_json_roundtrip() {
    let url = WebhookUrl::new("https://example.com/path?query=value").unwrap();
    let json = serde_json::to_string(&url).unwrap();
    assert_eq!(json, "\"https://example.com/path?query=value\"");
    let decoded: WebhookUrl = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.as_str(), url.as_str());
}

#[test]
fn webhook_url_fromstr_trait() {
    let url: Result<WebhookUrl, _> = "https://example.com".parse();
    assert!(url.is_ok());
    assert_eq!(url.unwrap().as_str(), "https://example.com");
}

#[test]
fn webhook_url_minimal_valid_url_accepted() {
    // Minimal valid URL with just scheme and single-char host
    let result = WebhookUrl::new("http://a.b");
    assert!(
        result.is_ok(),
        "Expected minimal URL to be valid, got: {:?}",
        result.err()
    );
    assert_eq!(result.unwrap().as_str(), "http://a.b");
}

#[test]
fn webhook_url_https_minimal_accepted() {
    // Minimal https URL
    let result = WebhookUrl::new("https://x.y");
    assert!(
        result.is_ok(),
        "Expected minimal https URL to be valid, got: {:?}",
        result.err()
    );
    assert_eq!(result.unwrap().as_str(), "https://x.y");
}

#[test]
fn webhook_url_http_with_path_accepted() {
    // Minimal URL with path
    let result = WebhookUrl::new("http://localhost/");
    assert!(
        result.is_ok(),
        "Expected URL with root path to be valid, got: {:?}",
        result.err()
    );
    assert_eq!(result.unwrap().as_str(), "http://localhost/");
}

#[test]
fn webhook_url_localhost_accepted() {
    // Localhost variations
    let result = WebhookUrl::new("http://localhost");
    assert!(
        result.is_ok(),
        "Expected localhost URL to be valid, got: {:?}",
        result.err()
    );
}

// ============================================================================
// Hostname Tests
// ============================================================================

#[test]
fn hostname_valid_hostnames_1() {
    let hostname = "localhost";
    let result = Hostname::new(hostname);
    assert!(
        result.is_ok(),
        "Expected '{}' to be valid, got: {:?}",
        hostname,
        result.err()
    );
    assert_eq!(result.unwrap().as_str(), hostname);
}

#[test]
fn hostname_valid_hostnames_2() {
    let hostname = "example.com";
    let result = Hostname::new(hostname);
    assert!(
        result.is_ok(),
        "Expected '{}' to be valid, got: {:?}",
        hostname,
        result.err()
    );
    assert_eq!(result.unwrap().as_str(), hostname);
}

#[test]
fn hostname_valid_hostnames_3() {
    let hostname = "api.example.com";
    let result = Hostname::new(hostname);
    assert!(
        result.is_ok(),
        "Expected '{}' to be valid, got: {:?}",
        hostname,
        result.err()
    );
    assert_eq!(result.unwrap().as_str(), hostname);
}

#[test]
fn hostname_valid_hostnames_4() {
    let hostname = "my-host.example.com";
    let result = Hostname::new(hostname);
    assert!(
        result.is_ok(),
        "Expected '{}' to be valid, got: {:?}",
        hostname,
        result.err()
    );
    assert_eq!(result.unwrap().as_str(), hostname);
}

#[test]
fn hostname_valid_hostnames_5() {
    let hostname = "a.b.c.d";
    let result = Hostname::new(hostname);
    assert!(
        result.is_ok(),
        "Expected '{}' to be valid, got: {:?}",
        hostname,
        result.err()
    );
    assert_eq!(result.unwrap().as_str(), hostname);
}

#[test]
fn hostname_valid_hostnames_6() {
    let hostname = "example-domain.com";
    let result = Hostname::new(hostname);
    assert!(
        result.is_ok(),
        "Expected '{}' to be valid, got: {:?}",
        hostname,
        result.err()
    );
    assert_eq!(result.unwrap().as_str(), hostname);
}

#[test]
fn hostname_port_rejected() {
    // Hostname with port should be rejected with InvalidCharacter(':')
    let result = Hostname::new("example.com:8080");
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, HostnameError::InvalidCharacter(':')));

    let result = Hostname::new("localhost:3000");
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, HostnameError::InvalidCharacter(':')));
}

#[test]
fn hostname_too_long_rejected() {
    // 254 characters should fail (> 253)
    let long_hostname = "a".repeat(254);
    let result = Hostname::new(&long_hostname);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, HostnameError::TooLong(len) if len == 254));
}

#[test]
fn hostname_empty_rejected() {
    let result = Hostname::new("");
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), HostnameError::Empty));
}

#[test]
fn hostname_all_numeric_rejected() {
    // All-numeric labels should be rejected (avoid IP ambiguity)
    // Labels must start and end with alphanumeric
    let result = Hostname::new("123.456.789.0");
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        matches!(err, HostnameError::InvalidLabel(label, reason) if label == "123" && reason == "all_numeric")
    );
}

#[test]
fn hostname_json_roundtrip() {
    let host = Hostname::new("api.example.com").unwrap();
    let json = serde_json::to_string(&host).unwrap();
    assert_eq!(json, "\"api.example.com\"");
    let decoded: Hostname = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.as_str(), host.as_str());
}

#[test]
fn hostname_fromstr_trait() {
    let host: Result<Hostname, _> = "example.com".parse();
    assert!(host.is_ok());
    assert_eq!(host.unwrap().as_str(), "example.com");
}

// ============================================================================
// Hostname Boundary Tests (branch coverage)
// ============================================================================

#[test]
fn hostname_max_length_253_accepted() {
    // Exactly 253 characters should be valid (max boundary)
    // Structure: label1.label2.label3.label4.label5.com
    // 5 labels of 49 chars each + 5 dots + "com" (3) = 5*49 + 5 + 3 = 253
    let hostname = format!(
        "{}.{}.{}.{}.{}.com",
        "a".repeat(49),
        "a".repeat(49),
        "a".repeat(49),
        "a".repeat(49),
        "a".repeat(49)
    );
    assert_eq!(hostname.len(), 253);
    let result = Hostname::new(&hostname);
    assert!(
        result.is_ok(),
        "Expected 253-char hostname to be valid, got: {:?}",
        result.err()
    );
    assert_eq!(result.unwrap().as_str(), hostname);
}

#[test]
fn hostname_min_length_single_char_accepted() {
    // Single character hostname is valid (min boundary)
    let result = Hostname::new("a");
    assert!(
        result.is_ok(),
        "Expected single-char hostname to be valid, got: {:?}",
        result.err()
    );
    assert_eq!(result.unwrap().as_str(), "a");
}

#[test]
fn hostname_label_max_length_63_accepted() {
    // Exactly 63-char label should be valid (max label boundary)
    let label = "a".repeat(63);
    let hostname = format!("{}.com", label);
    assert_eq!(label.len(), 63);
    let result = Hostname::new(&hostname);
    assert!(
        result.is_ok(),
        "Expected 63-char label to be valid, got: {:?}",
        result.err()
    );
}

#[test]
fn hostname_label_exceeds_64_rejected() {
    // Exactly 64-char label should fail (one above max label boundary)
    let label = "a".repeat(64);
    let hostname = format!("{}.com", label);
    assert_eq!(label.len(), 64);
    let result = Hostname::new(&hostname);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, HostnameError::LabelTooLong(l, 64) if l.len() == 64));
}

#[test]
fn hostname_invalid_character_at_start_rejected() {
    // Label starting with hyphen should be rejected
    let result = Hostname::new("-example.com");
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        matches!(err, HostnameError::InvalidLabel(label, reason) if label == "-example" && reason == "must start with alphanumeric")
    );
}

#[test]
fn hostname_invalid_character_at_end_rejected() {
    // Label ending with hyphen should be rejected
    let result = Hostname::new("example-.com");
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        matches!(err, HostnameError::InvalidLabel(label, reason) if label == "example-" && reason == "must end with alphanumeric")
    );
}

// ============================================================================
// CronExpression Tests
// ============================================================================

#[test]
fn cron_expression_valid_5_field_1() {
    let cron = "0 0 * * MON";
    let result = CronExpression::new(cron);
    assert!(
        result.is_ok(),
        "Expected '{}' to be valid, got: {:?}",
        cron,
        result.err()
    );
    assert_eq!(result.unwrap().as_str(), cron);
}

#[test]
fn cron_expression_valid_5_field_2() {
    let cron = "0 0 * * *";
    let result = CronExpression::new(cron);
    assert!(
        result.is_ok(),
        "Expected '{}' to be valid, got: {:?}",
        cron,
        result.err()
    );
    assert_eq!(result.unwrap().as_str(), cron);
}

#[test]
fn cron_expression_valid_5_field_3() {
    let cron = "*/5 * * * *";
    let result = CronExpression::new(cron);
    assert!(
        result.is_ok(),
        "Expected '{}' to be valid, got: {:?}",
        cron,
        result.err()
    );
    assert_eq!(result.unwrap().as_str(), cron);
}

#[test]
fn cron_expression_valid_5_field_4() {
    let cron = "0 30 8 1 1 *";
    let result = CronExpression::new(cron);
    assert!(
        result.is_ok(),
        "Expected '{}' to be valid, got: {:?}",
        cron,
        result.err()
    );
    assert_eq!(result.unwrap().as_str(), cron);
}

#[test]
fn cron_expression_valid_6_field_1() {
    let cron = "0 0 0 1 1 *";
    let result = CronExpression::new(cron);
    assert!(
        result.is_ok(),
        "Expected '{}' to be valid, got: {:?}",
        cron,
        result.err()
    );
    assert_eq!(result.unwrap().as_str(), cron);
}

#[test]
fn cron_expression_valid_6_field_2() {
    let cron = "0 30 8 15 * *";
    let result = CronExpression::new(cron);
    assert!(
        result.is_ok(),
        "Expected '{}' to be valid, got: {:?}",
        cron,
        result.err()
    );
    assert_eq!(result.unwrap().as_str(), cron);
}

#[test]
fn cron_expression_valid_6_field_3() {
    let cron = "0 0 12 * * 1-5";
    let result = CronExpression::new(cron);
    assert!(
        result.is_ok(),
        "Expected '{}' to be valid, got: {:?}",
        cron,
        result.err()
    );
    assert_eq!(result.unwrap().as_str(), cron);
}

#[test]
fn cron_expression_invalid_rejected_1() {
    let cron = "not a cron";
    let result = CronExpression::new(cron);
    assert!(
        result.is_err(),
        "Expected '{}' to be invalid, but it was valid",
        cron
    );
}

#[test]
fn cron_expression_invalid_rejected_2() {
    let cron = "* * *"; // 3 fields
    let result = CronExpression::new(cron);
    assert!(
        result.is_err(),
        "Expected '{}' to be invalid, but it was valid",
        cron
    );
}

#[test]
fn cron_expression_invalid_rejected_3() {
    let cron = "* * * * * * *"; // 7 fields
    let result = CronExpression::new(cron);
    assert!(
        result.is_err(),
        "Expected '{}' to be invalid, but it was valid",
        cron
    );
}

#[test]
fn cron_expression_empty_rejected() {
    let result = CronExpression::new("");
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), CronExpressionError::Empty));
}

#[test]
fn cron_expression_json_roundtrip() {
    let expr = CronExpression::new("0 0 * * MON").unwrap();
    let json = serde_json::to_string(&expr).unwrap();
    assert_eq!(json, "\"0 0 * * MON\"");
    let decoded: CronExpression = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.as_str(), expr.as_str());
}

#[test]
fn cron_expression_fromstr_trait() {
    let expr: Result<CronExpression, _> = "0 0 * * *".parse();
    assert!(expr.is_ok());
    assert_eq!(expr.unwrap().as_str(), "0 0 * * *");
}
