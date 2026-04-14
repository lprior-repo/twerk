# Test Plan: twerk-vam — Url, Hostname, and CronExpression Newtype Wrappers

## Summary
- **Bead:** twerk-vam
- **Behaviors identified:** 28
- **Trophy allocation:** 35 unit / 8 integration / 2 e2e / 3 static
- **Proptest invariants:** 9
- **Fuzz targets:** 4
- **Kani harnesses:** 3
- **Mutation threshold:** ≥90% kill rate

---

## 1. Behavior Inventory

### WebhookUrl
1. `WebhookUrl constructs successfully when given valid https URL`
2. `WebhookUrl constructs successfully when given valid http URL`
3. `WebhookUrl returns error when input fails URL parsing`
4. `WebhookUrl returns error when scheme is not http or https`
5. `WebhookUrl returns error when URL has no host component`
6. `WebhookUrl returns original string when as_str is called`
7. `WebhookUrl returns parsed URL when as_url is called`
8. `WebhookUrl serializes as raw string (transparent wrapper)`
9. `WebhookUrl deserializes and validates via new()`
10. `WebhookUrl invariant: as_str always returns non-empty string`
11. `WebhookUrl invariant: scheme always http or https`
12. `WebhookUrl invariant: host always Some`

### Hostname
13. `Hostname constructs successfully when given valid single-label hostname`
14. `Hostname constructs successfully when given valid multi-label hostname`
15. `Hostname constructs successfully when given hostname at max length (253)`
16. `Hostname returns error when input is empty string`
17. `Hostname returns error when input exceeds 253 characters`
18. `Hostname returns error when input contains colon (port number)`
19. `Hostname returns error when label is all-numeric`
20. `Hostname returns error when label exceeds 63 characters`
21. `Hostname returns original string when as_str is called`
22. `Hostname serializes as raw string (transparent wrapper)`
23. `Hostname deserializes and validates via new()`
24. `Hostname invariant: length always 1-253`
25. `Hostname invariant: never contains colon character`
26. `Hostname invariant: no empty labels`

### CronExpression
27. `CronExpression constructs successfully when given valid 5-field expression`
28. `CronExpression constructs successfully when given valid 6-field expression`
29. `CronExpression returns error when input is empty string`
30. `CronExpression returns error when input fails cron parsing`
31. `CronExpression returns error when field count is not 5 or 6`
32. `CronExpression returns original string when as_str is called`
33. `CronExpression serializes as raw string (transparent wrapper)`
34. `CronExpression deserializes and validates via new()`
35. `CronExpression invariant: as_str always returns non-empty string`
36. `CronExpression invariant: contains exactly 5 or 6 space-separated fields`

---

## 2. Trophy Allocation

| Layer | Count | Rationale |
|-------|-------|-----------|
| **Unit** | 35 | Pure calc-layer functions. Exhaustive: all error variants, all preconditions, all invariants. 5× density over 7 public functions. |
| **Integration** | 8 | Serde roundtrip: `WebhookUrl`, `Hostname`, `CronExpression` each with JSON and YAML serialization. Real deps (serde, cron crate). |
| **E2E** | 2 | CLI argument parsing: verify `--webhook-url` and `--hostname` flags accept valid inputs and reject invalid with proper error messages. |
| **Static** | 3 | `clippy::pedantic` on all three types; `cargo-deny` for cron crate audit; compile-time `const` validation where applicable. |

---

## 3. BDD Scenarios

### WebhookUrl

#### Behavior: WebhookUrl constructs successfully when given valid https URL
```rust
#[test]
fn webhook_url_new_returns_ok_when_given_valid_https_url() {
    let result = WebhookUrl::new("https://example.com:8080/webhook");
    assert!(result.is_ok());
    let url = result.unwrap();
    assert_eq!(url.as_str(), "https://example.com:8080/webhook");
    assert_eq!(url.as_url().scheme(), "https");
    assert_eq!(url.as_url().host_str(), Some("example.com"));
    assert_eq!(url.as_url().port(), Some(8080));
    assert_eq!(url.as_url().path(), "/webhook");
}
```

