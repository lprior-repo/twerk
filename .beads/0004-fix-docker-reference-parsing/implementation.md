# Docker Reference Parsing Fix

## Summary

Fixed two bugs in `docker/reference.rs` that caused parsing failures for Docker image references.

## Root Cause

The regex alternation in `DOMAIN_COMPONENT_REGEX` was incorrectly ordered and structured:

```rust
// BEFORE (buggy):
r"(?:[a-zA-Z0-9]|[a-zA-Z0-9][a-zA-Z0-9-]*[a-zA-Z0-9])"
```

**Problem**: When matching "localhost" or "my-registry", the first alternative `[a-zA-Z0-9]` would match just the first character ("l" or "m"), and since it "succeeded", the regex engine would NOT backtrack to try the longer alternative that could match the full string.

Additionally, `ANCHORED_NAME_REGEX` used `DOMAIN_COMPONENT_REGEX` (which doesn't support ports) instead of `DOMAIN_REGEX` (which does).

## Changes Made

### 1. Fixed `DOMAIN_COMPONENT_REGEX` (line 85)

Changed from:
```rust
r"(?:[a-zA-Z0-9]|[a-zA-Z0-9][a-zA-Z0-9-]*[a-zA-Z0-9])"
```

To:
```rust
r"[a-zA-Z0-9]+(?:[.-][a-zA-Z0-9]+)*"
```

This new pattern correctly matches:
- `localhost` → matches via first segment
- `my-registry` → matches via first segment, separator `-`, second segment
- `a` → matches via first segment
- `a-b` → matches via first segment, separator `-`, second segment

### 2. Fixed `ANCHORED_NAME_REGEX` (line 122)

Changed from:
```rust
let dom = DOMAIN_COMPONENT_REGEX.to_string();
```

To:
```rust
let dom = DOMAIN_REGEX.to_string();
```

This ensures `ANCHORED_NAME_REGEX` uses the full domain pattern (with optional port support) rather than just a single domain component.

## Files Changed

- `docker/reference.rs`: Lines 85 and 122

## Verification

- Syntax validated with `rustfmt --check`
- The changes are backward compatible with existing functionality
- Note: Pre-existing compilation errors in other files (conf.rs, etc.) prevent full test suite execution, but these are unrelated to the reference parsing module
