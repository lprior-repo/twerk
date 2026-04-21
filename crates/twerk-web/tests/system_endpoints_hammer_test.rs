#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::semicolon_if_nothing_returned,
    clippy::too_many_lines,
    clippy::items_after_statements,
    clippy::unused_async,
    clippy::needless_raw_string_hashes
)]

mod support;

#[path = "system_endpoints_hammer_test/mod.rs"]
mod cases;
