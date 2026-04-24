# Findings: tw-4ww1 - user-cli: repair user creation contract mismatch

## Issue Identified

The CLI `user create` command had a contract mismatch with the API endpoint:

### Original CLI Behavior (BUG)
- CLI only collected `username` from user
- CLI sent `{"username": username}` to API
- CLI expected API to return `201 CREATED` with `UserCreateResponse` body

### API Contract Requirements
- API requires BOTH `username` AND `password` in request body
- API returns `400 BAD_REQUEST` if password is missing
- API returns `200 OK` with empty body on success (not `201 CREATED`)

## Root Cause

File: `crates/twerk-cli/src/handlers/user.rs`
- The `user_create` function only sent `{"username": username}`
- The response parsing expected `UserCreateResponse` which was never returned

File: `crates/twerk-cli/src/commands.rs`
- `UserCommand::Create` only had `username` argument, no `password`

## Fixes Applied

### 1. commands.rs
Added `password` required argument to `UserCommand::Create`:

```rust
Create {
    username: String,
    password: String,  // NEW
}
```

### 2. handlers/user.rs
- Updated `user_create` to accept `password: &str`
- Updated request body to include both fields: `{"username": username, "password": password}`
- Changed response handling from `201 CREATED` to `200 OK`
- Removed unused `User` and `UserCreateResponse` structs

### 3. dispatch.rs
- Updated to pass `password` to `user_create`: `handlers::user::user_create(ep_str, &username, &password, json_mode)`

## Verification

Build succeeded: `cargo build --package twerk-cli`
Tests passed: 140 passed (6 suites)

## Infrastructure Note

Dolt database has PROJECT IDENTITY MISMATCH warning:
- Local metadata expects: `e73a37e0-a1e9-417b-940b-bce186abda73`
- Database contains: `af445fe7-feaa-48f5-b33b-258b66d93a10`

This caused `bd update tw-4ww1 --claim` to fail, but `bd show tw-4ww1 --json` worked.

## Status

COMPLETED - Contract mismatch fixed, build passes, tests pass.