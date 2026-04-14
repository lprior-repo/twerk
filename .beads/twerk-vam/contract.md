bead_id: twerk-vam
bead_title: "refactor: Create Url, Hostname, and CronExpression newtype wrappers"
phase: 1
updated_at: 2026-04-13T00:00:00Z

# Contract Specification

## Context

- **Feature:** Create three newtype wrappers for domain primitives
- **Domain terms:**
  - `WebhookUrl`: Validated webhook URL with RFC 3986 compliance
  - `Hostname`: Validated DNS hostname with RFC 1123 compliance
  - `CronExpression`: Validated cron schedule expression (5-field or 6-field)
- **Assumptions:**
  - All types follow existing `domain_types.rs` patterns (QueueName, Priority, etc.)
  - `cron` crate is available for cron expression parsing
  - Serde serialization must be transparent (inner string serialized directly)
  - Error types use `thiserror` and implement `std::error::Error`
- **Open questions:**
  - None identified; codebase analysis complete

---

## WebhookUrl

### Type Definition

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
#[must_use = "WebhookUrl should be used; it validates at construction"]
pub struct WebhookUrl(String);
```

### Validation Rules
- Must be a valid URI per RFC 3986
- Scheme must be `http` or `https` (case-insensitive)
- Host component must be non-empty
- Port is optional (may be present or absent)
- Path must be non-empty or defaults to `/`
- Query and fragment components are allowed but optional

### Constructor Preconditions
- [PC1] Input string must parse successfully as a `url::Url`
- [PC2] Scheme must be `http` or `https` (reject `ftp`, `file`, `ws`, `wss`, etc.)
- [PC3] Host must be non-empty (reject `http://localhost` without host, `http://` alone)
- [PC4] If scheme is `https`, TLS semantics implied but not enforced at construction

### Postconditions
- [PO1] `WebhookUrl::as_str()` returns the original input string exactly
- [PO2] `WebhookUrl::as_url()` returns a `&Url` with parsed components
- [PO3] The inner string is preserved verbatim for serialization

### Invariants
- [I1] `as_str()` always returns a non-empty string
- [I2] `as_url().scheme()` is always `http` or `https`
- [I3] `as_url().host()` is always `Some(...)` (non-None)

### Error Taxonomy

| Variant | Trigger | Example |
|---------|---------|---------|
| `UrlParseError(String)` | String fails to parse as URL | `"not a url"` |
| `InvalidScheme(String)` | Scheme is not http/https | `"ftp://host/path"` → `InvalidScheme("ftp")` |
| `MissingHost` | URL has no host component | `"http://"` or `"file:///path"` |
| `UrlTooLong` | URL exceeds 2048 characters | `"https://..."` with very long URL |
| `SpaceInPath` | URL path contains unencoded spaces | `"https://host/path with spaces"` |

### Trait Implementations
- `Display`: Formats as the inner string
- `AsRef<str>`: Returns `&str` view of inner string
- `Deref<Target = str>`: Enables string slice operations
- `FromStr`: Parse from `&str` returning `Result<WebhookUrl, WebhookUrlError>`
- `serde::Serialize`: Serializes as the raw string (transparent wrapper)
- `serde::Deserialize`: Deserializes and validates via `new()`

### Contract Signatures

```rust
pub fn new(url: impl Into<String>) -> Result<WebhookUrl, WebhookUrlError>
pub fn as_str(&self) -> &str
pub fn as_url(&self) -> &Url

impl FromStr for WebhookUrl {
    type Err = WebhookUrlError;
}
```

---

## Hostname

