#![no_main]

use arbitrary::{Arbitrary, Unstructured};
use axum::http::{header::CONTENT_TYPE, HeaderMap, HeaderValue};
use libfuzzer_sys::fuzz_target;
use twerk_web::api::trigger_api::handlers::prepare_update;

#[derive(arbitrary::Arbitrary)]
struct EnvelopeInput<'a> {
    path_id: &'a [u8],
    body: &'a [u8],
    content_type: &'a [u8],
}

fuzz_target!(|input: EnvelopeInput| {
    let path_id = String::from_utf8_lossy(input.path_id).to_string();

    let mut headers = HeaderMap::new();
    if !input.content_type.is_empty() {
        if let Ok(ct) = HeaderValue::from_bytes(input.content_type) {
            headers.insert(CONTENT_TYPE, ct);
        }
    }

    let body = axum::body::Bytes::from(input.body.to_vec());
    let _ = prepare_update(&headers, &body, &path_id);
});
