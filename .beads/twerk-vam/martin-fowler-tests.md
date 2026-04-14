bead_id: twerk-vam
bead_title: "refactor: Create Url, Hostname, and CronExpression newtype wrappers"

# Martin Fowler Test Plan

## Happy Path Tests

### WebhookUrl
- `test_webhook_url_accepts_https_url_with_path`
- `test_webhook_url_accepts_http_url_with_port`
- `test_webhook_url_preserves_original_string`
- `test_webhook_url_roundtrip_serialize_deserialize`

### Hostname
- `test_hostname_accepts_simple_domain`
- `test_hostname_accepts_subdomain`
- `test_hostname_accepts_multilevel_subdomain`
- `test_hostname_preserves_original_case`
- `test_hostname_roundtrip_serialize_deserialize`

### CronExpression
- `test_cron_expression_accepts_5_field_standard`
- `test_cron_expression_accepts_6_field_with_seconds`
- `test_cron_expression_accepts_ranges`
- `test_cron_expression_accepts_lists`
- `test_cron_expression_accepts_wildcards`
- `test_cron_expression_roundtrip_serialize_deserialize`

---

## Error Path Tests

### WebhookUrl
- `test_webhook_url_rejects_empty_string`
- `test_webhook_url_rejects_invalid_url`
- `test_webhook_url_rejects_ftp_scheme`
- `test_webhook_url_rejects_file_scheme`
- `test_webhook_url_rejects_missing_host`
- `test_webhook_url_rejects_localhost_without_host`
- `test_webhook_url_rejects_ws_scheme`
- `test_webhook_url_rejects_wss_scheme`

### Hostname
- `test_hostname_rejects_empty_string`
- `test_hostname_rejects_too_long_hostname`
- `test_hostname_rejects_port_number_with_invalid_character_error`
- `test_hostname_rejects_colon_character`
- `test_hostname_rejects_leading_hyphen`
- `test_hostname_rejects_trailing_hyphen`
- `test_hostname_rejects_consecutive_dots`
- `test_hostname_rejects_label_starting_with_hyphen`
- `test_hostname_rejects_label_ending_with_hyphen`
- `test_hostname_rejects_all_numeric_label`

### CronExpression
- `test_cron_expression_rejects_empty_string`
- `test_cron_expression_rejects_invalid_expression`
- `test_cron_expression_rejects_wrong_field_count`

---

## Edge Case Tests

### WebhookUrl
- `test_webhook_url_handles_query_string`
- `test_webhook_url_handles_fragment`
- `test_webhook_url_handles_complex_path_with_multiple_segments`
- `test_webhook_url_https_default_port_preserved`
- `test_webhook_url_http_explicit_port`

### Hostname
- `test_hostname_handles_single_label`
- `test_hostname_handles_max_length_label_63_chars`
- `test_hostname_handles_max_total_length_253_chars`
- `test_hostname_handles_case_insensitivity`
- `test_hostname_handles_trailing_dot_fqdn`
- `test_hostname_handles_international_characters_rejected`

### CronExpression
- `test_cron_expression_handles_step_values`
- `test_cron_expression_handles_ranges_with_steps`
- `test_cron_expression_handles_day_names`
- `test_cron_expression_handles_month_names`
- `test_cron_expression_handles_question_mark`
- `test_cron_expression_handles_last_day_of_month`
- `test_cron_expression_handles_weekday`

---

## Contract Verification Tests

### WebhookUrl
- `test_webhook_url_precondition_scheme_validation`
- `test_webhook_url_precondition_host_presence`
- `test_webhook_url_postcondition_string_preservation`
- `test_webhook_url_invariant_non_empty`

### Hostname
- `test_hostname_precondition_length_bounds`
- `test_hostname_precondition_no_port_character`
- `test_hostname_precondition_label_format`
- `test_hostname_postcondition_string_preservation`
- `test_hostname_invariant_no_colon`
- `test_hostname_invariant_length_bounds`

### CronExpression
- `test_cron_expression_precondition_non_empty`
- `test_cron_expression_precondition_field_count`
- `test_cron_expression_postcondition_string_preservation`
- `test_cron_expression_invariant_field_count`

---

## Given-When-Then Scenarios

### Scenario 1: Valid HTTPS webhook URL creation
**Given:** A valid HTTPS URL string `"https://api.example.com/webhooks/endpoint?token=abc123"`
**When:** `WebhookUrl::new()` is called
**Then:**
- Returns `Ok(WebhookUrl)`
- `as_str()` returns the original string exactly
- `as_url().scheme()` returns `"https"`
- `as_url().host()` returns `Some(Domain("api.example.com"))`
- Serialization roundtrips successfully

### Scenario 2: Invalid scheme rejection
**Given:** A URL string with invalid scheme `"ftp://files.server.com/data"`
**When:** `WebhookUrl::new()` is called
**Then:**
- Returns `Err(WebhookUrlError::InvalidScheme("ftp"))`
- No URL is constructed

### Scenario 3: Hostname with port rejection
**Given:** A hostname string `"server.example.com:8080"`
**When:** `Hostname::new()` is called
**Then:**
- Returns `Err(HostnameError::InvalidCharacter(':'))`
- No hostname is constructed

### Scenario 4: Valid cron expression parsing
**Given:** A cron expression string `"0 30 9 * * Mon-Fri"`
**When:** `CronExpression::new()` is called
**Then:**
- Returns `Ok(CronExpression)`
- `as_str()` returns `"0 30 9 * * Mon-Fri"`
- `as_str()` contains 6 fields

### Scenario 5: Six-field cron with seconds
**Given:** A cron expression string `"0 0 0 1 * *"` (midnight on first of month)
**When:** `CronExpression::new()` is called
**Then:**
- Returns `Ok(CronExpression)`
- `as_str()` returns `"0 0 0 1 * *"`
- `as_str()` contains 6 fields

### Scenario 6: Invalid cron field count
**Given:** A cron expression string `"* * *"` (only 3 fields)
**When:** `CronExpression::new()` is called
**Then:**
- Returns `Err(CronExpressionError::InvalidFieldCount(3))`

---

## Property-Based Tests (Proptest)

### WebhookUrl
- Arbitrary valid https URLs are accepted
- Arbitrary valid http URLs are accepted
- Roundtrip serialization preserves value

### Hostname
- Arbitrary valid RFC 1123 hostnames are accepted
- No port numbers in generated hostnames
- Roundtrip serialization preserves value

### CronExpression
- Arbitrary valid 5-field cron expressions are accepted
- Arbitrary valid 6-field cron expressions are accepted
- Roundtrip serialization preserves value

---

## Test Implementation Notes

1. Use `proptest` for property-based testing where applicable
2. Error variants must be matched exhaustively in error path tests
3. Use `assert_matches` pattern for error variant verification
4. Serde roundtrip tests should use both JSON and other relevant formats
5. Each test function name should clearly describe what is being tested
