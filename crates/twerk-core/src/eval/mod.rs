//! Expression evaluation module — 100% parity with Go `internal/eval`.
//!
//! Provides template evaluation with `{{ expression }}` syntax and
//! support for built-in functions like `randomInt()` and `sequence()`.
//! Uses the `evalexpr` crate for expression evaluation support.
//!
//! ## Architecture
//!
//! This module follows functional-first design:
//! - **Data**: `EvalError` for error taxonomy
//! - **Calculations**: Pure functions for expression evaluation
//! - **Actions**: Public API functions for external use
//!
//! ## Modules
//!
//! - [`condition`] - Job and task condition evaluation
//! - [`context`] - Context building and JSON conversion
//! - [`functions`] - Built-in evalexpr functions
//! - [`task`] - Recursive task template evaluation
//! - [`template`] - Template string evaluation
//! - [`transform`] - Expression sanitization and operator transforms

use regex::Regex;
use thiserror::Error;

pub mod condition;
pub mod context;
pub mod data_flow;
pub mod functions;
pub mod intrinsics;
pub mod state_dispatch;
pub mod task;
pub mod template;
pub mod transform;
pub mod vm;

// Re-export all public APIs at module level for convenience
pub use condition::{evaluate_condition, evaluate_task_condition};
pub use state_dispatch::{evaluate_state, evaluate_state_machine, StateEvalError};
pub use task::evaluate_task;
pub use template::{evaluate_expr, evaluate_template, valid_expr};
pub use transform::{sanitize_expr, transform_operators};

fn get_template_regex() -> Result<Regex, EvalError> {
    Regex::new(r"\{\{\s*(.+?)\s*\}\}")
        .map_err(|error| EvalError::CompileError("template regex".into(), error.to_string()))
}

/// Errors that can occur during evaluation.
#[derive(Debug, Error, PartialEq)]
pub enum EvalError {
    /// Failed to compile an expression or regex.
    #[error("error compiling expression '{0}': {1}")]
    CompileError(String, String),

    /// Failed to evaluate an expression.
    #[error("error evaluating expression '{0}': {1}")]
    ExpressionError(String, String),

    /// Expression is invalid or malformed.
    #[error("invalid expression: {0}")]
    InvalidExpression(String),

    /// Requested function is not supported.
    #[error("unsupported function: {0}")]
    UnsupportedFunction(String),
}
