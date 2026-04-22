
package validation

import "list"

// Validation schema for bead: twerk-20260421192311-hwfde4sc
// Title: cli: Implement Viper-style long/short help system
//
// This schema validates that implementation is complete.
// Use: cue vet twerk-20260421192311-hwfde4sc.cue implementation.cue

#BeadImplementation: {
  bead_id: "twerk-20260421192311-hwfde4sc"
  title: "cli: Implement Viper-style long/short help system"

  // Contract verification
  contracts_verified: {
    preconditions_checked: bool & true
    postconditions_verified: bool & true
    invariants_maintained: bool & true

    // Specific preconditions that must be verified
    precondition_checks: [
      "Command parser recognizes --help and --long flags",
    ]

    // Specific postconditions that must be verified
    postcondition_checks: [
      "Short help shows usage and flags",
      "Long help shows description, examples, input/output formats",
    ]

    // Specific invariants that must be maintained
    invariant_checks: [
      "Help always exits with code 0",
      "Help output is valid text/JSON based on --json flag",
    ]
  }

  // Test verification
  tests_passing: {
    all_tests_pass: bool & true

    happy_path_tests: [...string] & list.MinItems(2)
    error_path_tests: [...string] & list.MinItems(2)

    // Note: Actual test names provided by implementer, must include all required tests

    // Required happy path tests
    required_happy_tests: [
      "twerk job create --help displays brief usage",
      "twerk job create --help --long displays rich examples",
    ]

    // Required error path tests
    required_error_tests: [
      "Unknown flag combination returns exit 2",
      "Help for nonexistent command returns exit 2",
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