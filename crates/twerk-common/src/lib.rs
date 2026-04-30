#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![forbid(unsafe_code)]

pub mod conf;
pub mod constants;
pub mod env;
pub mod logging;
pub mod reexec;
pub mod slices;
pub mod slot;
pub mod syncx;
pub mod uuid;
pub mod wildcard;

pub use conf::load_config;
pub use env::var_with_twerk_prefix;
pub use logging::{setup_logging, TracingWriter};
pub use slices::{intersect, map_slice};
pub use slot::{SlotAllocator, SlotIdx, SlotValue, MAX_SLOTS};
pub use syncx::Map;
pub use uuid::{new_short_uuid, new_uuid};
pub use wildcard::{is_wild_pattern, match_pattern, match_wildcard, r#match, wildcard_match};
