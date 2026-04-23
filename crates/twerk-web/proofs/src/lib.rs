// Kani proof harnesses for twerk-web API domain types
//
// These harnesses verify:
// 1. TriggerId parsing and validation boundaries
// 2. Username and Password construction rules
// 3. Page and PageSize boundary conditions

mod trigger_id_harness;
mod auth_harness;
mod pagination_harness;
mod validate_trigger_harness;
