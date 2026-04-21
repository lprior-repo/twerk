# Test Plan: Red Queen - twerk-cli

**Module under test:** `crates/twerk-cli`
**Red Queen Session:** `drq-twerk-cli-v3`
**Date:** 2026-04-17

---

## Section 1 — CLI Contract (done_when checks)

The following commands form the permanent regression lineage (9 checks, ALL PASS):

| # | Command | Expected Exit | Dimension | Severity |
|---|---------|---------------|-----------|----------|
| C1 | `./target/release/twerk --help` | 0 | contract | MAJOR |
| C2 | `./target/release/twerk --version` | 0 | contract | MAJOR |
| C3 | `./target/release/twerk --json` | 0 | contract | MAJOR |
| C4 | `./target/release/twerk run --help` | 0 | contract | MAJOR |
| C5 | `./target/release/twerk migration --help` | 0 | contract | MAJOR |
| C6 | `./target/release/twerk health --help` | 0 | contract | MAJOR |
| E1 | `./target/release/twerk run invalid-mode` | 2 | error-handling | MAJOR |
| E2 | `./target/release/twerk run` | 2 | error-handling | MAJOR |
| E3 | `./target/release/twerk migration` | 1 | error-handling | MAJOR |

**Result: 9/9 PASS — ratchet holds**

---

## Section 2 — Quality Gate Results (Generation 1)

| Gate | Status | Details | Bead |
|------|--------|---------|------|
| No Panic | **FAIL** | `webhook_url.rs:114` uses `.expect()` | tw-bmu, tw-o8o |
| Exhaustive Match | **FAIL** | `parsing.rs:66` wildcard enum match | tw-60d, tw-22k |
| Format | **FAIL** | Test files formatting issues | tw-jy3 |
| Lint | **FAIL** | clippy warnings | - |
| Tests | **FAIL** | cargo test failures | - |
| DRY | **FAIL** | `in_memory.rs:137` unnecessary_wraps, `:164` redundant_clone | tw-ek0, tw-cem |
| Cognitive | **FAIL** | cognitive complexity violations | - |
| Test Coverage | **FAIL** | coverage below 80% | - |
| Security | **FAIL** | RUSTSEC-2023-0071 RSA, RUSTSEC-2026-0098 rustls-webpki | tw-y4b, tw-qre |
| Licenses | **FAIL** | cargo deny license issues | - |

**Result: 16 survivors across 11 dimensions**

---

## Section 3 — Findings Summary

### Critical Issues (Fix Immediately)

| ID | Dimension | Severity | Finding | File |
|----|-----------|----------|---------|------|
| GEN-1-5 | fp-gate-tests | CRITICAL | cargo test failures | multiple |
| GEN-1-1 | fp-gate-no-panic | MAJOR | `.expect()` on Result | `webhook_url.rs:114` |
| GEN-1-2 | fp-gate-exhaustive | MAJOR | wildcard enum match | `parsing.rs:66` |

### High Priority Issues

| ID | Dimension | Severity | Finding |
|----|-----------|----------|---------|
| GEN-1-6 | quality-dry | MAJOR | unnecessary_wraps in `in_memory.rs:137` |
| GEN-1-6 | quality-dry | MAJOR | redundant_clone in `in_memory.rs:164` |
| GEN-1-12 | fowler-security | MAJOR | RUSTSEC-2023-0071 RSA (no fix available) |
| GEN-1-12 | fowler-security | MINOR | RUSTSEC-2026-0098 rustls-webpki (upgrade available) |

---

## Section 4 — Test Implementation

**Location:** `crates/twerk-cli/tests/e2e_cli_test.rs`

**Test Results:** 21 tests, ALL PASS

| Module | Tests | Status |
|--------|-------|--------|
| contract | 6 (C1-C6) | PASS |
| behavioral | 11 (B1-B14) | PASS |
| error_handling | 5 (E1-E5) | PASS |
| json_output | 2 | PASS |

---

## Section 5 — BDD Scenarios

### C1: Help Command

```
Given: twerk is installed
When:  user runs "twerk --help"
Then:  help text is displayed
And:  exit code is 0
```

### E1: Invalid Run Mode

```
Given: twerk is installed
When:  user runs "twerk run invalid-mode"
Then:  error message is shown
And:  exit code is 2
```

### E4: JSON Mode Suppresses Banner

```
Given: twerk is installed
When:  user runs "twerk --json run standalone"
Then:  no banner is displayed
And:  JSON error output is produced
```

---

## Section 6 — Recommendations

1. **Fix expect() in webhook_url.rs** - Replace with proper error handling
2. **Fix wildcard match in parsing.rs:66** - Use explicit variants
3. **Fix DRY violations in in_memory.rs** - Remove unnecessary_wraps and redundant_clone
4. **Run cargo fmt** on test files
5. **Fix test failures** - Investigate cargo test failures
6. **Address security vulnerabilities** - Upgrade rustls-webpki, monitor RSA

---

## Section 7 — Beads Filed

| Bead | Title |
|------|-------|
| tw-bmu | Red Queen MAJOR: fp-gate-no-panic — .expect() in webhook_url.rs:114 |
| tw-60d | Red Queen MAJOR: fp-gate-exhaustive — wildcard enum match in parsing.rs:66 |
| tw-ek0 | Red Queen MAJOR: quality-dry — unnecessary_wraps/redundant_clone in in_memory.rs |
| tw-y4b | Red Queen MAJOR: fowler-security — RUSTSEC-2023-0071 RSA vulnerability |
| tw-qre | Red Queen MINOR: fowler-security — RUSTSEC-2026-0098 rustls-webpki |
| tw-jy3 | Red Queen MINOR: fp-gate-format — formatting issues |
| tw-o8o | Red Queen MAJOR: fp-gate-no-panic (liza auto-filed) |
| tw-22k | Red Queen MAJOR: fp-gate-exhaustive (liza auto-filed) |
| tw-cem | Red Queen MAJOR: quality-dry (liza auto-filed) |

---

## Verification

```bash
# Contract checks (should all pass)
./target/release/twerk --help
./target/release/twerk --version
./target/release/twerk --json
./target/release/twerk run --help
./target/release/twerk migration --help
./target/release/twerk health --help
./target/release/twerk run invalid-mode  # should exit 2
./target/release/twerk run               # should exit 2
./target/release/twerk migration         # should exit 1

# Run E2E tests
cargo test -p twerk-cli --test e2e_cli_test
```