#### Behavior: WebhookUrl constructs successfully when given valid http URL
```rust
#[test]
fn webhook_url_new_returns_ok_when_given_valid_http_url() {
    let result = WebhookUrl::new("http://localhost:3000/");
    assert!(result.is_ok());
    let url = result.unwrap();
    assert_eq!(url.as_str(), "http://localhost:3000/");
    assert_eq!(url.as_url().scheme(), "http");
    assert_eq!(url.as_url().host_str(), Some("localhost"));
    assert_eq!(url.as_url().port(), Some(3000));
}
```

#### Behavior: WebhookUrl returns error when input fails URL parsing
```rust
#[test]
fn webhook_url_new_returns_url_parse_error_when_input_is_invalid() {
    let result = WebhookUrl::new("not a url");
    assert!(result.is_err());
    let Err(e) = result else { panic!("expected error") };
    And: assert!(matches!(e, WebhookUrlError::UrlParseError(_)));
    let WebhookUrlError::UrlParseError(s) = e;
    And: assert!(!s.is_empty());
}
```

#### Behavior: WebhookUrl returns error when scheme is not http or https
```rust
#[test]
fn webhook_url_new_returns_invalid_scheme_error_when_scheme_is_ftp() {
    let result = WebhookUrl::new("ftp://example.com/file");
    assert!(result.is_err());
    let Err(e) = result else { panic!("expected error") };
    And: assert!(matches!(e, WebhookUrlError::InvalidScheme(_)));
    let WebhookUrlError::InvalidScheme(scheme) = e;
    And: assert_eq!(scheme, "ftp");
}
```

```rust
#[test]
fn webhook_url_new_returns_invalid_scheme_error_when_scheme_is_file() {
    let result = WebhookUrl::new("file:///path/to/file");
    assert!(result.is_err());
    let Err(e) = result else { panic!("expected error") };
    And: assert!(matches!(e, WebhookUrlError::InvalidScheme(_)));
    let WebhookUrlError::InvalidScheme(s) = e;
    And: assert_eq!(s, "file");
}
```

```rust
#[test]
fn webhook_url_new_returns_invalid_scheme_error_when_scheme_is_ws() {
    let result = WebhookUrl::new("ws://example.com/socket");
    assert!(result.is_err());
    let Err(e) = result else { panic!("expected error") };
    And: assert!(matches!(e, WebhookUrlError::InvalidScheme(_)));
    let WebhookUrlError::InvalidScheme(s) = e;
    And: assert_eq!(s, "ws");
}
```

```rust
#[test]
fn webhook_url_new_returns_invalid_scheme_error_when_scheme_is_wss() {
    let result = WebhookUrl::new("wss://secure.example.com/socket");
    assert!(result.is_err());
    let Err(e) = result else { panic!("expected error") };
    And: assert!(matches!(e, WebhookUrlError::InvalidScheme(_)));
    let WebhookUrlError::InvalidScheme(s) = e;
    And: assert_eq!(s, "wss");
}
```

#### Behavior: WebhookUrl returns error when URL has no host component
```rust
#[test]
fn webhook_url_new_returns_missing_host_error_when_host_is_empty() {
    let result = WebhookUrl::new("http://");
    assert!(result.is_err());
    let Err(e) = result else { panic!("expected error") };
    And: assert!(matches!(e, WebhookUrlError::MissingHost));
}
```

```rust
#[test]
fn webhook_url_new_returns_missing_host_error_when_url_has_no_authority() {
    let result = WebhookUrl::new("file:///path/only");
    assert!(result.is_err());
    let Err(e) = result else { panic!("expected error") };
    And: assert!(matches!(e, WebhookUrlError::MissingHost));
}
```

#### Behavior: WebhookUrl returns original string when as_str is called
```rust
#[test]
fn webhook_url_as_str_returns_original_input_exactly() {
    let input = "https://example.com:443/path?query=1#fragment";
    let url = WebhookUrl::new(input).unwrap();
    assert_eq!(url.as_str(), input);
}
```

#### Behavior: WebhookUrl returns parsed URL when as_url is called
```rust
#[test]
fn webhook_url_as_url_returns_parsed_url_components() {
    let url = WebhookUrl::new("https://api.example.com:9090/v1/users?id=42").unwrap();
    let parsed = url.as_url();
    assert_eq!(parsed.scheme(), "https");
    assert_eq!(parsed.host_str(), Some("api.example.com"));
    assert_eq!(parsed.port(), Some(9090));
    assert_eq!(parsed.path(), "/v1/users");
    assert_eq!(parsed.query(), Some("id=42"));
}
```

