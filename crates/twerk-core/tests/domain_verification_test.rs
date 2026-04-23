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
    let webhook_url = result.expect("https://example.com should be a valid webhook URL");
    assert_eq!(webhook_url.as_str(), url);
}

#[test]
fn webhook_url_valid_https_urls_2() {
    let url = "https://example.com/path";
    let result = WebhookUrl::new(url);
    let webhook_url = result.expect("https://example.com/path should be a valid webhook URL");
    assert_eq!(webhook_url.as_str(), url);
}

#[test]
fn webhook_url_valid_https_urls_3() {
    let url = "https://example.com/path?query=value";
    let webhook_url = WebhookUrl::new(url)
        .expect("https://example.com/path?query=value should be a valid webhook URL");
    assert_eq!(webhook_url.as_str(), url);
}

#[test]
fn webhook_url_valid_https_urls_4() {
    let url = "https://localhost";
    let webhook_url =
        WebhookUrl::new(url).expect("https://localhost should be a valid webhook URL");
    assert_eq!(webhook_url.as_str(), url);
}

#[test]
fn webhook_url_valid_https_urls_5() {
    let url = "https://localhost:8080";
    let webhook_url =
        WebhookUrl::new(url).expect("https://localhost:8080 should be a valid webhook URL");
    assert_eq!(webhook_url.as_str(), url);
}

#[test]
fn webhook_url_valid_https_urls_6() {
    let url = "http://example.com";
    let result = WebhookUrl::new(url);
    let webhook_url = result.expect("http://example.com should be a valid webhook URL");
    assert_eq!(webhook_url.as_str(), url);
}

#[test]
fn webhook_url_invalid_scheme_rejected() {
    // file:// scheme
    let result = WebhookUrl::new("file:///path");
    let err = result.expect_err("file scheme should be rejected");
    assert!(matches!(err, WebhookUrlError::InvalidScheme(s) if s == "file"));

    // ws:// scheme
    let result = WebhookUrl::new("ws://example.com");
    let err = result.expect_err("ws scheme should be rejected");
    assert!(matches!(err, WebhookUrlError::InvalidScheme(s) if s == "ws"));

    // ftp:// scheme
    let result = WebhookUrl::new("ftp://example.com");
    let err = result.expect_err("ftp scheme should be rejected");
    assert!(matches!(err, WebhookUrlError::InvalidScheme(s) if s == "ftp"));
}

#[test]
fn webhook_url_empty_string_rejected() {
    let result = WebhookUrl::new("");
    let error = result.expect_err("empty webhook URL should be rejected");
    assert!(matches!(error, WebhookUrlError::UrlParseError(_)));
}

#[test]
fn webhook_url_empty_host_rejected() {
    // URL with empty host - the url crate rejects this with UrlParseError("empty host")
    let result = WebhookUrl::new("http://");
    let error = result.expect_err("empty-host webhook URL should be rejected");
    assert!(matches!(error, WebhookUrlError::UrlParseError(msg) if msg.contains("empty host")));
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
    let webhook_url = url.expect("FromStr should accept a valid https URL");
    assert_eq!(webhook_url.as_str(), "https://example.com");
}

#[test]
fn webhook_url_minimal_valid_url_accepted() {
    // Minimal valid URL with just scheme and single-char host
    let result = WebhookUrl::new("http://a.b");
    let webhook_url = result.expect("minimal valid webhook URL should be accepted");
    assert_eq!(webhook_url.as_str(), "http://a.b");
}

#[test]
fn webhook_url_https_minimal_accepted() {
    // Minimal https URL
    let webhook_url = WebhookUrl::new("https://x.y").expect("minimal https URL should be valid");
    assert_eq!(webhook_url.as_str(), "https://x.y");
}

#[test]
fn webhook_url_http_with_path_accepted() {
    // Minimal URL with path
    let webhook_url =
        WebhookUrl::new("http://localhost/").expect("URL with root path should be valid");
    assert_eq!(webhook_url.as_str(), "http://localhost/");
}

