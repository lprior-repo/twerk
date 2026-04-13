//! Domain newtypes for twerk-web API layer.
//!
//! This module re-exports types from the domain/ subdirectory.
//!
//! # Design Principles
//!
//! - **Parse, don't validate**: Raw input is parsed into domain types at
//!   boundary entry points. Core logic receives only validated types.
//! - **Make illegal states unrepresentable**: Newtypes encode business rules
//!   that cannot be violated after construction.

// Re-export all domain types for convenience
pub use super::domain::api::{
    ApiFeature, ContentType, FeatureFlags, ServerAddress, ServerAddressError,
};
pub use super::domain::auth::{Password, PasswordError, Username, UsernameError};
pub use super::domain::pagination::{
    Page, PageError as PaginationPageError, PageSize, PageSizeError as PaginationPageSizeError,
};
pub use super::domain::search::SearchQuery;