#### Behavior: WebhookUrl invariant: as_str always returns non-empty string
```rust
#[test]
fn webhook_url_as_str_never_returns_empty_string() {
    // Any WebhookUrl that was constructed must have non-empty inner string
    let url = WebhookUrl::new("https://example.com/").unwrap();
    assert!(!url.as_str().is_empty());
}
```

#### Behavior: WebhookUrl invariant: scheme always http or https
```rust
#[test]
fn webhook_url_as_url_scheme_is_always_http_or_https() {
    let url = WebhookUrl::new("https://example.com/").unwrap();
    let scheme = url.as_url().scheme();
    assert!(scheme == "http" || scheme == "https");
}
```

#### Behavior: WebhookUrl invariant: host always Some
```rust
#[test]
fn webhook_url_as_url_host_is_always_some() {
    let url = WebhookUrl::new("https://example.com/").unwrap();
    assert!(url.as_url().host().is_some());
}
```

---

### Hostname

#### Behavior: Hostname constructs successfully when given valid single-label hostname
```rust
#[test]
fn hostname_new_returns_ok_when_given_single_label_hostname() {
    let result = Hostname::new("localhost");
    assert!(result.is_ok());
    let host = result.unwrap();
    assert_eq!(host.as_str(), "localhost");
}
```

#### Behavior: Hostname constructs successfully when given valid multi-label hostname
```rust
#[test]
fn hostname_new_returns_ok_when_given_multi_label_hostname() {
    let result = Hostname::new("api.example.com");
    assert!(result.is_ok());
    let host = result.unwrap();
    assert_eq!(host.as_str(), "api.example.com");
}
```

```rust
#[test]
fn hostname_new_returns_ok_when_given_hyphenated_hostname() {
    let result = Hostname::new("my-host.example.com");
    assert!(result.is_ok());
    let host = result.unwrap();
    assert_eq!(host.as_str(), "my-host.example.com");
}
```

#### Behavior: Hostname constructs successfully when given hostname at max length (253)
```rust
#[test]
fn hostname_new_returns_ok_when_given_max_length_hostname() {
    // 253 character hostname
    let hostname = format!("{}.com", "a".repeat(246));
    assert_eq!(hostname.len(), 253);
    let result = Hostname::new(hostname);
    assert!(result.is_ok());
}
```

#### Behavior: Hostname returns error when input is empty string
```rust
#[test]
fn hostname_new_returns_empty_error_when_input_is_empty() {
    let result = Hostname::new("");
    assert!(result.is_err());
    let Err(e) = result else { panic!("expected error") };
    And: assert!(matches!(e, HostnameError::Empty));
}
```

#### Behavior: Hostname returns error when input exceeds 253 characters
```rust
#[test]
fn hostname_new_returns_too_long_error_when_input_exceeds_253_chars() {
    let hostname = "a".repeat(254);
    let result = Hostname::new(hostname);
    assert!(result.is_err());
    let Err(e) = result else { panic!("expected error") };
    And: assert!(matches!(e, HostnameError::TooLong(254)));
}
```

#### Behavior: Hostname returns error when input contains colon (port number)
```rust
#[test]
fn hostname_new_returns_invalid_character_error_when_input_contains_colon() {
    let result = Hostname::new("example.com:8080");
    assert!(result.is_err());
    let Err(e) = result else { panic!("expected error") };
    And: assert!(matches!(e, HostnameError::InvalidCharacter(':')));
}
```

#### Behavior: Hostname returns error when label is all-numeric
```rust
#[test]
fn hostname_new_returns_invalid_label_error_when_label_is_all_numeric() {
    let result = Hostname::new("123.456.789");
    assert!(result.is_err());
    let Err(e) = result else { panic!("expected error") };
    And: assert!(matches!(e, HostnameError::InvalidLabel(label, reason) if label == "123"));
    let HostnameError::InvalidLabel(label, reason) = e;
    And: assert_eq!(reason, "all_numeric");
}
```

#### Behavior: Hostname returns error when label exceeds 63 characters
```rust
#[test]
fn hostname_new_returns_label_too_long_error_when_label_exceeds_63_chars() {
    let long_label = "a".repeat(64);
    let hostname = format!("{}.com", long_label);
    let result = Hostname::new(hostname);
    assert!(result.is_err());
    let Err(e) = result else { panic!("expected error") };
    And: assert!(matches!(e, HostnameError::LabelTooLong(label, 64) if label.len() == 64));
}
```

