//! Fuzz target for CronExpression::new with arbitrary string input.
//!
//! This target fuzzes the CronExpression constructor to ensure it never panics
//! on any arbitrary string input.
//!
//! Risk: Panic from cron crate parsing failure, logic error in field count validation
//! Corpus seeds: ["0 0 * * *", "*/15 * * * MON-FRI", "0 30 8 1 * *", "not cron", ""]

#![no_main]

use libfuzzer_sys::fuzz_target;
use twerk_core::domain::CronExpression;

fuzz_target!(|data: &str| {
    // Must not panic — any input is valid to attempt parsing
    let _ = CronExpression::new(data);
});