#[test]
fn webhook_url_localhost_accepted() {
    // Localhost variations
    let webhook_url = WebhookUrl::new("http://localhost").expect("localhost URL should be valid");
    assert_eq!(webhook_url.as_str(), "http://localhost");
}

// ============================================================================
// Hostname Tests
// ============================================================================

#[test]
fn hostname_valid_hostnames_1() {
    let hostname = "localhost";
    let parsed_hostname = Hostname::new(hostname).expect("localhost should be a valid hostname");
    assert_eq!(parsed_hostname.as_str(), hostname);
}

#[test]
fn hostname_valid_hostnames_2() {
    let hostname = "example.com";
    let parsed_hostname = Hostname::new(hostname).expect("example.com should be a valid hostname");
    assert_eq!(parsed_hostname.as_str(), hostname);
}

#[test]
fn hostname_valid_hostnames_3() {
    let hostname = "api.example.com";
    let parsed_hostname =
        Hostname::new(hostname).expect("api.example.com should be a valid hostname");
    assert_eq!(parsed_hostname.as_str(), hostname);
}

#[test]
fn hostname_valid_hostnames_4() {
    let hostname = "my-host.example.com";
    let parsed_hostname =
        Hostname::new(hostname).expect("my-host.example.com should be a valid hostname");
    assert_eq!(parsed_hostname.as_str(), hostname);
}

#[test]
fn hostname_valid_hostnames_5() {
    let hostname = "a.b.c.d";
    let parsed_hostname = Hostname::new(hostname).expect("a.b.c.d should be a valid hostname");
    assert_eq!(parsed_hostname.as_str(), hostname);
}

#[test]
fn hostname_valid_hostnames_6() {
    let hostname = "example-domain.com";
    let result = Hostname::new(hostname);
    let parsed_hostname = result.expect("example-domain.com should be a valid hostname");
    assert_eq!(parsed_hostname.as_str(), hostname);
}

#[test]
fn hostname_port_rejected() {
    // Hostname with port should be rejected with InvalidCharacter(':')
    let result = Hostname::new("example.com:8080");
    let err = result.expect_err("hostnames containing ports should be rejected");
    assert!(matches!(err, HostnameError::InvalidCharacter(':')));

    let result = Hostname::new("localhost:3000");
    let err = result.expect_err("localhost with port should be rejected");
    assert!(matches!(err, HostnameError::InvalidCharacter(':')));
}

#[test]
fn hostname_too_long_rejected() {
    // 254 characters should fail (> 253)
    let long_hostname = "a".repeat(254);
    let result = Hostname::new(&long_hostname);
    let err = result.expect_err("254-character hostname should be rejected");
    assert!(matches!(err, HostnameError::TooLong(len) if len == 254));
}

#[test]
fn hostname_empty_rejected() {
    let result = Hostname::new("");
    let err = result.expect_err("empty hostname should be rejected");
    assert!(matches!(err, HostnameError::Empty));
}

#[test]
fn hostname_all_numeric_rejected() {
    // All-numeric labels should be rejected (avoid IP ambiguity)
    // Labels must start and end with alphanumeric
    let result = Hostname::new("123.456.789.0");
    let err = result.expect_err("all-numeric hostname should be rejected");
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
    let hostname = host.expect("FromStr should accept a valid hostname");
    assert_eq!(hostname.as_str(), "example.com");
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
    let parsed_hostname = Hostname::new(&hostname).expect("253-character hostname should be valid");
    assert_eq!(parsed_hostname.as_str(), hostname);
}

#[test]
fn hostname_min_length_single_char_accepted() {
    // Single character hostname is valid (min boundary)
    let parsed_hostname = Hostname::new("a").expect("single-character hostname should be valid");
    assert_eq!(parsed_hostname.as_str(), "a");
}

#[test]
fn hostname_label_max_length_63_accepted() {
    // Exactly 63-char label should be valid (max label boundary)
    let label = "a".repeat(63);
    let hostname = format!("{}.com", label);
    assert_eq!(label.len(), 63);
    let parsed_hostname =
        Hostname::new(&hostname).expect("63-character label hostname should be valid");
    assert_eq!(parsed_hostname.as_str(), hostname);
}

