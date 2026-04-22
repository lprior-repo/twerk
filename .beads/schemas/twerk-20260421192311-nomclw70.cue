
package validation

import "list"

// Validation schema for bead: twerk-20260421192311-nomclw70
// Title: cli: Define exit code system and error taxonomy
//
// This schema validates that implementation is complete.
// Use: cue vet twerk-20260421192311-nomclw70.cue implementation.cue

#BeadImplementation: {
  bead_id: "twerk-20260421192311-nomclw70"
  title: "cli: Define exit code system and error taxonomy"

  // Contract verification
  contracts_verified: {
    preconditions_checked: bool & true
    postconditions_verified: bool & true
    invariants_maintained: bool & true

    // Specific preconditions that must be verified
    precondition_checks: [
      "CLI binary is invoked with arguments",
    ]

    // Specific postconditions that must be verified
    postcondition_checks: [
      "exit_code is 0 on success",
      "exit_code is 1 on runtime error",
      "exit_code is 2 on validation/parse error",
    ]

    // Specific invariants that must be maintained
    invariant_checks: [
      "Exit codes are never negative",
      "Exit codes are always i32 values",
    ]
  }

  // Test verification
  tests_passing: {
    all_tests_pass: bool & true

    happy_path_tests: [...string] & list.MinItems(2)
    error_path_tests: [...string] & list.MinItems(3)

    // Note: Actual test names provided by implementer, must include all required tests

    // Required happy path tests
    required_happy_tests: [
      "Valid command with no args shows help and returns 0",
      "Health check to healthy endpoint returns exit 0",
    ]

    // Required error path tests
    required_error_tests: [
      "Invalid JSON input returns exit code 2",
      "Network failure returns exit code 1",
      "Missing required argument returns exit code 2",
    ]
  }

  // Code completion
  code_complete: {
    implementation_exists: string  // Path to implementation file
    tests_exist: string  // Path to test file
    ci_passing: bool & true
    no_unwrap_calls: bool & true  // Rust/functional constraint
    no_panics: bool & true  // Rust constraint
  }

  // Completion criteria
  completion: {
    all_sections_complete: bool & true
    documentation_updated: bool
    beads_closed: bool
    timestamp: string  // ISO8601 completion timestamp
  }
}

// Example implementation proof - create this file to validate completion:
//
// implementation.cue:
// package validation
//
// implementation: #BeadImplementation & {
//   contracts_verified: {
//     preconditions_checked: true
//     postconditions_verified: true
//     invariants_maintained: true
//     precondition_checks: [/* documented checks */]
//     postcondition_checks: [/* documented verifications */]
//     invariant_checks: [/* documented invariants */]
//   }
//   tests_passing: {
//     all_tests_pass: true
//     happy_path_tests: ["test_version_flag_works", "test_version_format", "test_exit_code_zero"]
//     error_path_tests: ["test_invalid_flag_errors", "test_no_flags_normal_behavior"]
//   }
//   code_complete: {
//     implementation_exists: "src/main.rs"
//     tests_exist: "tests/cli_test.rs"
//     ci_passing: true
//     no_unwrap_calls: true
//     no_panics: true
//   }
//   completion: {
//     all_sections_complete: true
//     documentation_updated: true
//     beads_closed: false
//     timestamp: "2026-04-21T19:23:11Z"
//   }
// }