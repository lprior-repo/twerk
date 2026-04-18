#![no_main]

use arbitrary::{Arbitrary, Unstructured};
use libfuzzer_sys::fuzz_target;
use twerk_web::api::trigger_api::TriggerId;

#[derive(arbitrary::Arbitrary)]
struct TriggerIdInput<'a> {
    data: &'a [u8],
}

fuzz_target!(|input: TriggerIdInput| {
    if let Ok(s) = std::str::from_utf8(input.data) {
        let _ = TriggerId::parse(s);
    }
});