#[test]
fn hostname_label_exceeds_64_rejected() {
    // Exactly 64-char label should fail (one above max label boundary)
    let label = "a".repeat(64);
    let hostname = format!("{}.com", label);
    assert_eq!(label.len(), 64);
    let result = Hostname::new(&hostname);
    let err = result.expect_err("64-character label should be rejected");
    assert!(matches!(err, HostnameError::LabelTooLong(l, 64) if l.len() == 64));
}

#[test]
fn hostname_invalid_character_at_start_rejected() {
    // Label starting with hyphen should be rejected
    let result = Hostname::new("-example.com");
    let err = result.expect_err("hostname labels must start with an alphanumeric character");
    assert!(
        matches!(err, HostnameError::InvalidLabel(label, reason) if label == "-example" && reason == "must start with alphanumeric")
    );
}

#[test]
fn hostname_invalid_character_at_end_rejected() {
    // Label ending with hyphen should be rejected
    let result = Hostname::new("example-.com");
    let err = result.expect_err("hostname labels must end with an alphanumeric character");
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
    let parsed_cron = CronExpression::new(cron).expect("0 0 * * MON should be a valid cron");
    assert_eq!(parsed_cron.as_str(), cron);
}

#[test]
fn cron_expression_valid_5_field_2() {
    let cron = "0 0 * * *";
    let parsed_cron = CronExpression::new(cron).expect("0 0 * * * should be a valid cron");
    assert_eq!(parsed_cron.as_str(), cron);
}

#[test]
fn cron_expression_valid_5_field_3() {
    let cron = "*/5 * * * *";
    let parsed_cron = CronExpression::new(cron).expect("*/5 * * * * should be a valid cron");
    assert_eq!(parsed_cron.as_str(), cron);
}

#[test]
fn cron_expression_valid_5_field_4() {
    let cron = "0 30 8 1 1 *";
    let parsed_cron = CronExpression::new(cron).expect("0 30 8 1 1 * should be a valid cron");
    assert_eq!(parsed_cron.as_str(), cron);
}

#[test]
fn cron_expression_valid_6_field_1() {
    let cron = "0 0 0 1 1 *";
    let parsed_cron = CronExpression::new(cron).expect("0 0 0 1 1 * should be a valid cron");
    assert_eq!(parsed_cron.as_str(), cron);
}

#[test]
fn cron_expression_valid_6_field_2() {
    let cron = "0 30 8 15 * *";
    let parsed_cron = CronExpression::new(cron).expect("0 30 8 15 * * should be a valid cron");
    assert_eq!(parsed_cron.as_str(), cron);
}

#[test]
fn cron_expression_valid_6_field_3() {
    let cron = "0 0 12 * * 1-5";
    let parsed_cron = CronExpression::new(cron).expect("0 0 12 * * 1-5 should be a valid cron");
    assert_eq!(parsed_cron.as_str(), cron);
}

#[test]
fn cron_expression_invalid_rejected_1() {
    let cron = "not a cron";
    assert_eq!(
        CronExpression::new(cron),
        Err(CronExpressionError::InvalidFieldCount(3))
    );
}

#[test]
fn cron_expression_invalid_rejected_2() {
    let cron = "* * *"; // 3 fields
    assert_eq!(
        CronExpression::new(cron),
        Err(CronExpressionError::InvalidFieldCount(3))
    );
}

#[test]
fn cron_expression_invalid_rejected_3() {
    let cron = "* * * * * * *"; // 7 fields
    assert_eq!(
        CronExpression::new(cron),
        Err(CronExpressionError::InvalidFieldCount(7))
    );
}

#[test]
fn cron_expression_empty_rejected() {
    let result = CronExpression::new("");
    let err = result.expect_err("empty cron expression should be rejected");
    assert!(matches!(err, CronExpressionError::Empty));
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
    let cron_expression = expr.expect("FromStr should accept a valid cron expression");
    assert_eq!(cron_expression.as_str(), "0 0 * * *");
}