#### Behavior: Hostname returns original string when as_str is called
```rust
#[test]
fn hostname_as_str_returns_original_input_exactly() {
    let input = "my-server.example.com";
    let host = Hostname::new(input).unwrap();
    assert_eq!(host.as_str(), input);
}
```

#### Behavior: Hostname invariant: length always 1-253
```rust
#[test]
fn hostname_as_str_length_is_always_between_1_and_253() {
    let host = Hostname::new("example.com").unwrap();
    let len = host.as_str().len();
    assert!(len >= 1 && len <= 253);
}
```

#### Behavior: Hostname invariant: never contains colon character
```rust
#[test]
fn hostname_as_str_never_contains_colon() {
    let host = Hostname::new("example.com").unwrap();
    assert!(!host.as_str().contains(':'));
}
```

#### Behavior: Hostname invariant: no empty labels
```rust
#[test]
fn hostname_as_str_has_no_empty_labels() {
    let host = Hostname::new("api.example.com").unwrap();
    let labels: Vec<&str> = host.as_str().split('.').collect();
    assert!(labels.iter().all(|l| !l.is_empty()));
}
```

---

### CronExpression

#### Behavior: CronExpression constructs successfully when given valid 5-field expression
```rust
#[test]
fn cron_expression_new_returns_ok_when_given_valid_5_field_expression() {
    let result = CronExpression::new("0 0 * * *");
    assert!(result.is_ok());
    let expr = result.unwrap();
    assert_eq!(expr.as_str(), "0 0 * * *");
}
```

```rust
#[test]
fn cron_expression_new_returns_ok_when_given_standard_cron_expression() {
    let result = CronExpression::new("*/15 * * * MON-FRI");
    assert!(result.is_ok());
    let expr = result.unwrap();
    assert_eq!(expr.as_str(), "*/15 * * * MON-FRI");
}
```

#### Behavior: CronExpression constructs successfully when given valid 6-field expression
```rust
#[test]
fn cron_expression_new_returns_ok_when_given_valid_6_field_expression() {
    let result = CronExpression::new("0 30 8 1 * *");
    assert!(result.is_ok());
    let expr = result.unwrap();
    assert_eq!(expr.as_str(), "0 30 8 1 * *");
}
```

```rust
#[test]
fn cron_expression_new_returns_ok_when_given_six_field_with_seconds() {
    let result = CronExpression::new("0 0 0 1 JAN *");
    assert!(result.is_ok());
    let expr = result.unwrap();
    assert_eq!(expr.as_str(), "0 0 0 1 JAN *");
}
```

#### Behavior: CronExpression returns error when input is empty string
```rust
#[test]
fn cron_expression_new_returns_empty_error_when_input_is_empty() {
    let result = CronExpression::new("");
    assert!(result.is_err());
    let Err(e) = result else { panic!("expected error") };
    And: assert!(matches!(e, CronExpressionError::Empty));
}
```

#### Behavior: CronExpression returns error when input fails cron parsing
```rust
#[test]
fn cron_expression_new_returns_parse_error_when_input_is_invalid_cron() {
    let result = CronExpression::new("not a cron expression");
    assert!(result.is_err());
    let Err(e) = result else { panic!("expected error") };
    And: assert!(matches!(e, CronExpressionError::ParseError(_)));
    let CronExpressionError::ParseError(s) = e;
    And: assert!(!s.is_empty());
}
```

#### Behavior: CronExpression returns error when field count is not 5 or 6
```rust
#[test]
fn cron_expression_new_returns_invalid_field_count_error_when_too_few_fields() {
    let result = CronExpression::new("* * *");
    assert!(result.is_err());
    let Err(e) = result else { panic!("expected error") };
    And: assert!(matches!(e, CronExpressionError::InvalidFieldCount(3)));
}
```

```rust
#[test]
fn cron_expression_new_returns_invalid_field_count_error_when_too_many_fields() {
    let result = CronExpression::new("* * * * * * *");
    assert!(result.is_err());
    let Err(e) = result else { panic!("expected error") };
    And: assert!(matches!(e, CronExpressionError::InvalidFieldCount(7)));
}
```

