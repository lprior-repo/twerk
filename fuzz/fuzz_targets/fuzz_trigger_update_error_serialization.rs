#![no_main]

use arbitrary::{Arbitrary, Unstructured};
use libfuzzer_sys::fuzz_target;
use twerk_web::api::trigger_api::TriggerUpdateError;

#[derive(arbitrary::Arbitrary)]
struct ErrorInput<'a> {
    variant_index: u8,
    string_data: &'a [u8],
}

fuzz_target!(|input: ErrorInput| {
    let string_val = String::from_utf8_lossy(input.string_data).to_string();

    let error = match input.variant_index % 10 {
        0 => TriggerUpdateError::InvalidIdFormat(string_val.clone()),
        1 => TriggerUpdateError::UnsupportedContentType(string_val.clone()),
        2 => TriggerUpdateError::MalformedJson(string_val.clone()),
        3 => TriggerUpdateError::ValidationFailed(string_val.clone()),
        4 => TriggerUpdateError::IdMismatch {
            path_id: string_val.clone(),
            body_id: string_val.clone(),
        },
        5 => TriggerUpdateError::TriggerNotFound(string_val.clone()),
        6 => TriggerUpdateError::VersionConflict(string_val.clone()),
        7 => TriggerUpdateError::Persistence(string_val.clone()),
        8 => TriggerUpdateError::Serialization(string_val.clone()),
        _ => return,
    };

    if let Ok(serialized) = serde_json::to_string(&error) {
        let _ = serde_json::from_str::<TriggerUpdateError>(&serialized);
    }
});
