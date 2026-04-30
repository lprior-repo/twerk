#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![forbid(unsafe_code)]

pub mod api;
pub mod helpers;
pub mod middleware;

pub use api::openapi::ApiDoc;
pub use api::{create_router, AppState, Config};
