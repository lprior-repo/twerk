#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _: Result<serde_json::Value, _> = twerk_web::api::yaml::from_slice(data);
});