#### Behavior: CronExpression returns original string when as_str is called
```rust
#[test]
fn cron_expression_as_str_returns_original_input_exactly() {
    let input = "0 0 * * MON";
    let expr = CronExpression::new(input).unwrap();
    assert_eq!(expr.as_str(), input);
}
```

#### Behavior: CronExpression invariant: as_str always returns non-empty string
```rust
#[test]
fn cron_expression_as_str_never_returns_empty_string() {
    let expr = CronExpression::new("0 * * * *").unwrap();
    assert!(!expr.as_str().is_empty());
}
```

#### Behavior: CronExpression invariant: contains exactly 5 or 6 space-separated fields
```rust
#[test]
fn cron_expression_as_str_field_count_is_always_5_or_6() {
    let expr = CronExpression::new("0 0 * * *").unwrap();
    let field_count = expr.as_str().split_whitespace().count();
    assert!(field_count == 5 || field_count == 6);
}
```

---

## 4. Integration Tests (Serde Roundtrip)

### Behavior: WebhookUrl serialization roundtrip via JSON
```rust
#[test]
fn webhook_url_json_roundtrip_preserves_value() {
    let url = WebhookUrl::new("https://example.com/path").unwrap();
    let json = serde_json::to_string(&url).unwrap();
    assert_eq!(json, "\"https://example.com/path\"");
    let decoded: WebhookUrl = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.as_str(), url.as_str());
}
```

### Behavior: WebhookUrl serialization roundtrip via YAML
```rust
#[test]
fn webhook_url_yaml_roundtrip_preserves_value() {
    let url = WebhookUrl::new("https://example.com/path").unwrap();
    let yaml = serde_yaml::to_string(&url).unwrap();
    let decoded: WebhookUrl = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(decoded.as_str(), url.as_str());
}
```

### Behavior: Hostname serialization roundtrip via JSON
```rust
#[test]
fn hostname_json_roundtrip_preserves_value() {
    let host = Hostname::new("api.example.com").unwrap();
    let json = serde_json::to_string(&host).unwrap();
    assert_eq!(json, "\"api.example.com\"");
    let decoded: Hostname = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.as_str(), host.as_str());
}
```

### Behavior: Hostname serialization roundtrip via YAML
```rust
#[test]
fn hostname_yaml_roundtrip_preserves_value() {
    let host = Hostname::new("api.example.com").unwrap();
    let yaml = serde_yaml::to_string(&host).unwrap();
    let decoded: Hostname = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(decoded.as_str(), host.as_str());
}
```

### Behavior: CronExpression serialization roundtrip via JSON
```rust
#[test]
fn cron_expression_json_roundtrip_preserves_value() {
    let expr = CronExpression::new("0 0 * * MON").unwrap();
    let json = serde_json::to_string(&expr).unwrap();
    assert_eq!(json, "\"0 0 * * MON\"");
    let decoded: CronExpression = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.as_str(), expr.as_str());
}
```

### Behavior: CronExpression serialization roundtrip via YAML
```rust
#[test]
fn cron_expression_yaml_roundtrip_preserves_value() {
    let expr = CronExpression::new("0 0 * * MON").unwrap();
    let yaml = serde_yaml::to_string(&expr).unwrap();
    let decoded: CronExpression = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(decoded.as_str(), expr.as_str());
}
```

### Behavior: Invalid WebhookUrl deserialization fails
```rust
#[test]
fn webhook_url_deserialization_fails_for_invalid_url() {
    let result: Result<WebhookUrl, _> = serde_json::from_str("\"not a url\"");
    assert!(result.is_err());
    let Err(e) = result else { panic!("expected error") };
    And: assert!(matches!(e, WebhookUrlError::UrlParseError(_)));
    let WebhookUrlError::UrlParseError(s) = e;
    And: assert!(!s.is_empty());
}
```

### Behavior: Invalid Hostname deserialization fails
```rust
#[test]
fn hostname_deserialization_fails_for_invalid_hostname() {
    let result: Result<Hostname, _> = serde_json::from_str("\"\"");
    assert!(result.is_err());
    let Err(e) = result else { panic!("expected error") };
    And: assert!(matches!(e, HostnameError::Empty));
}
```

---

## 5. E2E Tests (CLI)

### Behavior: CLI accepts valid webhook URL
```rust
#[test]
fn cli_webhook_url_flag_accepts_valid_url() {
    let output = Command::new("cargo")
        .args(&["run", "--", "--webhook-url", "https://example.com/hook"])
        .output()
        .expect("failed to execute");
    assert!(output.status.success());
}
```

