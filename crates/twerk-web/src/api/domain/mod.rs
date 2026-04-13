//! Domain newtypes for twerk-web API layer.
//!
//! These types enforce validation at the API boundary, ensuring that invalid
//! representations are rejected before reaching core business logic.
//!
//! # Design Principles
//!
//! - **Parse, don't validate**: Raw input is parsed into domain types at
//!   boundary entry points. Core logic receives only validated types.
//! - **Make illegal states unrepresentable**: Newtypes encode business rules
//!   that cannot be violated after construction.
//! - **Zero-cost abstractions**: Newtypes are compile-time enforced with no
//!   runtime overhead beyond their validation.

pub mod api;
pub mod auth;
pub mod pagination;
pub mod search;

// Re-exports for convenience
pub use api::{ApiFeature, ContentType, FeatureFlags, ServerAddress};
pub use auth::{Password, Username};
pub use pagination::{Page, PageError, PageSize, PageSizeError};
pub use search::SearchQuery;

// Error re-exports
pub use api::ServerAddressError;
pub use auth::{PasswordError, UsernameError};
pub use pagination::{PageError as PaginationPageError, PageSizeError as PaginationPageSizeError};
