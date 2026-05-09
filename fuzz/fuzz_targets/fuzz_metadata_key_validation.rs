#![no_main]

use arbitrary::{Arbitrary, Unstructured};
use libfuzzer_sys::fuzz_target;
use std::collections::HashMap;
use twerk_web::api::trigger_api::domain::validate_metadata;

#[derive(arbitrary::Arbitrary)]
struct MetadataInput<'a> {
    data: &'a [u8],
}

fuzz_target!(|input: MetadataInput| {
    let u = Unstructured::new(input.data);
    if let Ok(metadata) = <Option<HashMap<String, String>> as Arbitrary>::arbitrary(u) {
        let _ = validate_metadata(metadata.as_ref());
    }
});