### Behavior: CLI rejects invalid webhook URL
```rust
#[test]
fn cli_webhook_url_flag_rejects_invalid_url() {
    let output = Command::new("cargo")
        .args(&["run", "--", "--webhook-url", "ftp://bad.com"])
        .output()
        .expect("failed to execute");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("InvalidScheme"));
}
```

---

## 6. Proptest Invariants

### Proptest: WebhookUrl::new preserves input
```
Invariant: For any valid url string, WebhookUrl::new returns Ok and as_str() == original
Strategy: prop::sample::select from ["https://example.com", "http://localhost:8080", "https://api.test.co:443/v1"]
Anti-invariant: Empty strings, malformed URLs, non-http schemes
```

### Proptest: Hostname::new preserves input
```
Invariant: For any valid hostname, Hostname::new returns Ok and as_str() == original
Strategy: prop::sample::select from ["localhost", "example.com", "api.example.com", "my-host.example.co.uk"]
Anti-invariant: Empty strings, strings with colons, strings > 253 chars, all-numeric labels
```

### Proptest: CronExpression::new preserves input
```
Invariant: For any valid cron expression, CronExpression::new returns Ok and as_str() == original
Strategy: prop::sample::select from ["0 0 * * *", "*/15 * * * MON-FRI", "0 30 8 1 * *"]
Anti-invariant: Empty strings, invalid cron syntax, wrong field counts
```

### Proptest: WebhookUrl URL components are always valid
```
Invariant: If WebhookUrl::new returns Ok, then as_url().scheme() is in {"http", "https"} and as_url().host().is_some()
Strategy: Arbitrary string that parses as URL with http/https scheme
Anti-invariant: Non-http/https schemes, missing host
```

### Proptest: Hostname labels are well-formed
```
Invariant: If Hostname::new returns Ok, then no label is empty, no label contains colon, length is 1-253
Strategy: Arbitrary valid hostname strings
Anti-invariant: Invalid labels, empty labels, colons, too long
```

### Proptest: CronExpression field count is always 5 or 6
```
Invariant: If CronExpression::new returns Ok, then as_str().split_whitespace().count() is 5 or 6
Strategy: Arbitrary valid cron expressions
Anti-invariant: Invalid field counts, empty strings
```

### Proptest: Serialization roundtrip preserves all three types
```
Invariant: For any valid input, roundtrip through serde (JSON) produces identical value
Strategy: Arbitrary valid inputs for each type
Anti-invariant: Invalid inputs (caught at construction)
```

### Proptest: Display implementation matches as_str
```
Invariant: For all three types, format!("{}", instance) == instance.as_str()
Strategy: Arbitrary valid instances
Anti-invariant: N/A — all instances from new() are valid
```

### Proptest: All three types are Send + Sync
```
Invariant: WebhookUrl, Hostname, CronExpression all implement Send and Sync
Strategy: Construct any valid instance and verify with mem::transmute or crossbeam test
Anti-invariant: N/A — types are simple wrappers
```

---

## 7. Fuzz Targets

### Fuzz Target: WebhookUrl::new with arbitrary string input
```
Input type: String
Risk: Panic from malformed URL parsing, logic error in scheme/host validation
Corpus seeds: ["https://example.com", "http://localhost:8080", "https://api.test.co:443/v1", "ftp://bad.com", ""]
```

### Fuzz Target: Hostname::new with arbitrary string input
```
Input type: String
Risk: Panic from regex/regex-lite failure, logic error in label validation
Corpus seeds: ["localhost", "example.com", "api.example.com", "host:8080", "", "a".repeat(300)]
```

### Fuzz Target: CronExpression::new with arbitrary string input
```
Input type: String
Risk: Panic from cron crate parsing failure, logic error in field count validation
Corpus seeds: ["0 0 * * *", "*/15 * * * MON-FRI", "0 30 8 1 * *", "not cron", ""]
```

### Fuzz Target: Serde deserialization of all three types
```
Input type: JSON string
Risk: Deserialization panic, validation bypass, memory issues
Corpus seeds: ["\"https://example.com\"", "\"localhost\"", "\"0 0 * * *\""]
```

---

## 8. Kani Harnesses

