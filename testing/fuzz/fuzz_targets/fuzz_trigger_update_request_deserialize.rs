#![no_main]

use arbitrary::{Arbitrary, Unstructured};
use libfuzzer_sys::fuzz_target;
use twerk_web::api::trigger_api::TriggerUpdateRequest;

#[derive(arbitrary::Arbitrary)]
struct TriggerUpdateRequestInput<'a> {
    json_bytes: &'a [u8],
}

fuzz_target!(|input: TriggerUpdateRequestInput| {
    let _ = serde_json::from_slice::<TriggerUpdateRequest>(input.json_bytes);
});
