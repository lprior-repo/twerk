// Kani proof harnesses for twerk-infrastructure
//
// These harnesses verify:
// 1. parse_go_duration correctness for various units (s, m, h, d)
// 2. parse_memory_bytes correctness for GB, MB, and plain integers
// 3. slugify output invariants (lowercase, valid character set)
// 4. prefixed_queue / extract_engine_id round-trip and edge cases
// 5. podman slug::make output invariants (lowercase, valid character set)
//
// NOTE: docker::reference::parse is NOT covered here because it relies on
// the `regex` crate, which is not supported under Kani verification.

mod docker_helpers_harness;
mod broker_utils_harness;
mod slug_harness;