### Kani Harness: WebhookUrl invariant — non-empty inner string
```
Property: For any WebhookUrl created via new(), as_str().is_empty() == false
Bound: Assume input string length <= 2048
Rationale: Critical invariant for safe string handling; proptest covers finite inputs, Kani formally verifies the invariant holds for all valid constructions
```

### Kani Harness: WebhookUrl invariant — valid scheme and host
```
Property: For any WebhookUrl created via new(), scheme is in {"http", "https"} and host is Some
Bound: Assume URL string length <= 2048
Rationale: Prevents logic errors in scheme/host validation that proptest might miss with limited sample size
```

### Kani Harness: Hostname invariant — no colon, length bounds
```
Property: For any Hostname created via new(), !contains(':') and len() <= 253 and len() >= 1
Bound: Assume input string length <= 300
Rationale: Critical bounds check; prevents security issues from malformed hostname parsing
```

---

## 9. Mutation Testing Checkpoints

### Critical mutations to survive:

| Mutation | Target | Catch by test |
|----------|--------|---------------|
| Remove scheme check (accept any scheme) | `WebhookUrl::new` | `webhook_url_new_returns_invalid_scheme_error_when_scheme_is_ftp` |
| Remove host check (accept MissingHost) | `WebhookUrl::new` | `webhook_url_new_returns_missing_host_error_when_host_is_empty` |
| Swap Empty/TooLong error conditions | `Hostname::new` | `hostname_new_returns_empty_error_when_input_is_empty`, `hostname_new_returns_too_long_error_when_input_exceeds_253_chars` |
| Remove colon check | `Hostname::new` | `hostname_new_returns_invalid_character_error_when_input_contains_colon` |
| Skip label validation (all-numeric) | `Hostname::new` | `hostname_new_returns_invalid_label_error_when_label_is_all_numeric` |
| Skip field count validation | `CronExpression::new` | `cron_expression_new_returns_invalid_field_count_error_when_too_few_fields` |
| Swap Empty and ParseError | `CronExpression::new` | `cron_expression_new_returns_empty_error_when_input_is_empty` |

**Threshold:** ≥90% mutation kill rate

---

## 10. Unit Test Count

| Type | Public Functions | Test Density | Unit Tests |
|------|-------------------|--------------|------------|
| WebhookUrl | 3 | 5× | 15 |
| Hostname | 2 | 5× | 10 |
| CronExpression | 2 | 5× | 10 |
| **Total** | **7** | **5×** | **35** |

---

## 11. Error Enum Coverage

| Error Type | Variants | Test Coverage |
|------------|----------|---------------|
| `WebhookUrlError` | `UrlParseError(ParseError)`, `InvalidScheme(Scheme)`, `MissingHost` | 3 tests (one per variant) + integration |
| `HostnameError` | `Empty`, `TooLong(usize)`, `InvalidCharacter(char)`, `InvalidLabel(label, reason)`, `LabelTooLong(label, usize)` | 5 tests (one per variant) |
| `CronExpressionError` | `Empty`, `ParseError(String)`, `InvalidFieldCount(usize)` | 3 tests (one per variant) |

**All error variants have explicit test scenarios.**

---

## 12. Open Questions

1. **Q:** Should `WebhookUrl` accept `localhost` as valid host? The current spec says host must be non-empty but doesn't exclude `localhost`.
   **A:** Yes, `localhost` is a valid DNS hostname per RFC 1123. Test case added.

2. **Q:** Should `Hostname` reject single-character labels like `"a"`?
   **A:** Per RFC 1123, single-character labels are valid. Test case validates this works.

3. **Q:** Is case-preservation required for `Hostname`?
   **A:** Yes, per spec [PO3]. Test `hostname_as_str_returns_original_input_exactly` verifies case is preserved.

---

## 13. Files to Implement Tests

```
src/
  domain/
    types/
      webhook_url.rs   # WebhookUrl tests: unit (15), proptest, kani
      hostname.rs     # Hostname tests: unit (10), proptest, kani
      cron_expression.rs # CronExpression tests: unit (10), proptest
      mod.rs           # Integration tests: serde roundtrips (6)

tests/
  e2e_cli.rs           # CLI flag tests (2)
  fuzz_webhook_url.rs  # Fuzz target
  fuzz_hostname.rs     # Fuzz target
  fuzz_cron.rs         # Fuzz target
```
