//! Container module for Docker task execution.
//!
//! Provides container lifecycle management for task execution.

pub mod archive;
pub mod factory;
pub mod monitoring;
pub mod probe;
pub mod tcontainer;

// Re-export public types
pub use factory::create_task_container;
pub use tcontainer::Tcontainer;

/// Container is a type alias for Tcontainer for ergonomic use.
pub type Container = Tcontainer;