### Type Definition

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
#[must_use = "Hostname should be used; it validates at construction"]
pub struct Hostname(String);
```

### Validation Rules (RFC 1123)
- Each label: 1-63 characters
- Labels: alphanumeric ASCII, hyphens allowed (but not at start or end)
- Total length: 1-253 characters
- No port number (reject `:` character explicitly)
- Case-insensitive but preserved as-is

### Constructor Preconditions
- [PC1] String length must be 1-253 characters
- [PC2] String must not be empty
- [PC3] String must not contain the character `:`
- [PC4] Each label must match: `^[a-zA-Z0-9]([a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?$`
  - First char: alphanumeric
  - Middle chars: alphanumeric or hyphen
  - Last char: alphanumeric
- [PC5] Labels cannot be all-numeric (to avoid ambiguity with IP addresses)

### Postconditions
- [PO1] `Hostname::as_str()` returns the original input string exactly
- [PO2] The inner string is preserved verbatim for serialization
- [PO3] Labels are NOT lowercased (original case preserved)

### Invariants
- [I1] `as_str()` always returns a string with length 1-253
- [I2] `as_str()` never contains the character `:`
- [I3] No label is empty (no consecutive dots, no leading/trailing dots unless the whole string is exactly `.`)

### Error Taxonomy

| Variant | Trigger | Example |
|---------|---------|---------|
| `Empty` | String is empty | `""` |
| `TooLong(usize)` | Length exceeds 253 | `"a".repeat(254)` → `TooLong(254)` |
| `InvalidCharacter(char)` | Contains disallowed character | `"host:8080"` → `InvalidCharacter(':')` |
| `InvalidLabel(label, reason)` | Label fails RFC 1123 rules | `"123.abc"` → `InvalidLabel("123", "all_numeric")` |
| `LabelTooLong(label, usize)` | Label exceeds 63 chars | `&"a".repeat(64)` → `LabelTooLong("aaaa...", 64)` |

### Trait Implementations
- `Display`: Formats as the inner string
- `AsRef<str>`: Returns `&str` view of inner string
- `Deref<Target = str>`: Enables string slice operations
- `FromStr`: Parse from `&str` returning `Result<Hostname, HostnameError>`
- `serde::Serialize`: Serializes as the raw string (transparent wrapper)
- `serde::Deserialize`: Deserializes and validates via `new()`

### Contract Signatures

```rust
pub fn new(hostname: impl Into<String>) -> Result<Hostname, HostnameError>
pub fn as_str(&self) -> &str

impl FromStr for Hostname {
    type Err = HostnameError;
}
```

---

## CronExpression

### Type Definition

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
#[must_use = "CronExpression should be used; it validates at construction"]
pub struct CronExpression(String);
```

### Validation Rules
- 5-field format: `second? minute hour day_of_month month day_of_week`
  - When seconds omitted (standard cron), field order: minute, hour, day_of_month, month, day_of_week
- 6-field format: `second minute hour day_of_month month day_of_week`
- Supported special characters: `*`, `?`, `-`, `,`, `/`
- Supported day names: `MON-SUN` (case-insensitive)
- Supported month names: `JAN-DEC` (case-insensitive)

### Constructor Preconditions
- [PC1] String must not be empty
- [PC2] String must parse successfully via `cron::Schedule::from_str`
- [PC3] Must be exactly 5 or 6 fields separated by spaces

### Postconditions
- [PO1] `CronExpression::as_str()` returns the original input string exactly
- [PO2] The inner string is preserved verbatim for serialization
- [PO3] The expression is compatible with the `cron` crate's `Schedule`

### Invariants
- [I1] `as_str()` always returns a non-empty string
- [I2] Contains 5 or 6 space-separated fields

### Error Taxonomy

| Variant | Trigger | Example |
|---------|---------|---------|
| `Empty` | String is empty | `""` |
| `ParseError(String)` | Cron parsing fails | `"not a cron"` → `ParseError("...")` |
| `InvalidFieldCount(usize)` | Not 5 or 6 fields | `"* * *"` → `InvalidFieldCount(3)` |

### Trait Implementations
- `Display`: Formats as the inner string
- `AsRef<str>`: Returns `&str` view of inner string
- `Deref<Target = str>`: Enables string slice operations
- `FromStr`: Parse from `&str` returning `Result<CronExpression, CronExpressionError>`
- `serde::Serialize`: Serializes as the raw string (transparent wrapper)
- `serde::Deserialize`: Deserializes and validates via `new()`

### Contract Signatures

```rust
pub fn new(expr: impl Into<String>) -> Result<CronExpression, CronExpressionError>
pub fn as_str(&self) -> &str

impl FromStr for CronExpression {
    type Err = CronExpressionError;
}
```

---

## Unified Error Type (DomainParseError Extension)

```rust
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DomainParseError {
    // ... existing variants ...
    #[error("invalid hostname: {0}")]
    Hostname(#[from] HostnameError),
    #[error("invalid webhook url: {0}")]
    WebhookUrl(#[from] WebhookUrlError),
}
```

---

## Non-Goals

- [NG1] TLS/SSL certificate validation for `WebhookUrl`
- [NG2] DNS resolution for `Hostname`
- [NG3] Actual cron schedule execution (parsing only)
- [NG4] IPv4 or IPv6 address validation (hostname only)
- [NG5] IDN/punycode support for international hostnames
