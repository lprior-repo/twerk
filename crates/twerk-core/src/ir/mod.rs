//! Compiled Workflow IR - hot path types
//!
//! YAML source -> ParsedWorkflow -> ValidatedWorkflow -> CompiledWorkflow
//! CompiledWorkflow is immutable and contains no strings for hot execution except user values.

pub mod types;
pub mod workflow;
pub mod slot;
pub mod expression;
pub mod expr_vm;
pub mod source_map;